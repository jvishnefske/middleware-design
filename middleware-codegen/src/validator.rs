//! JSON-Schema validation of YAML model files.
//!
//! The model schema (`schema/model.schema.json`, JSON Schema draft-07) is
//! embedded into the proc-macro at compile time with `include_str!`, so the
//! macro never looks the schema up on disk while expanding user code.
//!
//! Validation is a pure function: YAML text goes in, and either `Ok(())` or a
//! [`ValidationError`] comes out. The YAML is first parsed into a
//! `serde_yaml::Value`, converted to a `serde_json::Value`, and then checked
//! against the schema with the `jsonschema` crate. Every schema violation is
//! collected (not just the first), each carrying the JSON pointer of the
//! offending instance location (for example `/types/enums/0/repr`) and the
//! validator's message.
//!
//! The schema is compiled without any remote or file reference resolution;
//! only the schema's internal `#/definitions/...` references are used.
//!
//! This check is structural. The semantic checks in
//! `parser::validate_model` still run afterwards, during code generation.
//!
//! # Traceability
//! - REQ-GEN-012: Macro validates YAML against schema and emits compile errors

use std::fmt;

/// The model schema, embedded at compile time.
const MODEL_SCHEMA: &str = include_str!("../../schema/model.schema.json");

/// A single schema violation at one location in the model document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaViolation {
    /// JSON pointer (RFC 6901) to the offending value; empty for the root.
    pub instance_path: String,
    /// Human-readable description of the violation from the schema validator.
    pub message: String,
}

impl fmt::Display for SchemaViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = if self.instance_path.is_empty() {
            "/"
        } else {
            self.instance_path.as_str()
        };
        write!(f, "at `{}`: {}", path, self.message)
    }
}

/// Reasons a model document fails schema validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The input is not valid YAML.
    InvalidYaml(String),
    /// The YAML cannot be represented as JSON (for example a non-string
    /// mapping key), so it cannot be checked against a JSON Schema.
    NotJsonCompatible(String),
    /// The embedded schema itself failed to compile. This indicates a bug in
    /// `schema/model.schema.json`, not in the user's model.
    InvalidSchema(String),
    /// The document parsed but violates the schema. Holds every violation
    /// found, in the order the validator reported them; never empty.
    Violations(Vec<SchemaViolation>),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidYaml(e) => write!(f, "YAML parse error: {}", e),
            Self::NotJsonCompatible(e) => {
                write!(f, "YAML cannot be checked against the JSON schema: {}", e)
            }
            Self::InvalidSchema(e) => write!(f, "embedded model schema is invalid: {}", e),
            Self::Violations(violations) => {
                write!(
                    f,
                    "model violates schema ({} error{}):",
                    violations.len(),
                    if violations.len() == 1 { "" } else { "s" }
                )?;
                for v in violations {
                    write!(f, "\n  - {}", v)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validates YAML model text against the embedded model schema.
///
/// # Returns
/// `Ok(())` if the document conforms to the schema, otherwise a
/// [`ValidationError`]; schema violations are all reported together in
/// [`ValidationError::Violations`].
///
/// # Traceability
/// - REQ-GEN-012: Validates YAML against schema
pub fn validate_yaml(yaml_content: &str) -> Result<(), ValidationError> {
    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content)
        .map_err(|e| ValidationError::InvalidYaml(e.to_string()))?;
    let instance = serde_json::to_value(yaml)
        .map_err(|e| ValidationError::NotJsonCompatible(e.to_string()))?;
    validate_value(&instance)
}

/// Validates an already-parsed JSON value against the embedded model schema.
///
/// # Returns
/// `Ok(())` if the value conforms, otherwise a [`ValidationError`].
///
/// # Traceability
/// - REQ-GEN-012: Validates YAML against schema
pub fn validate_value(instance: &serde_json::Value) -> Result<(), ValidationError> {
    let schema: serde_json::Value = serde_json::from_str(MODEL_SCHEMA)
        .map_err(|e| ValidationError::InvalidSchema(e.to_string()))?;
    let validator = jsonschema::draft7::new(&schema)
        .map_err(|e| ValidationError::InvalidSchema(e.to_string()))?;

    let violations: Vec<SchemaViolation> = validator
        .iter_errors(instance)
        .map(|e| SchemaViolation {
            instance_path: e.instance_path.to_string(),
            message: e.to_string(),
        })
        .collect();

    if violations.is_empty() {
        Ok(())
    } else {
        Err(ValidationError::Violations(violations))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../examples/attitude_controller.yaml");

    fn violations(yaml: &str) -> Vec<SchemaViolation> {
        match validate_yaml(yaml) {
            Err(ValidationError::Violations(v)) => v,
            other => panic!("expected schema violations, got {:?}", other),
        }
    }

    #[test]
    fn embedded_schema_compiles() {
        let schema: serde_json::Value = serde_json::from_str(MODEL_SCHEMA).unwrap();
        assert!(jsonschema::draft7::new(&schema).is_ok());
    }

    #[test]
    fn attitude_controller_example_passes() {
        assert_eq!(validate_yaml(EXAMPLE), Ok(()));
    }

    #[test]
    fn missing_required_field_fails_with_path() {
        let v = violations("version: \"1.0\"\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_path, "");
        assert!(v[0].message.contains("namespace"), "{}", v[0].message);
    }

    #[test]
    fn missing_nested_required_field_fails_with_path() {
        let yaml = r#"
version: "1.0"
namespace: "demo"
ports:
  - name: input_a
    direction: input
"#;
        let v = violations(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_path, "/ports/0");
        assert!(v[0].message.contains("message_type"), "{}", v[0].message);
    }

    #[test]
    fn wrong_type_fails_with_path() {
        let yaml = r#"
version: "1.0"
namespace: "demo"
types:
  enums:
    - name: Mode
      repr: u8
      variants: Idle
"#;
        let v = violations(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_path, "/types/enums/0/variants");
        assert!(v[0].message.contains("array"), "{}", v[0].message);
    }

    #[test]
    fn unknown_enum_value_fails_with_path() {
        let yaml = r#"
version: "1.0"
namespace: "demo"
components:
  - name: Ctrl
    kind: reactive
"#;
        let v = violations(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_path, "/components/0/kind");
        assert!(v[0].message.contains("reactive"), "{}", v[0].message);
    }

    #[test]
    fn all_violations_are_reported() {
        let yaml = r#"
version: 1
components:
  - name: Ctrl
    kind: reactive
"#;
        let v = violations(yaml);
        let paths: Vec<&str> = v.iter().map(|x| x.instance_path.as_str()).collect();
        assert!(paths.contains(&""), "{:?}", paths); // missing namespace
        assert!(paths.contains(&"/version"), "{:?}", paths); // wrong type
        assert!(paths.contains(&"/components/0/kind"), "{:?}", paths); // bad enum
        let rendered = ValidationError::Violations(v).to_string();
        assert!(rendered.contains("/components/0/kind"), "{}", rendered);
    }

    #[test]
    fn invalid_yaml_is_reported() {
        assert!(matches!(
            validate_yaml("version: [unclosed"),
            Err(ValidationError::InvalidYaml(_))
        ));
    }
}
