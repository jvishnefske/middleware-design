//! Model types for YAML deserialization.
//!
//! These types correspond to the JSON schema in schema/model.schema.json.
//!
//! # Traceability
//! - REQ-GEN-001: YAML schema defines component structure
//! - REQ-GEN-002: YAML schema defines state machine structure
//! - REQ-GEN-003: YAML schema defines math block configuration
//! - REQ-GEN-004: All YAML elements support traces field

use serde::Deserialize;

/// Root model structure.
///
/// # Traceability
/// - REQ-GEN-001: Component structure
/// - REQ-GEN-004: Traceability support
#[derive(Debug, Deserialize)]
pub struct Model {
    /// Schema version.
    pub version: String,
    /// Namespace for generated code.
    pub namespace: String,
    /// Requirements for traceability.
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    /// Type definitions.
    #[serde(default)]
    pub types: Types,
    /// Message definitions.
    #[serde(default)]
    pub messages: Vec<MessageDef>,
    /// Port definitions; components refer to them by name, and each
    /// `message_type` gets a generated `PortMessage` implementation.
    #[serde(default)]
    pub ports: Vec<PortDef>,
    /// Component definitions.
    #[serde(default)]
    pub components: Vec<Component>,
    /// Math block definitions.
    #[serde(default)]
    pub math_blocks: Vec<MathBlock>,
    /// State machine definitions.
    #[serde(default)]
    pub state_machines: Vec<StateMachine>,
}

/// Requirement definition for traceability.
///
/// # Traceability
/// - REQ-GEN-004: Traceability support
#[derive(Debug, Deserialize)]
pub struct Requirement {
    /// Requirement identifier (e.g., REQ-ATT-001).
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// DO-178C level (A, B, C, D).
    pub level: String,
}

/// Type definitions container.
#[derive(Debug, Default, Deserialize)]
pub struct Types {
    /// Enum definitions.
    #[serde(default)]
    pub enums: Vec<EnumDef>,
    /// Struct definitions.
    #[serde(default)]
    pub structs: Vec<StructDef>,
}

/// Enum type definition.
///
/// # Traceability
/// - REQ-FSM-001: States as enums with repr
#[derive(Debug, Deserialize)]
pub struct EnumDef {
    /// Enum name (PascalCase).
    pub name: String,
    /// Traceability to requirements.
    #[serde(default)]
    pub traces: Vec<String>,
    /// Integer representation (u8, u16, etc.).
    pub repr: String,
    /// Variant names.
    pub variants: Vec<String>,
}

/// Struct type definition.
#[derive(Debug, Deserialize)]
pub struct StructDef {
    /// Struct name (PascalCase).
    pub name: String,
    /// Traceability to requirements.
    #[serde(default)]
    pub traces: Vec<String>,
    /// Fields.
    pub fields: Vec<Field>,
}

/// Struct or message field.
#[derive(Debug, Deserialize)]
pub struct Field {
    /// Field name (snake_case).
    pub name: String,
    /// Rust type.
    #[serde(rename = "type")]
    pub field_type: String,
}

/// Message type definition.
#[derive(Debug, Deserialize)]
pub struct MessageDef {
    /// Message name (PascalCase).
    pub name: String,
    /// Traceability to requirements.
    #[serde(default)]
    pub traces: Vec<String>,
    /// Fields.
    pub fields: Vec<Field>,
}

/// Port definition.
///
/// # Traceability
/// - REQ-PRT-010: Input ports parameterized by message type
/// - REQ-PRT-020: Output ports parameterized by message type
#[derive(Debug, Deserialize)]
pub struct PortDef {
    /// Port name (snake_case).
    pub name: String,
    /// Direction: "input" or "output".
    pub direction: String,
    /// Message type name: a model message or struct with primitive fields.
    pub message_type: String,
}

/// Component definition.
///
/// Generates a component struct with config, state and typed ports, a
/// commands enum, and an `impl middleware::Component`.
///
/// # Traceability
/// - REQ-GEN-001: Component structure
/// - REQ-GEN-011: Generated component structs and implementations
#[derive(Debug, Deserialize)]
pub struct Component {
    /// Component name (PascalCase).
    pub name: String,
    /// Traceability to requirements.
    #[serde(default)]
    pub traces: Vec<String>,
    /// Component kind: "passive" or "active".
    pub kind: String,
    /// State type name. Must equal the linked state machine's `state_type`;
    /// without a state machine it must be a model enum.
    #[serde(default)]
    pub state: Option<String>,
    /// Input port names.
    #[serde(default)]
    pub input_ports: Vec<String>,
    /// Output port names.
    #[serde(default)]
    pub output_ports: Vec<String>,
    /// Command names (snake_case or PascalCase), generated as enum variants.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Associated state machine name; drives the component's `handle` method.
    #[serde(default)]
    pub state_machine: Option<String>,
}

/// Math block definition.
///
/// # Traceability
/// - REQ-GEN-003: Math block configuration
#[derive(Debug, Deserialize)]
pub struct MathBlock {
    /// Block name (snake_case).
    pub name: String,
    /// Traceability to requirements.
    #[serde(default)]
    pub traces: Vec<String>,
    /// Block kind: pid, low_pass, high_pass, notch, state_space.
    pub kind: String,
    /// Configuration parameters.
    pub config: MathBlockConfig,
}

/// Math block configuration.
///
/// PID blocks use the gain and limit fields; `integral_min` defaults to
/// `-integral_max` when unset. Filter blocks use the frequency fields.
#[derive(Debug, Deserialize)]
pub struct MathBlockConfig {
    // PID parameters
    #[serde(default)]
    pub kp: Option<f32>,
    #[serde(default)]
    pub ki: Option<f32>,
    #[serde(default)]
    pub kd: Option<f32>,
    #[serde(default)]
    pub output_min: Option<f32>,
    #[serde(default)]
    pub output_max: Option<f32>,
    #[serde(default)]
    pub integral_min: Option<f32>,
    #[serde(default)]
    pub integral_max: Option<f32>,
    // Filter parameters
    #[serde(default)]
    pub cutoff_hz: Option<f32>,
    #[serde(default)]
    pub sample_rate_hz: Option<f32>,
    #[serde(default)]
    pub bandwidth_hz: Option<f32>,
    #[serde(default)]
    pub notch_hz: Option<f32>,
}

/// State machine definition.
///
/// # Traceability
/// - REQ-GEN-002: State machine structure
#[derive(Debug, Deserialize)]
pub struct StateMachine {
    /// State machine name (PascalCase).
    pub name: String,
    /// Traceability to requirements.
    #[serde(default)]
    pub traces: Vec<String>,
    /// State enum type name.
    pub state_type: String,
    /// Initial state variant name.
    pub initial_state: String,
    /// Event enum type name.
    pub events: String,
    /// State definitions.
    pub states: Vec<StateDef>,
    /// Transition definitions.
    pub transitions: Vec<TransitionDef>,
}

/// State definition.
///
/// # Traceability
/// - REQ-FSM-002: State lifecycle actions
#[derive(Debug, Deserialize)]
pub struct StateDef {
    /// State variant name.
    pub name: String,
    /// On-enter action name (snake_case), a method of the generated `Hooks` trait.
    #[serde(default)]
    pub on_enter: Option<String>,
    /// On-exit action name (snake_case), a method of the generated `Hooks` trait.
    #[serde(default)]
    pub on_exit: Option<String>,
}

/// Transition definition.
///
/// # Traceability
/// - REQ-FSM-010: Transition specification
/// - REQ-FSM-011: Guard conditions
/// - REQ-FSM-012: Transition actions
#[derive(Debug, Deserialize)]
pub struct TransitionDef {
    /// Source state variant name.
    pub from: String,
    /// Event variant name.
    pub event: String,
    /// Target state variant name.
    pub to: String,
    /// Guard condition name (snake_case); a `&self -> bool` method of the
    /// generated `Hooks` trait. The row fires only if it returns `true`.
    #[serde(default)]
    pub guard: Option<String>,
    /// Transition action name (snake_case); a `&mut self` method of the
    /// generated `Hooks` trait, run between exit and enter.
    #[serde(default)]
    pub action: Option<String>,
}
