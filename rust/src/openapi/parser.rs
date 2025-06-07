use std::collections::HashMap;
use openapiv3::{OpenAPI, Operation, Parameter, ReferenceOr, Schema, Type};
use serde_json::{Value, json};
use tracing::{debug, warn};

use crate::utils::{Result as McpResult, AnytypeMcpError};

// MCP Tool representation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub method: String,
    pub path: String,
    pub operation_id: String,
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
                "OpenAPI specification must have a title".to_string()
            ));
        }

        debug!("OpenAPI specification is valid");
        Ok(())
    }

    pub fn convert_to_tools(&self) -> McpResult<Vec<McpTool>> {
        let mut tools = Vec::new();

        // Process paths directly since paths is not Option
        for (path, path_item) in &self.spec.paths.paths {
            if let Some(path_item) = path_item.as_item() {
                // Handle different HTTP methods
                if let Some(ref operation) = path_item.get {
                    if let Ok(tool) = self.process_operation(path, "GET", operation) {
                        tools.push(tool);
                    }
                }
                if let Some(ref operation) = path_item.post {
                    if let Ok(tool) = self.process_operation(path, "POST", operation) {
                        tools.push(tool);
                    }
                }
                if let Some(ref operation) = path_item.put {
                    if let Ok(tool) = self.process_operation(path, "PUT", operation) {
                        tools.push(tool);
                    }
                }
                if let Some(ref operation) = path_item.delete {
                    if let Ok(tool) = self.process_operation(path, "DELETE", operation) {
                        tools.push(tool);
                    }
                }
                if let Some(ref operation) = path_item.patch {
                    if let Ok(tool) = self.process_operation(path, "PATCH", operation) {
                        tools.push(tool);
                    }
                }
            }
        }

        debug!("Converted {} OpenAPI operations to MCP tools", tools.len());
        Ok(tools)
    }

    fn process_operation(&self, path: &str, method: &str, operation: &Operation) -> McpResult<McpTool> {
        let default_id = format!("{}_{}", method.to_lowercase(), path.replace('/', "_"));
        let operation_id = operation.operation_id.as_deref()
            .unwrap_or(&default_id);

        let mut tool = McpTool {
            name: operation_id.to_string(),
            description: operation.description.clone().or_else(|| operation.summary.clone()),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            method: method.to_string(),
            path: path.to_string(),
            operation_id: operation_id.to_string(),
        };

        let mut properties = HashMap::new();
        let mut required = Vec::new();

        // Process parameters
        for param_ref in &operation.parameters {
            match param_ref {
                ReferenceOr::Item(param) => {
                    if let Ok(schema) = self.process_parameter(param) {
                        properties.insert(param.parameter_data_ref().name.clone(), schema);
                        if param.parameter_data_ref().required {
                            required.push(param.parameter_data_ref().name.clone());
                        }
                    }
                }
                ReferenceOr::Reference { reference } => {
                    warn!("Parameter reference not supported yet: {}", reference);
                }
            }
        }

        // Process request body for POST/PUT/PATCH
        if let Some(ref request_body_ref) = operation.request_body {
            match request_body_ref {
                ReferenceOr::Item(request_body) => {
                    if let Some(content) = request_body.content.get("application/json") {
                        if let Some(ref schema_ref) = content.schema {
                            if let Ok(schema) = self.convert_schema_or_ref(schema_ref) {
                                properties.insert("body".to_string(), schema);
                                if request_body.required {
                                    required.push("body".to_string());
                                }
                            }
                        }
                    }
                }
                ReferenceOr::Reference { reference } => {
                    warn!("Request body reference not supported yet: {}", reference);
                }
            }
        }

        tool.input_schema = json!({
            "type": "object",
            "properties": properties,
            "required": required
        });

        Ok(tool)
    }

    fn process_parameter(&self, param: &Parameter) -> McpResult<Value> {
        let param_data = param.parameter_data_ref();

        // Handle the parameter schema/content
        match &param_data.format {
            openapiv3::ParameterSchemaOrContent::Schema(schema_ref) => {
                self.convert_schema_or_ref(schema_ref)
            }
            openapiv3::ParameterSchemaOrContent::Content(content_map) => {
                // For content, try to get the first available schema
                for (_media_type, media) in content_map {
                    if let Some(ref schema_ref) = media.schema {
                        return self.convert_schema_or_ref(schema_ref);
                    }
                }
                Ok(json!({"type": "string"})) // fallback
            }
        }
    }

    fn convert_schema_or_ref(&self, schema_ref: &ReferenceOr<Schema>) -> McpResult<Value> {
        match schema_ref {
            ReferenceOr::Item(schema) => self.convert_schema(schema),
            ReferenceOr::Reference { reference } => {
                // For now, just return a generic object schema for references
                warn!("Schema reference not fully supported yet: {}", reference);
                Ok(json!({"type": "object"}))
            }
        }
    }

    fn convert_boxed_schema_or_ref(&self, schema_ref: &ReferenceOr<Box<Schema>>) -> McpResult<Value> {
        match schema_ref {
            ReferenceOr::Item(boxed_schema) => self.convert_schema(boxed_schema),
            ReferenceOr::Reference { reference } => {
                // For now, just return a generic object schema for references
                warn!("Schema reference not fully supported yet: {}", reference);
                Ok(json!({"type": "object"}))
            }
        }
    }

    fn convert_schema(&self, schema: &Schema) -> McpResult<Value> {
        let mut json_schema = json!({});

        match &schema.schema_kind {
            openapiv3::SchemaKind::Type(type_) => {
                match type_ {
                    Type::String(string_type) => {
                        json_schema["type"] = json!("string");

                        // Handle format
                        match &string_type.format {
                            openapiv3::VariantOrUnknownOrEmpty::Item(format) => {
                                json_schema["format"] = json!(format!("{:?}", format));
                            }
                            _ => {}
                        }

                        // Handle enumeration
                        if !string_type.enumeration.is_empty() {
                            let enum_values: Vec<_> = string_type.enumeration.iter()
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
                                json_schema["format"] = json!(format!("{:?}", format));
                            }
                            _ => {}
                        }
                    }
                    Type::Integer(integer_type) => {
                        json_schema["type"] = json!("integer");

                        match &integer_type.format {
                            openapiv3::VariantOrUnknownOrEmpty::Item(format) => {
                                json_schema["format"] = json!(format!("{:?}", format));
                            }
                            _ => {}
                        }
                    }
                    Type::Object(object_type) => {
                        json_schema["type"] = json!("object");

                        if !object_type.properties.is_empty() {
                            let mut properties = HashMap::new();
                            for (key, schema_ref) in &object_type.properties {
                                properties.insert(key.clone(), self.convert_boxed_schema_or_ref(schema_ref)?);
                            }
                            json_schema["properties"] = json!(properties);
                        }

                        if !object_type.required.is_empty() {
                            json_schema["required"] = json!(object_type.required);
                        }
                    }
                    Type::Array(array_type) => {
                        json_schema["type"] = json!("array");
                        if let Some(ref items) = array_type.items {
                            json_schema["items"] = self.convert_boxed_schema_or_ref(items)?;
                        }
                    }
                    Type::Boolean(_) => {
                        json_schema["type"] = json!("boolean");
                    }
                }
            }
            openapiv3::SchemaKind::OneOf { one_of } => {
                let mut schemas = Vec::new();
                for schema_ref in one_of {
                    schemas.push(self.convert_schema_or_ref(schema_ref)?);
                }
                json_schema["oneOf"] = json!(schemas);
            }
            openapiv3::SchemaKind::AllOf { all_of } => {
                let mut schemas = Vec::new();
                for schema_ref in all_of {
                    schemas.push(self.convert_schema_or_ref(schema_ref)?);
                }
                json_schema["allOf"] = json!(schemas);
            }
            openapiv3::SchemaKind::AnyOf { any_of } => {
                let mut schemas = Vec::new();
                for schema_ref in any_of {
                    schemas.push(self.convert_schema_or_ref(schema_ref)?);
                }
                json_schema["anyOf"] = json!(schemas);
            }
            _ => {
                // Default to object for unhandled schema kinds
                json_schema["type"] = json!("object");
            }
        }

        // Add title and description if available
        if let Some(ref title) = schema.schema_data.title {
            json_schema["title"] = json!(title);
        }
        if let Some(ref description) = schema.schema_data.description {
            json_schema["description"] = json!(description);
        }

        Ok(json_schema)
    }
}
