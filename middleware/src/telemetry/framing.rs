//! Frame header and message type definitions for telemetry.
//!
//! This module defines the framing protocol for telemetry messages,
//! including message type identifiers and frame headers.
//!
//! # Traceability
//!
//! - REQ-SER-020: Telemetry frames include message type, sequence number, payload length

use minicbor::{Decode, Encode};

/// Message type identifier for telemetry frames.
///
/// This enum represents the different types of messages that can be
/// transmitted in the telemetry system. Each variant maps to a specific
/// message structure.
///
/// # Traceability
///
/// - REQ-SER-020: Frame message type field
///
/// # Representation
///
/// Uses `#[repr(u8)]` to ensure compact encoding and compatibility with
/// low-level protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
#[repr(u8)]
#[cbor(index_only)]
pub enum MessageType {
    /// Control telemetry message (ControlTelemetry)
    #[n(0)]
    ControlTelemetry = 0,
    /// PID telemetry message (PidTelemetry)
    #[n(1)]
    PidTelemetry = 1,
    /// State machine telemetry message (StateMachineTelemetry)
    #[n(2)]
    StateMachineTelemetry = 2,
    /// Heartbeat keepalive message
    #[n(3)]
    Heartbeat = 3,
    /// Command message
    #[n(4)]
    Command = 4,
    /// Acknowledgment message
    #[n(5)]
    Ack = 5,
}

impl MessageType {
    /// Converts a u8 value to a MessageType.
    ///
    /// # Arguments
    ///
    /// - `value`: The u8 value to convert
    ///
    /// # Returns
    ///
    /// - `Some(MessageType)` if the value is valid
    /// - `None` if the value does not correspond to any MessageType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(MessageType::ControlTelemetry),
            1 => Some(MessageType::PidTelemetry),
            2 => Some(MessageType::StateMachineTelemetry),
            3 => Some(MessageType::Heartbeat),
            4 => Some(MessageType::Command),
            5 => Some(MessageType::Ack),
            _ => None,
        }
    }

    /// Converts the MessageType to its u8 representation.
    ///
    /// # Returns
    ///
    /// The u8 value corresponding to this MessageType. This equals the
    /// `#[repr(u8)]` discriminant and is the inverse of
    /// [`from_u8`](Self::from_u8).
    pub fn to_u8(self) -> u8 {
        match self {
            MessageType::ControlTelemetry => 0,
            MessageType::PidTelemetry => 1,
            MessageType::StateMachineTelemetry => 2,
            MessageType::Heartbeat => 3,
            MessageType::Command => 4,
            MessageType::Ack => 5,
        }
    }
}

impl From<MessageType> for u8 {
    /// Converts a [`MessageType`] to its wire value; same as
    /// [`MessageType::to_u8`].
    fn from(value: MessageType) -> Self {
        value.to_u8()
    }
}

/// Frame header for telemetry messages.
///
/// This structure provides metadata about the telemetry frame, including
/// the type of message, sequence number for ordering, and payload length
/// for parsing.
///
/// # Traceability
///
/// - REQ-SER-020: Frame header with message type, sequence, and payload length
///
/// # Fields
///
/// - `msg_type`: Type of message contained in this frame
/// - `sequence`: Monotonic sequence number for message ordering
/// - `payload_len`: Length of the CBOR-encoded payload in bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct FrameHeader {
    #[n(0)]
    pub msg_type: MessageType,
    #[n(1)]
    pub sequence: u16,
    #[n(2)]
    pub payload_len: u16,
}

impl FrameHeader {
    /// Creates a new FrameHeader.
    ///
    /// # Arguments
    ///
    /// - `msg_type`: Type of message
    /// - `sequence`: Sequence number
    /// - `payload_len`: Payload length in bytes
    ///
    /// # Returns
    ///
    /// A new FrameHeader instance with the specified values.
    pub fn new(msg_type: MessageType, sequence: u16, payload_len: u16) -> Self {
        Self {
            msg_type,
            sequence,
            payload_len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_from_u8() {
        assert_eq!(
            MessageType::from_u8(0),
            Some(MessageType::ControlTelemetry)
        );
        assert_eq!(MessageType::from_u8(1), Some(MessageType::PidTelemetry));
        assert_eq!(
            MessageType::from_u8(2),
            Some(MessageType::StateMachineTelemetry)
        );
        assert_eq!(MessageType::from_u8(3), Some(MessageType::Heartbeat));
        assert_eq!(MessageType::from_u8(4), Some(MessageType::Command));
        assert_eq!(MessageType::from_u8(5), Some(MessageType::Ack));
        assert_eq!(MessageType::from_u8(6), None);
    }

    #[test]
    fn test_message_type_to_u8() {
        assert_eq!(MessageType::ControlTelemetry.to_u8(), 0);
        assert_eq!(MessageType::PidTelemetry.to_u8(), 1);
        assert_eq!(MessageType::StateMachineTelemetry.to_u8(), 2);
        assert_eq!(MessageType::Heartbeat.to_u8(), 3);
        assert_eq!(MessageType::Command.to_u8(), 4);
        assert_eq!(MessageType::Ack.to_u8(), 5);
    }

    #[test]
    fn test_frame_header_new() {
        let header = FrameHeader::new(MessageType::ControlTelemetry, 42, 128);
        assert_eq!(header.msg_type, MessageType::ControlTelemetry);
        assert_eq!(header.sequence, 42);
        assert_eq!(header.payload_len, 128);
    }
}
