//! Integration tests for telemetry encoding and decoding.
//!
//! These tests verify end-to-end serialization and deserialization of
//! telemetry messages using CBOR encoding.

use middleware::telemetry::{
    ControlTelemetry, FrameHeader, MessageType, PidTelemetry, StateMachineTelemetry,
    TelemetryBuffer,
};
use minicbor::decode;

#[test]
fn test_control_telemetry_encode_decode() {
    // Create a telemetry message
    let original = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

    // Encode to buffer
    let mut buffer = TelemetryBuffer::<128>::new();
    buffer.encode(&original).expect("Failed to encode");

    // Verify buffer has data
    assert!(!buffer.is_empty());
    let bytes = buffer.as_bytes();
    assert!(bytes.len() > 0);

    // Decode from bytes
    let decoded: ControlTelemetry = decode(bytes).expect("Failed to decode");

    // Verify data integrity
    assert_eq!(decoded.timestamp_ms, original.timestamp_ms);
    assert_eq!(decoded.setpoint, original.setpoint);
    assert_eq!(decoded.measurement, original.measurement);
    assert_eq!(decoded.output, original.output);
    assert_eq!(decoded.error, original.error);
}

#[test]
fn test_pid_telemetry_encode_decode() {
    // Create a PID telemetry message
    let original = PidTelemetry::new(1.0, 0.5, 0.2, false);

    // Encode to buffer
    let mut buffer = TelemetryBuffer::<128>::new();
    buffer.encode(&original).expect("Failed to encode");

    // Decode
    let decoded: PidTelemetry = decode(buffer.as_bytes()).expect("Failed to decode");

    // Verify
    assert_eq!(decoded.p_term, original.p_term);
    assert_eq!(decoded.i_term, original.i_term);
    assert_eq!(decoded.d_term, original.d_term);
    assert_eq!(decoded.saturated, original.saturated);
}

#[test]
fn test_state_machine_telemetry_encode_decode() {
    // Create a state machine telemetry message
    let original = StateMachineTelemetry::new(3, 5000, 10000);

    // Encode to buffer
    let mut buffer = TelemetryBuffer::<128>::new();
    buffer.encode(&original).expect("Failed to encode");

    // Decode
    let decoded: StateMachineTelemetry =
        decode(buffer.as_bytes()).expect("Failed to decode");

    // Verify
    assert_eq!(decoded.state_id, original.state_id);
    assert_eq!(decoded.state_duration_ms, original.state_duration_ms);
    assert_eq!(decoded.last_transition, original.last_transition);
}

#[test]
fn test_frame_header_encode_decode() {
    // Create a frame header
    let original = FrameHeader::new(MessageType::ControlTelemetry, 42, 128);

    // Encode to buffer
    let mut buffer = TelemetryBuffer::<128>::new();
    buffer.encode(&original).expect("Failed to encode");

    // Decode
    let decoded: FrameHeader = decode(buffer.as_bytes()).expect("Failed to decode");

    // Verify
    assert_eq!(decoded.msg_type, original.msg_type);
    assert_eq!(decoded.sequence, original.sequence);
    assert_eq!(decoded.payload_len, original.payload_len);
}

#[test]
fn test_message_type_conversions() {
    // Test all message types
    let types = [
        MessageType::ControlTelemetry,
        MessageType::PidTelemetry,
        MessageType::StateMachineTelemetry,
        MessageType::Heartbeat,
        MessageType::Command,
        MessageType::Ack,
    ];

    for msg_type in &types {
        let u8_value = msg_type.to_u8();
        let converted = MessageType::from_u8(u8_value).expect("Failed to convert");
        assert_eq!(&converted, msg_type);
    }
}

#[test]
fn test_buffer_reuse() {
    // Create buffer and encode multiple messages
    let mut buffer = TelemetryBuffer::<128>::new();

    // First message
    let msg1 = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
    buffer.encode(&msg1).expect("Failed to encode msg1");
    let len1 = buffer.len();
    assert!(len1 > 0);

    // Second message (buffer should be reset)
    let msg2 = PidTelemetry::new(1.0, 0.5, 0.2, true);
    buffer.encode(&msg2).expect("Failed to encode msg2");
    let len2 = buffer.len();
    assert!(len2 > 0);
    // Lengths may differ
    assert_ne!(len1, len2);

    // Verify second message was encoded correctly
    let decoded: PidTelemetry = decode(buffer.as_bytes()).expect("Failed to decode");
    assert_eq!(decoded.saturated, true);
}

#[test]
fn test_buffer_clear() {
    let mut buffer = TelemetryBuffer::<128>::new();

    // Encode a message
    let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
    buffer.encode(&msg).expect("Failed to encode");
    assert!(!buffer.is_empty());

    // Clear buffer
    buffer.clear();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
}

#[test]
fn test_buffer_overflow() {
    // Create a very small buffer
    let mut buffer = TelemetryBuffer::<4>::new();

    // Try to encode a message that won't fit
    let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
    let result = buffer.encode(&msg);

    // Should fail with buffer overflow
    assert!(result.is_err());
}

#[test]
fn test_telemetry_with_frame_header() {
    // Simulate a complete telemetry frame with header and payload

    // Create frame header
    let mut header_buffer = TelemetryBuffer::<64>::new();
    let header = FrameHeader::new(MessageType::ControlTelemetry, 1, 0);
    header_buffer.encode(&header).expect("Failed to encode header");

    // Create payload
    let mut payload_buffer = TelemetryBuffer::<128>::new();
    let payload = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
    payload_buffer
        .encode(&payload)
        .expect("Failed to encode payload");

    // Update header with actual payload length
    let header_with_len =
        FrameHeader::new(MessageType::ControlTelemetry, 1, payload_buffer.len() as u16);
    header_buffer
        .encode(&header_with_len)
        .expect("Failed to encode updated header");

    // Decode header
    let decoded_header: FrameHeader =
        decode(header_buffer.as_bytes()).expect("Failed to decode header");
    assert_eq!(decoded_header.msg_type, MessageType::ControlTelemetry);
    assert_eq!(decoded_header.sequence, 1);
    assert_eq!(decoded_header.payload_len, payload_buffer.len() as u16);

    // Decode payload
    let decoded_payload: ControlTelemetry =
        decode(payload_buffer.as_bytes()).expect("Failed to decode payload");
    assert_eq!(decoded_payload.timestamp_ms, 1000);
}

#[test]
fn test_multiple_message_types_encoding() {
    // Encode different message types and verify they can be decoded based on type

    let msg_types = [
        (
            MessageType::ControlTelemetry,
            ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5),
        ),
        (
            MessageType::ControlTelemetry,
            ControlTelemetry::new(2000, 20.0, 19.5, 5.0, 1.0),
        ),
        (
            MessageType::ControlTelemetry,
            ControlTelemetry::new(3000, 30.0, 29.5, 7.5, 1.5),
        ),
    ];

    for (msg_type, msg) in &msg_types {
        let mut buffer = TelemetryBuffer::<128>::new();
        buffer.encode(msg).expect("Failed to encode");

        // Create header
        let header = FrameHeader::new(*msg_type, 0, buffer.len() as u16);
        assert_eq!(header.msg_type, *msg_type);
        assert_eq!(usize::from(header.payload_len), buffer.len());

        // Verify we can decode
        let decoded: ControlTelemetry =
            decode(buffer.as_bytes()).expect("Failed to decode");
        assert_eq!(decoded.timestamp_ms, msg.timestamp_ms);
        assert_eq!(decoded.setpoint, msg.setpoint);
    }
}

#[test]
fn test_contiguous_frame_header_then_payload() {
    // A frame on the wire is the header bytes immediately followed by the
    // payload bytes. The receiver must find the payload right after the
    // header and the frame must end exactly after `payload_len` bytes.
    let payload = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
    let mut payload_buffer = TelemetryBuffer::<128>::new();
    payload_buffer
        .encode(&payload)
        .expect("Failed to encode payload");

    let payload_len = u16::try_from(payload_buffer.len()).expect("payload fits in u16");
    let header = FrameHeader::new(MessageType::ControlTelemetry, 7, payload_len);
    let mut header_buffer = TelemetryBuffer::<64>::new();
    header_buffer.encode(&header).expect("Failed to encode header");

    let mut frame = [0u8; 192];
    let header_bytes = header_buffer.as_bytes();
    let payload_bytes = payload_buffer.as_bytes();
    let frame_len = header_bytes.len() + payload_bytes.len();
    frame[..header_bytes.len()].copy_from_slice(header_bytes);
    frame[header_bytes.len()..frame_len].copy_from_slice(payload_bytes);
    let frame = &frame[..frame_len];

    let mut decoder = minicbor::Decoder::new(frame);
    let decoded_header: FrameHeader = decoder.decode().expect("Failed to decode header");
    assert_eq!(decoded_header, header);

    let payload_start = decoder.position();
    assert_eq!(payload_start, header_bytes.len());
    let payload_end = payload_start + usize::from(decoded_header.payload_len);
    assert_eq!(payload_end, frame.len());

    let mut payload_decoder = minicbor::Decoder::new(&frame[payload_start..payload_end]);
    let decoded_payload: ControlTelemetry =
        payload_decoder.decode().expect("Failed to decode payload");
    assert_eq!(decoded_payload, payload);
    assert_eq!(payload_decoder.position(), payload_end - payload_start);
}
