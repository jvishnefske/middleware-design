//! Telemetry module for CBOR-encoded messages.
//!
//! This module provides telemetry message types, framing, and fixed-size buffers
//! for no_std embedded environments. All messages are serialized using CBOR
//! (Concise Binary Object Representation) via the minicbor crate.
//!
//! # Traceability
//!
//! - REQ-SER-001: CBOR binary serialization
//! - REQ-SER-002: no_std compatible implementation
//! - REQ-SER-003: Derive Encode and Decode for all message types

pub mod buffer;
pub mod framing;
pub mod messages;

pub use buffer::TelemetryBuffer;
pub use framing::{FrameHeader, MessageType};
pub use messages::{ControlTelemetry, PidTelemetry, StateMachineTelemetry};
