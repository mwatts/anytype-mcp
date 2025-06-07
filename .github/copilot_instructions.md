Use Conventional Commits for all commits. The specification can be found here: https://www.conventionalcommits.org/en/v1.0.0/#specification

workitems should follow the conventions documented here: https://docs.github.com/en/issues

tags should be used to categorize workitems. The tags should be used as follows:
- `bug`: for issues that are bugs or defects in the code
- `enhancement`: for issues that are enhancements or improvements to existing features
- `feature`: for issues that are new features or functionality
- `documentation`: for issues that are related to documentation or comments
- `question`: for issues that are questions or requests for clarification
- `discussion`: for issues that are discussions or proposals for new features or changes
- `help wanted`: for issues that need help or assistance from the community
- `good first issue`: for issues that are good for first-time contributors or beginners
- `wontfix`: for issues that will not be fixed or addressed
- `invalid`: for issues that are invalid or not relevant to the project
- `duplicate`: for issues that are duplicates of existing issues
- `security`: for issues that are related to security vulnerabilities or concerns
- `performance`: for issues that are related to performance improvements or optimizations
- `refactor`: for issues that are related to code refactoring or restructuring
- `test`: for issues that are related to testing or test coverage
- `chore`: for issues that are related to maintenance or housekeeping tasks
- `release`: for issues that are related to releases or versioning
- `blocked`: for issues that are blocked by other issues or tasks
- `in progress`: for issues that are currently being worked on
- `needs review`: for issues that need to be reviewed or approved by someone
- `ready for review`: for issues that are ready for review or feedback
- `done`: for issues that are completed or resolved
- `won't do`: for issues that will not be addressed or completed
- `on hold`: for issues that are on hold or paused

Use the arc42 template for architecture documentation. The arc42 template can be found here: https://arc42.org/ diagrams should be created using d2 or mermaid.js and stored in the `docs/diagrams` directory with a preference for d2. The diagrams should be referenced in the architecture documentation.

use standard markdown for documentation. The documentation should be stored in the `docs` directory.

the structure of a rust library should follow the conventions where crates are all stored in the `crates` directory. Each crate should have its own directory with a `Cargo.toml` file and a `src` directory. The `src` directory should contain the main source code for the crate and tests should be stored in the `tests` directory within the crate directory. The `Cargo.toml` file should contain the metadata for the crate, including the name, version, authors, and dependencies. Unit tests for a file should be at the end of the file.

cli applications should follow the conventions where the main entry point is in the `src/main.rs` file.

cli and binary applications should be stored in the `bin` directory. Each binary should have its own directory with a `Cargo.toml` file and a `src` directory. The `src` directory should contain the main source code for the binary and tests should be stored in the `tests` directory within the binary directory. The `Cargo.toml` file should contain the metadata for the binary, including the name, version, authors, and dependencies.

