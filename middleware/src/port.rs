//! Port system for inter-component communication.
//!
//! Ports provide typed channels for message passing between components.
//! They support both synchronous (blocking) and asynchronous (non-blocking)
//! communication patterns.
//!
//! # Modules
//!
//! - `message`: Message serialization trait
//! - `input`: Input port traits (sync and async)
//! - `output`: Output port traits
//!
//! # Traceability
//! - REQ-PRT-001: Port messages implement serialize/deserialize
//! - REQ-PRT-002: Serialization errors returned as Result
//! - REQ-PRT-010: Input ports parameterized by message type
//! - REQ-PRT-020: Output ports parameterized by message type

#![forbid(unsafe_code)]

pub mod message;
pub mod input;
pub mod output;

// Re-export key types
pub use message::{PortMessage, MAX_MESSAGE_SIZE};
pub use input::{InputPort, SyncInputPort, AsyncInputPort};
pub use output::OutputPort;

/// Port direction enumeration.
///
/// Identifies whether a port is an input (receiving) or
/// output (sending) port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PortDirection {
    /// Input port (receives messages)
    Input = 0,

    /// Output port (sends messages)
    Output = 1,
}

impl PortDirection {
    /// Check if this is an input port.
    pub const fn is_input(self) -> bool {
        matches!(self, PortDirection::Input)
    }

    /// Check if this is an output port.
    pub const fn is_output(self) -> bool {
        matches!(self, PortDirection::Output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_direction_values() {
        assert_eq!(PortDirection::Input as u8, 0);
        assert_eq!(PortDirection::Output as u8, 1);
    }

    #[test]
    fn port_direction_is_input() {
        assert!(PortDirection::Input.is_input());
        assert!(!PortDirection::Output.is_input());
    }

    #[test]
    fn port_direction_is_output() {
        assert!(PortDirection::Output.is_output());
        assert!(!PortDirection::Input.is_output());
    }

    #[test]
    fn port_direction_is_copy() {
        let d1 = PortDirection::Input;
        let d2 = d1;
        assert_eq!(d1, d2);
    }
}
