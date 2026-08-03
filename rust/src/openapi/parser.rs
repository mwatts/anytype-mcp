use openapiv3::{
    AdditionalProperties, OpenAPI, Operation, Parameter, ReferenceOr, Schema, StatusCode, Type,
};
use serde_json::{Map, Value, json};
use tracing::{debug, warn};

use crate::utils::{AnytypeMcpError, Result as McpResult};

/// MCP tool name length limit.
const MAX_TOOL_NAME_LEN: usize = 64;

/// Prefix applied to every generated tool name, matching the TypeScript
/// implementation's `API-<operation>` naming convention.
const TOOL_NAME_PREFIX: &str = "API-";

// MCP Tool representation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub method: String,
    pub path: String,
    pub operation_id: String,
    /// Names of multipart/form-data properties that take local file paths.
    #[serde(default)]
    pub file_upload_params: Vec<String>,
}

pub struct OpenApiParser {
    pub spec: OpenAPI,
}

impl OpenApiParser {
    pub fn new(spec: OpenAPI) -> Self {
        Self { spec }
    }

    pub fn validate(&self) -> McpResult<()> {
        debug!("Validating OpenAPI specification");

        if self.spec.info.title.is_empty() {
            return Err(AnytypeMcpError::Config(
                "OpenAPI specification must have a title".to_string(),
            ));
        }

        debug!("OpenAPI specification is valid");
        Ok(())
    }

    pub fn convert_to_tools(&self) -> McpResult<Vec<McpTool>> {
        let mut tools = Vec::new();
        let mut name_counter: u32 = 0;

        for (path, path_item) in &self.spec.paths.paths {
            if let Some(path_item) = path_item.as_item() {
                let operations: [(&str, Option<&Operation>); 5] = [
                    ("GET", path_item.get.as_ref()),
                    ("POST", path_item.post.as_ref()),
                    ("PUT", path_item.put.as_ref()),
                    ("DELETE", path_item.delete.as_ref()),
                    ("PATCH", path_item.patch.as_ref()),
                ];
                for (method, operation) in operations {
                    let Some(operation) = operation else { continue };
                    // Auth operations (challenge/api-key issuance) are used by the
                    // get-key flow, not by MCP clients.
                    if operation.tags.iter().any(|t| t == "Auth") {
                        continue;
                    }
                    tools.push(self.process_operation(path, method, operation, &mut name_counter));
                }
            }
        }

        debug!("Converted {} OpenAPI operations to MCP tools", tools.len());
        Ok(tools)
    }

    /// Build the client-visible tool name: `API-` + kebab-case operation id,
    /// truncated to the MCP 64-char limit with a uniqueness suffix if needed.
    fn tool_name(&self, operation_id: &str, name_counter: &mut u32) -> String {
        let name = format!("{}{}", TOOL_NAME_PREFIX, operation_id.replace('_', "-"));
        if name.len() <= MAX_TOOL_NAME_LEN {
            return name;
        }
        *name_counter += 1;
        let truncated: String = name.chars().take(MAX_TOOL_NAME_LEN - 5).collect();
        format!("{}-{:04}", truncated, name_counter)
    }

    fn process_operation(
        &self,
        path: &str,
        method: &str,
        operation: &Operation,
        name_counter: &mut u32,
    ) -> McpTool {
        let default_id = format!("{}_{}", method.to_lowercase(), path.replace('/', "_"));
        let operation_id = operation.operation_id.as_deref().unwrap_or(&default_id);

        let mut properties = Map::new();
        let mut required: Vec<String> = Vec::new();
        let mut file_upload_params = Vec::new();

        // Process query/path/header parameters
        for param_ref in &operation.parameters {
            match param_ref {
                ReferenceOr::Item(param) => {
                    let data = param.parameter_data_ref();
                    // The version header is injected by the HTTP client; never
                    // expose it as a tool argument.
                    if data.name == "Anytype-Version" {
                        continue;
                    }
                    let mut schema = self.parameter_schema(param);
                    if let (Some(desc), None) = (
                        data.description.as_ref(),
                        schema.get("description").and_then(Value::as_str),
                    ) {
                        schema["description"] = json!(desc);
                    }
                    properties.insert(data.name.clone(), schema);
                    if data.required {
                        required.push(data.name.clone());
                    }
                }
                ReferenceOr::Reference { reference } => {
                    warn!("Parameter reference not supported: {}", reference);
                }
            }
        }

        // Process request body
        if let Some(ReferenceOr::Item(request_body)) = &operation.request_body {
            let json_content = request_body.content.get("application/json");
            let multipart_content = request_body.content.get("multipart/form-data");

            if let Some(schema_ref) = json_content.and_then(|c| c.schema.as_ref()) {
                let body_schema = self.convert_schema_or_ref(schema_ref, &mut Vec::new());
                Self::merge_body_schema(
                    body_schema,
                    request_body.required,
                    &mut properties,
                    &mut required,
                );
            } else if let Some(schema_ref) = multipart_content.and_then(|c| c.schema.as_ref()) {
                file_upload_params = Self::binary_property_names(schema_ref);
                let body_schema = self.convert_schema_or_ref(schema_ref, &mut Vec::new());
                Self::merge_body_schema(
                    body_schema,
                    request_body.required,
                    &mut properties,
                    &mut required,
                );
            }
        } else if let Some(ReferenceOr::Reference { reference }) = &operation.request_body {
            warn!("Request body reference not supported: {}", reference);
        }

        McpTool {
            name: self.tool_name(operation_id, name_counter),
            description: Some(self.build_description(operation)),
            input_schema: json!({
                "type": "object",
                "properties": properties,
                "required": required
            }),
            method: method.to_string(),
            path: path.to_string(),
            operation_id: operation_id.to_string(),
            file_upload_params,
        }
    }

    /// Inline an object body's properties at the top level of the tool input
    /// schema (excluding `filters`, which the API's MCP surface doesn't
    /// support yet); non-object bodies become a single `body` property.
    fn merge_body_schema(
        body_schema: Value,
        body_required: bool,
        properties: &mut Map<String, Value>,
        required: &mut Vec<String>,
    ) {
        let is_object_with_props = body_schema.get("properties").is_some_and(|p| p.is_object());

        if is_object_with_props {
            if let Some(props) = body_schema["properties"].as_object() {
                for (name, prop_schema) in props {
                    if name == "filters" {
                        continue;
                    }
                    properties.insert(name.clone(), prop_schema.clone());
                }
            }
            if let Some(body_req) = body_schema.get("required").and_then(Value::as_array) {
                required.extend(
                    body_req
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|r| *r != "filters")
                        .map(String::from),
                );
            }
        } else {
            properties.insert("body".to_string(), body_schema);
            if body_required {
                required.push("body".to_string());
            }
        }
    }

    /// Names of top-level multipart properties with `format: binary`
    /// (directly or as array items) — these accept local file paths.
    fn binary_property_names(schema_ref: &ReferenceOr<Schema>) -> Vec<String> {
        let ReferenceOr::Item(schema) = schema_ref else {
            return Vec::new();
        };
        let openapiv3::SchemaKind::Type(Type::Object(object)) = &schema.schema_kind else {
            return Vec::new();
        };

        fn is_binary(schema_ref: &ReferenceOr<Box<Schema>>) -> bool {
            let ReferenceOr::Item(schema) = schema_ref else {
                return false;
            };
            match &schema.schema_kind {
                openapiv3::SchemaKind::Type(Type::String(s)) => matches!(
                    s.format,
                    openapiv3::VariantOrUnknownOrEmpty::Item(openapiv3::StringFormat::Binary)
                ),
                openapiv3::SchemaKind::Type(Type::Array(a)) => {
                    a.items.as_ref().is_some_and(is_binary)
                }
                _ => false,
            }
        }

        object
            .properties
            .iter()
            .filter(|(_, prop)| is_binary(prop))
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Tool description: summary (or description) plus documented error responses.
    fn build_description(&self, operation: &Operation) -> String {
        let mut description = operation
            .summary
            .clone()
            .or_else(|| operation.description.clone())
            .unwrap_or_default();

        let mut error_lines = Vec::new();
        for (status, response_ref) in &operation.responses.responses {
            let is_error = match status {
                StatusCode::Code(code) => *code >= 400,
                StatusCode::Range(range) => *range >= 4,
            };
            if !is_error {
                continue;
            }
            let desc = match response_ref {
                ReferenceOr::Item(response) => response.description.clone(),
                ReferenceOr::Reference { .. } => String::new(),
            };
            error_lines.push(format!("{}: {}", status, desc));
        }
        if !error_lines.is_empty() {
            description.push_str("\nError Responses:\n");
            description.push_str(&error_lines.join("\n"));
        }
        description
    }

    fn parameter_schema(&self, param: &Parameter) -> Value {
        let param_data = param.parameter_data_ref();

        match &param_data.format {
            openapiv3::ParameterSchemaOrContent::Schema(schema_ref) => {
                self.convert_schema_or_ref(schema_ref, &mut Vec::new())
            }
            openapiv3::ParameterSchemaOrContent::Content(content_map) => {
                for media in content_map.values() {
                    if let Some(ref schema_ref) = media.schema {
                        return self.convert_schema_or_ref(schema_ref, &mut Vec::new());
                    }
                }
                json!({"type": "string"}) // fallback
            }
        }
    }

    /// Resolve a `#/components/schemas/<name>` reference to its schema.
    fn resolve_schema_ref(&self, reference: &str) -> Option<&Schema> {
        let name = reference.strip_prefix("#/components/schemas/")?;
        let mut schema_ref = self.spec.components.as_ref()?.schemas.get(name)?;
        // Follow at most one level of ref-to-ref indirection.
        if let ReferenceOr::Reference { reference } = schema_ref {
            let name = reference.strip_prefix("#/components/schemas/")?;
            schema_ref = self.spec.components.as_ref()?.schemas.get(name)?;
        }
        schema_ref.as_item()
    }

    /// Convert a schema-or-reference, inlining resolved references.
    ///
    /// `ref_stack` tracks in-progress reference names for cycle detection;
    /// a cyclic reference degrades to a generic object schema.
    fn convert_schema_or_ref(
        &self,
        schema_ref: &ReferenceOr<Schema>,
        ref_stack: &mut Vec<String>,
    ) -> Value {
        match schema_ref {
            ReferenceOr::Item(schema) => self.convert_schema(schema, ref_stack),
            ReferenceOr::Reference { reference } => self.convert_reference(reference, ref_stack),
        }
    }

    fn convert_boxed_schema_or_ref(
        &self,
        schema_ref: &ReferenceOr<Box<Schema>>,
        ref_stack: &mut Vec<String>,
    ) -> Value {
        match schema_ref {
            ReferenceOr::Item(boxed_schema) => self.convert_schema(boxed_schema, ref_stack),
            ReferenceOr::Reference { reference } => self.convert_reference(reference, ref_stack),
        }
    }

    fn convert_reference(&self, reference: &str, ref_stack: &mut Vec<String>) -> Value {
        // Filter expressions are excluded from the MCP surface (matches the
        // TypeScript implementation; filters are stripped from tool inputs).
        if reference.ends_with("FilterExpression") {
            return json!({});
        }
        if ref_stack.iter().any(|r| r == reference) {
            debug!("Cyclic schema reference: {}", reference);
            return json!({"type": "object"});
        }
        match self.resolve_schema_ref(reference) {
            Some(schema) => {
                ref_stack.push(reference.to_string());
                let value = self.convert_schema(schema, ref_stack);
                ref_stack.pop();
                value
            }
            None => {
                warn!("Unresolvable schema reference: {}", reference);
                json!({"type": "object"})
            }
        }
    }

    fn convert_schema(&self, schema: &Schema, ref_stack: &mut Vec<String>) -> Value {
        let mut json_schema = json!({});
        let mut binary_format = false;

        match &schema.schema_kind {
            openapiv3::SchemaKind::Type(type_) => {
                match type_ {
                    Type::String(string_type) => {
                        json_schema["type"] = json!("string");

                        match &string_type.format {
                            openapiv3::VariantOrUnknownOrEmpty::Item(format) => {
                                use openapiv3::StringFormat;
                                match format {
                                    StringFormat::Binary => {
                                        // File contents are passed as local file paths.
                                        json_schema["format"] = json!("uri-reference");
                                        binary_format = true;
                                    }
                                    StringFormat::Date => json_schema["format"] = json!("date"),
                                    StringFormat::DateTime => {
                                        json_schema["format"] = json!("date-time")
                                    }
                                    StringFormat::Password => {
                                        json_schema["format"] = json!("password")
                                    }
                                    StringFormat::Byte => json_schema["format"] = json!("byte"),
                                }
                            }
                            openapiv3::VariantOrUnknownOrEmpty::Unknown(format) => {
                                json_schema["format"] = json!(format);
                            }
                            openapiv3::VariantOrUnknownOrEmpty::Empty => {}
                        }

                        // Handle enumeration
                        if !string_type.enumeration.is_empty() {
                            let enum_values: Vec<_> = string_type
                                .enumeration
                                .iter()
                                .filter_map(|opt| opt.as_ref())
                                .collect();
                            if !enum_values.is_empty() {
                                json_schema["enum"] = json!(enum_values);
                            }
                        }
                    }
                    Type::Number(number_type) => {
                        json_schema["type"] = json!("number");

                        match &number_type.format {
                            openapiv3::VariantOrUnknownOrEmpty::Item(format) => {
                                json_schema["format"] = match format {
                                    openapiv3::NumberFormat::Float => json!("float"),
                                    openapiv3::NumberFormat::Double => json!("double"),
                                };
                            }
                            openapiv3::VariantOrUnknownOrEmpty::Unknown(format) => {
                                json_schema["format"] = json!(format);
                            }
                            openapiv3::VariantOrUnknownOrEmpty::Empty => {}
                        }
                    }
                    Type::Integer(integer_type) => {
                        json_schema["type"] = json!("integer");

                        match &integer_type.format {
                            openapiv3::VariantOrUnknownOrEmpty::Item(format) => {
                                json_schema["format"] = match format {
                                    openapiv3::IntegerFormat::Int32 => json!("int32"),
                                    openapiv3::IntegerFormat::Int64 => json!("int64"),
                                };
                            }
                            openapiv3::VariantOrUnknownOrEmpty::Unknown(format) => {
                                json_schema["format"] = json!(format);
                            }
                            openapiv3::VariantOrUnknownOrEmpty::Empty => {}
                        }
                    }
                    Type::Object(object_type) => {
                        json_schema["type"] = json!("object");

                        if !object_type.properties.is_empty() {
                            let mut properties = Map::new();
                            for (key, schema_ref) in &object_type.properties {
                                properties.insert(
                                    key.clone(),
                                    self.convert_boxed_schema_or_ref(schema_ref, ref_stack),
                                );
                            }
                            json_schema["properties"] = Value::Object(properties);
                        }

                        if !object_type.required.is_empty() {
                            json_schema["required"] = json!(object_type.required);
                        }

                        match &object_type.additional_properties {
                            None | Some(AdditionalProperties::Any(true)) => {
                                json_schema["additionalProperties"] = json!(true);
                            }
                            Some(AdditionalProperties::Any(false)) => {
                                json_schema["additionalProperties"] = json!(false);
                            }
                            Some(AdditionalProperties::Schema(schema_ref)) => {
                                json_schema["additionalProperties"] =
                                    self.convert_schema_or_ref(schema_ref, ref_stack);
                            }
                        }
                    }
                    Type::Array(array_type) => {
                        json_schema["type"] = json!("array");
                        if let Some(ref items) = array_type.items {
                            json_schema["items"] =
                                self.convert_boxed_schema_or_ref(items, ref_stack);
                        }
                    }
                    Type::Boolean(_) => {
                        json_schema["type"] = json!("boolean");
                    }
                }
            }
            openapiv3::SchemaKind::OneOf { one_of } => {
                if let Some(special) = self.convert_special_one_of(one_of, schema) {
                    return special;
                }
                let schemas: Vec<_> = one_of
                    .iter()
                    .map(|s| self.convert_schema_or_ref(s, ref_stack))
                    .collect();
                json_schema["oneOf"] = json!(schemas);
            }
            openapiv3::SchemaKind::AllOf { all_of } => {
                let schemas: Vec<_> = all_of
                    .iter()
                    .map(|s| self.convert_schema_or_ref(s, ref_stack))
                    .collect();
                json_schema["allOf"] = json!(schemas);
            }
            openapiv3::SchemaKind::AnyOf { any_of } => {
                let schemas: Vec<_> = any_of
                    .iter()
                    .map(|s| self.convert_schema_or_ref(s, ref_stack))
                    .collect();
                json_schema["anyOf"] = json!(schemas);
            }
            openapiv3::SchemaKind::Any(any) => {
                // Mixed schemas (e.g. `type: object` combined with `oneOf`)
                // deserialize as AnySchema.
                if !any.one_of.is_empty() {
                    if let Some(special) = self.convert_special_one_of(&any.one_of, schema) {
                        return special;
                    }
                    let schemas: Vec<_> = any
                        .one_of
                        .iter()
                        .map(|s| self.convert_schema_or_ref(s, ref_stack))
                        .collect();
                    json_schema["oneOf"] = json!(schemas);
                } else if !any.all_of.is_empty() {
                    let schemas: Vec<_> = any
                        .all_of
                        .iter()
                        .map(|s| self.convert_schema_or_ref(s, ref_stack))
                        .collect();
                    json_schema["allOf"] = json!(schemas);
                } else if !any.any_of.is_empty() {
                    let schemas: Vec<_> = any
                        .any_of
                        .iter()
                        .map(|s| self.convert_schema_or_ref(s, ref_stack))
                        .collect();
                    json_schema["anyOf"] = json!(schemas);
                } else {
                    if let Some(typ) = &any.typ {
                        json_schema["type"] = json!(typ);
                    }
                    if !any.properties.is_empty() {
                        let mut properties = Map::new();
                        for (key, schema_ref) in &any.properties {
                            properties.insert(
                                key.clone(),
                                self.convert_boxed_schema_or_ref(schema_ref, ref_stack),
                            );
                        }
                        json_schema["properties"] = Value::Object(properties);
                        if json_schema.get("type").is_none() {
                            json_schema["type"] = json!("object");
                        }
                    }
                    if !any.required.is_empty() {
                        json_schema["required"] = json!(any.required);
                    }
                    if let Some(items) = &any.items {
                        json_schema["items"] = self.convert_boxed_schema_or_ref(items, ref_stack);
                    }
                    if let Some(format) = &any.format {
                        if format == "binary" {
                            json_schema["format"] = json!("uri-reference");
                            binary_format = true;
                        } else {
                            json_schema["format"] = json!(format);
                        }
                    }
                    if !any.enumeration.is_empty() {
                        json_schema["enum"] = json!(any.enumeration);
                    }
                    if json_schema.as_object().is_some_and(Map::is_empty) {
                        json_schema["type"] = json!("object");
                    }
                }
            }
            _ => {
                // Default to object for unhandled schema kinds
                json_schema["type"] = json!("object");
            }
        }

        // Add title, description, and default if available
        if let Some(ref title) = schema.schema_data.title {
            json_schema["title"] = json!(title);
        }
        if binary_format {
            let binary_desc = "absolute paths to local files";
            json_schema["description"] = match &schema.schema_data.description {
                Some(desc) => json!(format!("{} ({})", desc, binary_desc)),
                None => json!(binary_desc),
            };
        } else if let Some(ref description) = schema.schema_data.description {
            json_schema["description"] = json!(description);
        }
        if let Some(ref default) = schema.schema_data.default {
            json_schema["default"] = default.clone();
        }

        json_schema
    }

    /// Special-cased `oneOf` unions, ported from the TypeScript parser:
    /// icon unions collapse to an emoji-only schema, and property value
    /// unions flatten into a single object exposing every value field.
    fn convert_special_one_of(
        &self,
        one_of: &[ReferenceOr<Schema>],
        schema: &Schema,
    ) -> Option<Value> {
        let ref_names: Vec<&str> = one_of
            .iter()
            .filter_map(|s| match s {
                ReferenceOr::Reference { reference } => Some(reference.as_str()),
                ReferenceOr::Item(_) => None,
            })
            .collect();

        // Icon unions: only the emoji variant is exposed.
        if ref_names.contains(&"#/components/schemas/EmojiIcon") {
            let mut result = json!({
                "type": "object",
                "properties": {
                    "emoji": {
                        "type": "string",
                        "description": "The emoji of the icon"
                    },
                    "format": {
                        "type": "string",
                        "description": "The format of the icon",
                        "enum": ["emoji"]
                    }
                },
                "additionalProperties": true
            });
            if let Some(ref description) = schema.schema_data.description {
                result["description"] = json!(description);
            }
            return Some(result);
        }

        let all_refs = ref_names.len() == one_of.len() && !ref_names.is_empty();
        let is_property_value = all_refs && ref_names.iter().all(|r| r.ends_with("PropertyValue"));
        let is_property_link_value =
            all_refs && ref_names.iter().all(|r| r.ends_with("PropertyLinkValue"));

        if !is_property_value && !is_property_link_value {
            return None;
        }

        let mut properties = if is_property_link_value {
            json!({
                "key": {
                    "type": "string",
                    "description": "The key of the property",
                    "examples": ["last_modified_date"]
                }
            })
        } else {
            json!({
                "id": {
                    "type": "string",
                    "description": "The id of the property",
                    "examples": ["last_modified_date"]
                },
                "key": {
                    "type": "string",
                    "description": "The key of the property",
                    "examples": ["last_modified_date"]
                },
                "name": {
                    "type": "string",
                    "description": "The name of the property",
                    "examples": ["Last modified date"]
                },
                "object": {
                    "type": "string",
                    "description": "The data model of the object",
                    "examples": ["property"]
                }
            })
        };

        let value_fields = json!({
            "text": {
                "type": "string",
                "description": "The text value, if applicable",
                "examples": ["Some text..."]
            },
            "number": {
                "type": "number",
                "description": "The number value, if applicable",
                "examples": [42]
            },
            "select": {
                "type": "string",
                "description": "The selected tag id, if applicable",
                "examples": ["tag_id"]
            },
            "multi_select": {
                "type": "array",
                "description": "The selected tag ids, if applicable",
                "items": {"type": "string"},
                "examples": [["tag_id"]]
            },
            "date": {
                "type": "string",
                "description": "The date value in ISO 8601 format, if applicable",
                "examples": ["2025-02-14T12:34:56Z"]
            },
            "files": {
                "type": "array",
                "description": "The file ids, if applicable",
                "items": {"type": "string"},
                "examples": [["['file_id']"]]
            },
            "checkbox": {
                "type": "boolean",
                "description": "The checkbox value, if applicable",
                "examples": [true]
            },
            "url": {
                "type": "string",
                "description": "The url value, if applicable",
                "examples": ["https://example.com"]
            },
            "email": {
                "type": "string",
                "description": "The email value, if applicable",
                "examples": ["example@example.com"]
            },
            "phone": {
                "type": "string",
                "description": "The phone number value, if applicable",
                "examples": ["+1234567890"]
            },
            "objects": {
                "type": "array",
                "description": "The object ids, if applicable",
                "items": {"type": "string"},
                "examples": [["['object_id']"]]
            }
        });

        let props = properties.as_object_mut().expect("built as object");
        for (key, value) in value_fields.as_object().expect("built as object") {
            props.insert(key.clone(), value.clone());
        }

        Some(json!({
            "type": "object",
            "properties": properties
        }))
    }
}
