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

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#[cfg(test)]
mod tests {
    use super::*;

    /// Every `MessageType` variant, in wire-value order.
    const ALL_TYPES: [MessageType; 6] = [
        MessageType::ControlTelemetry,
        MessageType::PidTelemetry,
        MessageType::StateMachineTelemetry,
        MessageType::Heartbeat,
        MessageType::Command,
        MessageType::Ack,
    ];

    /// Encodes `header` with minicbor's slice writer into `buf` and returns
    /// the number of bytes written.
    fn encode_header(header: &FrameHeader, buf: &mut [u8]) -> usize {
        let capacity = buf.len();
        let mut encoder = minicbor::Encoder::new(&mut buf[..]);
        encoder
            .encode(header)
            .expect("header fits in scratch buffer");
        capacity - encoder.writer().len()
    }

    /// Verifies: REQ-SER-020
    #[test]
    fn test_message_type_from_u8() {
        assert_eq!(MessageType::from_u8(0), Some(MessageType::ControlTelemetry));
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

    /// Verifies: REQ-SER-020
    #[test]
    fn test_message_type_to_u8() {
        assert_eq!(MessageType::ControlTelemetry.to_u8(), 0);
        assert_eq!(MessageType::PidTelemetry.to_u8(), 1);
        assert_eq!(MessageType::StateMachineTelemetry.to_u8(), 2);
        assert_eq!(MessageType::Heartbeat.to_u8(), 3);
        assert_eq!(MessageType::Command.to_u8(), 4);
        assert_eq!(MessageType::Ack.to_u8(), 5);
    }

    /// Verifies: REQ-SER-020
    #[test]
    fn test_frame_header_new() {
        let header = FrameHeader::new(MessageType::ControlTelemetry, 42, 128);
        assert_eq!(header.msg_type, MessageType::ControlTelemetry);
        assert_eq!(header.sequence, 42);
        assert_eq!(header.payload_len, 128);
    }

    /// Equivalence classes over the whole `u8` domain: exactly the six
    /// defined wire values decode to a type (and round-trip), every other
    /// value, including the boundary 6 and `u8::MAX`, is rejected.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_from_u8_full_domain() {
        for value in 0..=u8::MAX {
            match MessageType::from_u8(value) {
                Some(t) => {
                    assert!(value <= 5);
                    assert_eq!(t.to_u8(), value);
                }
                None => assert!(value > 5),
            }
        }
    }

    /// The `From<MessageType> for u8` conversion yields the same wire value
    /// as `to_u8` for every variant and is inverted by `from_u8`.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_from_message_type_for_u8() {
        for (expected, t) in (0u8..).zip(ALL_TYPES) {
            let wire: u8 = u8::from(t);
            assert_eq!(wire, expected);
            assert_eq!(wire, t.to_u8());
            assert_eq!(MessageType::from_u8(wire), Some(t));
        }
    }

    /// Header round-trip at the boundary values of every field: each message
    /// type combined with sequence 0 / `u16::MAX` and payload length
    /// 0 / `u16::MAX`. All three fields survive encoding unchanged.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_frame_header_round_trip_boundaries() {
        let edges = [0u16, 1, u16::MAX - 1, u16::MAX];
        for msg_type in ALL_TYPES {
            for sequence in edges {
                for payload_len in edges {
                    let header = FrameHeader::new(msg_type, sequence, payload_len);
                    let mut buf = [0u8; 16];
                    let n = encode_header(&header, &mut buf);
                    let bytes = buf.get(..n).expect("n is within buf");
                    let decoded: FrameHeader =
                        minicbor::decode(bytes).expect("encoded header decodes");
                    assert_eq!(decoded.msg_type, msg_type);
                    assert_eq!(decoded.sequence, sequence);
                    assert_eq!(decoded.payload_len, payload_len);
                }
            }
        }
    }

    /// The largest header (sequence and payload length both `u16::MAX`) has
    /// a fixed worst-case size of 8 bytes: array head, one-byte type index
    /// and two 3-byte `u16` values. A buffer of 8 bytes is enough; 7 is not.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_frame_header_max_encoded_size() {
        let header = FrameHeader::new(MessageType::Ack, u16::MAX, u16::MAX);
        let mut exact = [0u8; 8];
        assert_eq!(encode_header(&header, &mut exact), 8);
        let mut short = [0u8; 7];
        let result = minicbor::encode(header, &mut short[..]);
        assert!(result.is_err_and(|e| e.is_write()));
    }

    /// Fault injection: every strict prefix of an encoded header, from the
    /// empty slice up to one byte short, fails to decode instead of yielding
    /// a header with default or partial fields.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_truncated_header_decode_errors() {
        for header in [
            FrameHeader::new(MessageType::ControlTelemetry, 0, 0),
            FrameHeader::new(MessageType::Ack, u16::MAX, u16::MAX),
            FrameHeader::new(MessageType::Heartbeat, 42, 128),
        ] {
            let mut buf = [0u8; 16];
            let n = encode_header(&header, &mut buf);
            for cut in 0..n {
                let prefix = buf.get(..cut).expect("cut < n");
                let result: Result<FrameHeader, _> = minicbor::decode(prefix);
                assert!(result.is_err(), "prefix of {cut}/{n} bytes decoded");
            }
            let full = buf.get(..n).expect("n is within buf");
            assert_eq!(minicbor::decode::<FrameHeader>(full).ok(), Some(header));
        }
    }

    /// Fault injection: a header whose message type index is not a defined
    /// variant (6, the first invalid value) is rejected on decode.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_header_with_unknown_message_type_rejected() {
        // CBOR array(3): [6, 0, 0]
        let bytes = [0x83, 0x06, 0x00, 0x00];
        assert!(minicbor::decode::<FrameHeader>(&bytes).is_err());
        // Same layout with a valid index 5 (Ack) decodes.
        let ok = [0x83, 0x05, 0x00, 0x00];
        assert_eq!(
            minicbor::decode::<FrameHeader>(&ok).ok(),
            Some(FrameHeader::new(MessageType::Ack, 0, 0))
        );
    }

    /// Fault injection: a sequence or payload length that does not fit in
    /// `u16` is rejected rather than silently truncated.
    /// Verifies: REQ-SER-020
    #[test]
    fn test_header_out_of_range_fields_rejected() {
        // [0, 65536, 0]: 0x1a = u32 follows
        let seq_overflow = [0x83, 0x00, 0x1a, 0x00, 0x01, 0x00, 0x00, 0x00];
        assert!(minicbor::decode::<FrameHeader>(&seq_overflow).is_err());
        // [0, 0, 65536]
        let len_overflow = [0x83, 0x00, 0x00, 0x1a, 0x00, 0x01, 0x00, 0x00];
        assert!(minicbor::decode::<FrameHeader>(&len_overflow).is_err());
    }

    /// The header encodes as a CBOR array of exactly three items, in field
    /// order message type, sequence, payload length: `[0, 42, 128]` is
    /// `83 00 18 2a 18 80`.
    /// Verifies: REQ-SER-001, REQ-SER-020
    #[test]
    fn test_frame_header_exact_cbor_bytes() {
        let header = FrameHeader::new(MessageType::ControlTelemetry, 42, 128);
        let mut buf = [0u8; 16];
        let n = encode_header(&header, &mut buf);
        assert_eq!(
            buf.get(..n),
            Some(&[0x83, 0x00, 0x18, 0x2a, 0x18, 0x80][..])
        );
    }
}
