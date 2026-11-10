//! YAML parser for model files.
//!
//! # Traceability
//! - REQ-GEN-001: YAML schema defines component structure
//! - REQ-GEN-012: Validate YAML against schema

use crate::model::Model;

/// Parses YAML content into a Model.
///
/// # Parameters
/// - `yaml_content`: The YAML content to parse
///
/// # Returns
/// `Ok(Model)` on success, `Err(String)` with error message on failure
///
/// # Traceability
/// - REQ-GEN-012: Validates YAML structure
pub fn parse_model(yaml_content: &str) -> Result<Model, String> {
    serde_yaml::from_str(yaml_content).map_err(|e| format!("YAML parse error: {}", e))
}

/// Validates a parsed model against schema constraints.
///
/// # Parameters
/// - `model`: The model to validate
///
/// # Returns
/// `Ok(())` if valid, `Err(String)` with validation errors
///
/// # Traceability
/// - REQ-GEN-012: Validates YAML against schema
pub fn validate_model(model: &Model) -> Result<(), String> {
    // Validate version format
    if !model.version.contains('.') {
        return Err("Invalid version format, expected 'X.Y'".to_string());
    }

    // Validate namespace format
    if model.namespace.is_empty() {
        return Err("Namespace cannot be empty".to_string());
    }
    if !first_char_is(&model.namespace, |c| c.is_ascii_lowercase()) {
        return Err("Namespace must start with lowercase letter".to_string());
    }

    // Validate enum names are PascalCase
    for enum_def in &model.types.enums {
        if !first_char_is(&enum_def.name, |c| c.is_ascii_uppercase()) {
            return Err(format!("Enum '{}' must be PascalCase", enum_def.name));
        }
        if enum_def.variants.is_empty() {
            return Err(format!("Enum '{}' must have at least one variant", enum_def.name));
        }
        // Validate repr
        let valid_reprs = ["u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64"];
        if !valid_reprs.contains(&enum_def.repr.as_str()) {
            return Err(format!("Invalid repr '{}' for enum '{}'", enum_def.repr, enum_def.name));
        }
    }

    // Validate struct names
    for struct_def in &model.types.structs {
        if !first_char_is(&struct_def.name, |c| c.is_ascii_uppercase()) {
            return Err(format!("Struct '{}' must be PascalCase", struct_def.name));
        }
        if struct_def.fields.is_empty() {
            return Err(format!("Struct '{}' must have at least one field", struct_def.name));
        }
    }

    // Validate message names
    for msg in &model.messages {
        if !first_char_is(&msg.name, |c| c.is_ascii_uppercase()) {
            return Err(format!("Message '{}' must be PascalCase", msg.name));
        }
    }

    // Validate component names
    for comp in &model.components {
        if !first_char_is(&comp.name, |c| c.is_ascii_uppercase()) {
            return Err(format!("Component '{}' must be PascalCase", comp.name));
        }
        if comp.kind != "passive" && comp.kind != "active" {
            return Err(format!("Component '{}' kind must be 'passive' or 'active'", comp.name));
        }
    }

    // Validate math block kinds
    for block in &model.math_blocks {
        let valid_kinds = ["pid", "low_pass", "high_pass", "notch", "state_space"];
        if !valid_kinds.contains(&block.kind.as_str()) {
            return Err(format!("Invalid math block kind '{}' for '{}'", block.kind, block.name));
        }
    }

    // Validate state machines
    for fsm in &model.state_machines {
        if !first_char_is(&fsm.name, |c| c.is_ascii_uppercase()) {
            return Err(format!("State machine '{}' must be PascalCase", fsm.name));
        }
        if fsm.states.is_empty() {
            return Err(format!("State machine '{}' must have at least one state", fsm.name));
        }
        // Check initial state exists
        if !fsm.states.iter().any(|s| s.name == fsm.initial_state) {
            return Err(format!(
                "Initial state '{}' not found in state machine '{}'",
                fsm.initial_state, fsm.name
            ));
        }
    }

    // Validate requirement ID format
    for req in &model.requirements {
        if !req.id.starts_with("REQ-") {
            return Err(format!("Requirement ID '{}' must start with 'REQ-'", req.id));
        }
    }

    Ok(())
}

/// Returns `true` if `name` is non-empty and its first character satisfies
/// `pred`. An empty name never satisfies a naming rule.
fn first_char_is(name: &str, pred: impl Fn(char) -> bool) -> bool {
    name.chars().next().is_some_and(pred)
}

#[cfg(test)]
// test code: a panic is a test failure, the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::indexing_slicing))]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_model() {
        let yaml = r#"
version: "1.0"
namespace: "test"
"#;
        let model = parse_model(yaml).unwrap();
        assert_eq!(model.version, "1.0");
        assert_eq!(model.namespace, "test");
    }

    #[test]
    fn test_parse_with_enums() {
        let yaml = r#"
version: "1.0"
namespace: "test"
types:
  enums:
    - name: MyState
      repr: u8
      variants:
        - Idle
        - Running
"#;
        let model = parse_model(yaml).unwrap();
        assert_eq!(model.types.enums.len(), 1);
        assert_eq!(model.types.enums[0].name, "MyState");
        assert_eq!(model.types.enums[0].variants.len(), 2);
    }

    #[test]
    fn test_parse_rejects_value_of_wrong_model_type() {
        // The schema rejects this first; the parser error is defence in depth.
        let yaml = "version: \"1.0\"\nnamespace: t\nmath_blocks:\n  - name: p\n    kind: pid\n    config:\n      kp: high\n";
        let err = parse_model(yaml).unwrap_err();
        assert!(err.starts_with("YAML parse error"), "{}", err);
    }

    #[test]
    fn test_validate_empty_name_is_not_pascal_case() {
        let yaml = r#"
version: "1.0"
namespace: "test"
messages:
  - name: ""
    fields:
      - name: v
        type: u8
"#;
        let model = parse_model(yaml).unwrap();
        assert_eq!(
            validate_model(&model),
            Err("Message '' must be PascalCase".to_string())
        );
    }

    #[test]
    fn test_validate_invalid_namespace() {
        let yaml = r#"
version: "1.0"
namespace: "Test"
"#;
        let model = parse_model(yaml).unwrap();
        let result = validate_model(&model);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_enum_name() {
        let yaml = r#"
version: "1.0"
namespace: "test"
types:
  enums:
    - name: myState
      repr: u8
      variants:
        - Idle
"#;
        let model = parse_model(yaml).unwrap();
        let result = validate_model(&model);
        assert!(result.is_err());
    }
}
