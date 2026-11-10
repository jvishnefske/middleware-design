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
//! The schema is closed: every object, at the top level and nested, sets
//! `additionalProperties: false`, so an unknown or misspelled key (for
//! example `gaurd:` for `guard:`) is a violation rather than being ignored.
//!
//! This check is structural. The semantic checks in
//! `parser::validate_model` still run afterwards, during code generation.
//!
//! # Traceability
//! - REQ-GEN-012: Macro validates YAML against schema and emits compile errors
//! - REQ-GEN-014: The closed schema rejects unknown keys in every object

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
// test code: a panic is a test failure, the intended reporting mechanism
#[cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)
)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../examples/attitude_controller.yaml");

    fn violations(yaml: &str) -> Vec<SchemaViolation> {
        match validate_yaml(yaml) {
            Err(ValidationError::Violations(v)) => v,
            other => panic!("expected schema violations, got {:?}", other),
        }
    }

    /// Verifies: REQ-GEN-012
    #[test]
    fn embedded_schema_compiles() {
        let schema: serde_json::Value = serde_json::from_str(MODEL_SCHEMA).unwrap();
        assert!(jsonschema::draft7::new(&schema).is_ok());
    }

    /// Verifies: REQ-GEN-012
    #[test]
    fn attitude_controller_example_passes() {
        assert_eq!(validate_yaml(EXAMPLE), Ok(()));
    }

    /// Verifies: REQ-GEN-012
    #[test]
    fn missing_required_field_fails_with_path() {
        let v = violations("version: \"1.0\"\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_path, "");
        assert!(v[0].message.contains("namespace"), "{}", v[0].message);
    }

    /// Verifies: REQ-GEN-012
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

    /// Verifies: REQ-GEN-012
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

    /// Verifies: REQ-GEN-012
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

    /// Verifies: REQ-GEN-012
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

    /// Verifies: REQ-GEN-012, REQ-GEN-014
    #[test]
    fn unknown_key_is_rejected_with_path() {
        let yaml = r#"
version: "1.0"
namespace: "demo"
types:
  enums:
    - name: S
      repr: u8
      variants: [A, B]
    - name: E
      repr: u8
      variants: [Go]
state_machines:
  - name: M
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
    transitions:
      - from: A
        event: Go
        to: B
        gaurd: ready
"#;
        let v = violations(yaml);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].instance_path, "/state_machines/0/transitions/0");
        assert!(v[0].message.contains("gaurd"), "{}", v[0].message);
    }

    /// Verifies: REQ-GEN-012, REQ-GEN-014
    #[test]
    fn unknown_top_level_and_config_keys_are_rejected() {
        let yaml = r#"
version: "1.0"
namespace: "demo"
extra: true
math_blocks:
  - name: p
    kind: pid
    config:
      kp: 1.0
      k_p: 2.0
"#;
        let v = violations(yaml);
        let mut paths: Vec<&str> = v.iter().map(|x| x.instance_path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(paths, vec!["", "/math_blocks/0/config"], "{:?}", v);
        assert!(v.iter().any(|x| x.message.contains("'extra'")), "{:?}", v);
        assert!(v.iter().any(|x| x.message.contains("'k_p'")), "{:?}", v);
    }

    /// Verifies: REQ-GEN-012
    #[test]
    fn invalid_yaml_is_reported() {
        assert!(matches!(
            validate_yaml("version: [unclosed"),
            Err(ValidationError::InvalidYaml(_))
        ));
    }

    /// A model using every object kind the schema defines, each with every
    /// optional key it accepts, so injected keys land in every object.
    const EVERY_OBJECT: &str = r#"
version: "1.0"
namespace: "demo"
requirements:
  - id: REQ-DEMO-1
    description: d
    level: B
types:
  enums:
    - name: S
      traces: [REQ-DEMO-1]
      repr: u8
      variants: [A, B]
    - name: E
      repr: u8
      variants: [Go]
  structs:
    - name: P
      traces: [REQ-DEMO-1]
      fields:
        - name: x
          type: f32
messages:
  - name: M
    traces: [REQ-DEMO-1]
    fields:
      - name: v
        type: u8
ports:
  - name: in_m
    direction: input
    message_type: M
components:
  - name: C
    traces: [REQ-DEMO-1]
    kind: active
    state: S
    input_ports: [in_m]
    output_ports: []
    commands: [arm]
    state_machine: F
math_blocks:
  - name: p
    traces: [REQ-DEMO-1]
    kind: pid
    config:
      kp: 1.0
      ki: 0.0
      kd: 0.0
      output_min: -1.0
      output_max: 1.0
      integral_min: -1.0
      integral_max: 1.0
      cutoff_hz: 1.0
      sample_rate_hz: 1.0
      bandwidth_hz: 1.0
      notch_hz: 1.0
state_machines:
  - name: F
    traces: [REQ-DEMO-1]
    state_type: S
    initial_state: A
    events: E
    states:
      - name: A
        on_enter: arrive
        on_exit: leave
    transitions:
      - from: A
        event: Go
        to: B
        guard: ready
        action: act
"#;

    fn every_object() -> serde_json::Value {
        serde_json::to_value(serde_yaml::from_str::<serde_yaml::Value>(EVERY_OBJECT).unwrap())
            .unwrap()
    }

    /// JSON pointers of every object in `value`, root first.
    fn object_pointers(value: &serde_json::Value, at: &str, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                out.push(at.to_string());
                for (k, v) in map {
                    object_pointers(v, &format!("{}/{}", at, k), out);
                }
            }
            serde_json::Value::Array(items) => {
                for (i, v) in items.iter().enumerate() {
                    object_pointers(v, &format!("{}/{}", at, i), out);
                }
            }
            _ => {}
        }
    }

    /// Every object the schema defines, at the root, inline or under
    /// `definitions`, sets `additionalProperties: false`.
    /// Verifies: REQ-GEN-014
    #[test]
    fn every_schema_object_is_closed() {
        fn walk(schema: &serde_json::Value, at: &str, open: &mut Vec<String>, seen: &mut usize) {
            if let serde_json::Value::Object(map) = schema {
                if map.get("type") == Some(&serde_json::Value::from("object")) {
                    *seen += 1;
                    if map.get("additionalProperties") != Some(&serde_json::Value::Bool(false)) {
                        open.push(at.to_string());
                    }
                }
                for (k, v) in map {
                    walk(v, &format!("{}/{}", at, k), open, seen);
                }
            }
        }
        let schema: serde_json::Value = serde_json::from_str(MODEL_SCHEMA).unwrap();
        let mut open = Vec::new();
        let mut seen = 0;
        walk(&schema, "", &mut open, &mut seen);
        assert!(
            open.is_empty(),
            "objects without additionalProperties: false: {:?}",
            open
        );
        // root, types, math block config and the 11 object definitions
        assert_eq!(seen, 14);
    }

    /// For every object in a model that uses every object kind, adding one
    /// unknown key yields exactly one violation, at that object, naming the
    /// key; without it the model is valid.
    /// Verifies: REQ-GEN-014
    #[test]
    fn unknown_key_is_rejected_in_every_object() {
        let base = every_object();
        assert_eq!(validate_value(&base), Ok(()));
        let mut pointers = Vec::new();
        object_pointers(&base, "", &mut pointers);
        // root, requirement, types, 2 enums, struct, field, message, field,
        // port, component, math block, config, state machine, state, transition
        assert_eq!(pointers.len(), 16, "{:?}", pointers);
        for pointer in &pointers {
            let mut doc = base.clone();
            let target = doc.pointer_mut(pointer).unwrap().as_object_mut().unwrap();
            target.insert("zz_unknown".to_string(), serde_json::Value::Bool(true));
            match validate_value(&doc) {
                Err(ValidationError::Violations(v)) => {
                    assert_eq!(v.len(), 1, "{}: {:?}", pointer, v);
                    assert_eq!(&v[0].instance_path, pointer);
                    assert!(
                        v[0].message.contains("zz_unknown"),
                        "{}: {}",
                        pointer,
                        v[0].message
                    );
                }
                other => panic!("{}: expected one violation, got {:?}", pointer, other),
            }
        }
    }

    /// The schema defines the component structure: the full component is
    /// accepted, and `name` and `kind` are required.
    /// Verifies: REQ-GEN-001
    #[test]
    fn schema_defines_component_structure() {
        let base = every_object();
        for key in ["name", "kind"] {
            let mut doc = base.clone();
            doc.pointer_mut("/components/0")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let v = match validate_value(&doc) {
                Err(ValidationError::Violations(v)) => v,
                other => panic!("{}: {:?}", key, other),
            };
            assert_eq!(v.len(), 1, "{:?}", v);
            assert_eq!(v[0].instance_path, "/components/0");
            assert!(v[0].message.contains(key), "{}", v[0].message);
        }
        let yaml = "version: \"1.0\"\nnamespace: d\ncomponents:\n  - name: C\n    kind: passive\n    commands: [Bad-Name]\n";
        let v = violations(yaml);
        assert_eq!(v[0].instance_path, "/components/0/commands/0");
    }

    /// The schema defines state machine structure: each required key is
    /// enforced and hook names must be snake_case.
    /// Verifies: REQ-GEN-002
    #[test]
    fn schema_defines_state_machine_structure() {
        let base = every_object();
        for key in [
            "name",
            "state_type",
            "initial_state",
            "events",
            "states",
            "transitions",
        ] {
            let mut doc = base.clone();
            doc.pointer_mut("/state_machines/0")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            let v = match validate_value(&doc) {
                Err(ValidationError::Violations(v)) => v,
                other => panic!("{}: {:?}", key, other),
            };
            assert_eq!(v.len(), 1, "{:?}", v);
            assert_eq!(v[0].instance_path, "/state_machines/0");
            assert!(v[0].message.contains(key), "{}", v[0].message);
        }
        for pointer in [
            "/state_machines/0/states/0/on_enter",
            "/state_machines/0/states/0/on_exit",
            "/state_machines/0/transitions/0/guard",
            "/state_machines/0/transitions/0/action",
        ] {
            let mut doc = base.clone();
            *doc.pointer_mut(pointer).unwrap() = serde_json::Value::from("NotSnake");
            let v = match validate_value(&doc) {
                Err(ValidationError::Violations(v)) => v,
                other => panic!("{}: {:?}", pointer, other),
            };
            assert_eq!(v.len(), 1, "{:?}", v);
            assert_eq!(v[0].instance_path, pointer);
        }
        let mut doc = base.clone();
        *doc.pointer_mut("/state_machines/0/states").unwrap() = serde_json::json!([]);
        assert!(
            validate_value(&doc).is_err(),
            "a state machine needs at least one state"
        );
    }

    /// The schema defines math block configuration: every kind is accepted,
    /// config values must be numbers, and `config` is required.
    /// Verifies: REQ-GEN-003
    #[test]
    fn schema_defines_math_block_configuration() {
        let base = every_object();
        for kind in ["pid", "low_pass", "high_pass", "notch", "state_space"] {
            let mut doc = base.clone();
            *doc.pointer_mut("/math_blocks/0/kind").unwrap() = serde_json::Value::from(kind);
            assert_eq!(validate_value(&doc), Ok(()), "{}", kind);
        }
        let mut doc = base.clone();
        *doc.pointer_mut("/math_blocks/0/config/kp").unwrap() = serde_json::Value::from("high");
        let v = match validate_value(&doc) {
            Err(ValidationError::Violations(v)) => v,
            other => panic!("{:?}", other),
        };
        assert_eq!(v[0].instance_path, "/math_blocks/0/config/kp");
        let mut doc = base.clone();
        doc.pointer_mut("/math_blocks/0")
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("config");
        let v = match validate_value(&doc) {
            Err(ValidationError::Violations(v)) => v,
            other => panic!("{:?}", other),
        };
        assert!(v[0].message.contains("config"), "{}", v[0].message);
    }

    /// Every element kind that takes `traces` accepts a list of requirement
    /// IDs and rejects a malformed ID.
    /// Verifies: REQ-GEN-004
    #[test]
    fn schema_accepts_traces_on_every_element_kind() {
        let base = every_object();
        let kinds = [
            "/types/enums/0",
            "/types/structs/0",
            "/messages/0",
            "/components/0",
            "/math_blocks/0",
            "/state_machines/0",
        ];
        for pointer in kinds {
            let traces = base.pointer(&format!("{}/traces", pointer)).unwrap();
            assert_eq!(traces, &serde_json::json!(["REQ-DEMO-1"]), "{}", pointer);
            let mut doc = base.clone();
            *doc.pointer_mut(&format!("{}/traces", pointer)).unwrap() =
                serde_json::json!(["REQ-DEMO-1", "not-a-req"]);
            let v = match validate_value(&doc) {
                Err(ValidationError::Violations(v)) => v,
                other => panic!("{}: {:?}", pointer, other),
            };
            assert_eq!(v.len(), 1, "{:?}", v);
            assert_eq!(v[0].instance_path, format!("{}/traces/1", pointer));
        }
    }

    /// Error guessing: a mapping key that JSON cannot represent.
    /// Verifies: REQ-GEN-012
    #[test]
    fn non_json_compatible_yaml_is_reported() {
        let err = validate_yaml("version: \"1.0\"\nnamespace: t\n? [a, b]\n: c\n").unwrap_err();
        assert!(
            matches!(err, ValidationError::NotJsonCompatible(_)),
            "{:?}",
            err
        );
        assert!(
            err.to_string().starts_with("YAML cannot be checked"),
            "{}",
            err
        );
    }

    #[test]
    fn error_messages_render_each_variant() {
        let one = ValidationError::Violations(vec![SchemaViolation {
            instance_path: String::new(),
            message: "m".to_string(),
        }]);
        assert_eq!(
            one.to_string(),
            "model violates schema (1 error):\n  - at `/`: m"
        );
        let two = ValidationError::Violations(vec![
            SchemaViolation {
                instance_path: "/a".to_string(),
                message: "x".to_string(),
            },
            SchemaViolation {
                instance_path: "/b".to_string(),
                message: "y".to_string(),
            },
        ]);
        assert_eq!(
            two.to_string(),
            "model violates schema (2 errors):\n  - at `/a`: x\n  - at `/b`: y"
        );
        assert_eq!(
            ValidationError::InvalidSchema("bad".to_string()).to_string(),
            "embedded model schema is invalid: bad"
        );
        assert!(ValidationError::InvalidYaml("e".to_string())
            .to_string()
            .starts_with("YAML parse error"));
    }
}
