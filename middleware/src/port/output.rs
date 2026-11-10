//! Output port trait and types.
//!
//! Defines traits for output ports that send messages to
//! connected consumers.
//!
//! # Traceability
//! - REQ-PRT-020: Output ports are parameterized by message type
//! - REQ-PRT-021: Output ports return Result<(), SendError> on send

#![forbid(unsafe_code)]

use crate::error::SendError;
use crate::port::message::PortMessage;

/// Trait for output ports.
///
/// Output ports send messages of a specific type to connected
/// consumers. The port is parameterized by the message type to
/// ensure type safety.
///
/// # Type Parameters
///
/// * `M` - The message type this port sends
///
/// # Examples
///
/// ```
/// use middleware::{SendError, SerializationError};
/// use middleware::port::{OutputPort, PortMessage};
///
/// # #[derive(Debug, Clone, Copy, PartialEq)]
/// # struct SensorData { temperature: f32, pressure: f32 }
/// # impl PortMessage for SensorData {
/// #     fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
/// #         let out = buffer.get_mut(..8).ok_or(SerializationError::BufferTooSmall)?;
/// #         out[..4].copy_from_slice(&self.temperature.to_le_bytes());
/// #         out[4..].copy_from_slice(&self.pressure.to_le_bytes());
/// #         Ok(8)
/// #     }
/// #     fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
/// #         let b = buffer.get(..8).ok_or(SerializationError::InvalidFormat)?;
/// #         Ok(Self {
/// #             temperature: f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
/// #             pressure: f32::from_le_bytes([b[4], b[5], b[6], b[7]]),
/// #         })
/// #     }
/// # }
///
/// # use core::cell::Cell;
/// # /// Single-slot port: `send` fails with `Full` while a message is pending.
/// # struct MockOutput<M> { slot: Cell<Option<M>> }
/// # impl<M> MockOutput<M> {
/// #     fn new() -> Self { Self { slot: Cell::new(None) } }
/// #     fn take(&self) -> Option<M> { self.slot.take() }
/// # }
/// # impl<M: PortMessage> OutputPort<M> for MockOutput<M> {
/// #     fn name(&self) -> &'static str { "mock" }
/// #     fn is_connected(&self) -> bool { true }
/// #     fn send(&self, message: M) -> Result<(), SendError> {
/// #         match self.slot.take() {
/// #             Some(pending) => { self.slot.set(Some(pending)); Err(SendError::Full) }
/// #             None => { self.slot.set(Some(message)); Ok(()) }
/// #         }
/// #     }
/// # }
///
/// let port = MockOutput::new();
///
/// let data = SensorData { temperature: 25.0, pressure: 101.3 };
/// assert_eq!(port.send(data), Ok(()));
///
/// // This mock holds one message, so a second send reports `Full`.
/// match port.send(data) {
///     Ok(()) => panic!("expected the port to be full"),
///     Err(SendError::Full) => {}
///     Err(e) => panic!("unexpected send error: {:?}", e),
/// }
/// assert_eq!(port.take(), Some(data));
/// ```
pub trait OutputPort<M: PortMessage> {
    /// Get the port name for debugging and telemetry.
    fn name(&self) -> &'static str;

    /// Check if the port is connected to any consumers.
    fn is_connected(&self) -> bool;

    /// Send a message through the port.
    ///
    /// Attempts to send the message to all connected consumers.
    /// The message is consumed by this operation.
    ///
    /// # Errors
    ///
    /// Returns `SendError` if send fails:
    /// - `Full`: Port buffer full (would block)
    /// - `Disconnected`: No consumers connected
    /// - `SerializationFailed`: Message serialization failed
    ///
    /// # Examples
    ///
    /// ```
    /// # use middleware::{SendError, SerializationError};
    /// # use middleware::port::{OutputPort, PortMessage};
    /// # #[derive(Debug, Clone, Copy, PartialEq)]
    /// # struct ControlOutput { voltage: f32, current: f32 }
    /// # impl PortMessage for ControlOutput {
    /// #     fn serialize(&self, _buffer: &mut [u8]) -> Result<usize, SerializationError> { Ok(0) }
    /// #     fn deserialize(_buffer: &[u8]) -> Result<Self, SerializationError> {
    /// #         Err(SerializationError::InvalidFormat)
    /// #     }
    /// # }
    ///
    /// # use core::cell::Cell;
    /// # /// Single-slot port: `send` fails with `Full` while a message is pending.
    /// # struct MockOutput<M> { slot: Cell<Option<M>> }
    /// # impl<M> MockOutput<M> {
    /// #     fn new() -> Self { Self { slot: Cell::new(None) } }
    /// #     fn take(&self) -> Option<M> { self.slot.take() }
    /// # }
    /// # impl<M: PortMessage> OutputPort<M> for MockOutput<M> {
    /// #     fn name(&self) -> &'static str { "mock" }
    /// #     fn is_connected(&self) -> bool { true }
    /// #     fn send(&self, message: M) -> Result<(), SendError> {
    /// #         match self.slot.take() {
    /// #             Some(pending) => { self.slot.set(Some(pending)); Err(SendError::Full) }
    /// #             None => { self.slot.set(Some(message)); Ok(()) }
    /// #         }
    /// #     }
    /// # }
    ///
    /// # fn main() -> Result<(), SendError> {
    /// # let port = MockOutput::new();
    /// let msg = ControlOutput { voltage: 3.3, current: 0.5 };
    /// port.send(msg)?;
    /// # assert_eq!(port.take(), Some(msg));
    /// # Ok(())
    /// # }
    /// ```
    fn send(&self, message: M) -> Result<(), SendError>;

    /// Try to send a message without blocking.
    ///
    /// Returns immediately. If the port buffer is full, returns
    /// `SendError::Full` without blocking.
    ///
    /// # Errors
    ///
    /// Same as `send()`, but guaranteed not to block.
    fn try_send(&self, message: M) -> Result<(), SendError> {
        self.send(message)
    }

    /// Get the number of connected consumers.
    ///
    /// Returns the count of active connections to this port.
    fn consumer_count(&self) -> usize {
        if self.is_connected() {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SerializationError;

    #[derive(Debug, Clone, PartialEq)]
    struct TestMessage {
        value: u32,
    }

    impl PortMessage for TestMessage {
        fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
            buffer
                .get_mut(..4)
                .ok_or(SerializationError::BufferTooSmall)?
                .copy_from_slice(&self.value.to_le_bytes());
            Ok(4)
        }

        fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
            let bytes: [u8; 4] = buffer
                .get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or(SerializationError::InvalidFormat)?;
            Ok(Self {
                value: u32::from_le_bytes(bytes),
            })
        }
    }

    struct TestOutputPort {
        name: &'static str,
        connected: bool,
        full: bool,
    }

    impl TestOutputPort {
        fn new(name: &'static str, connected: bool) -> Self {
            Self {
                name,
                connected,
                full: false,
            }
        }

        fn with_full(mut self, full: bool) -> Self {
            self.full = full;
            self
        }
    }

    impl OutputPort<TestMessage> for TestOutputPort {
        fn name(&self) -> &'static str {
            self.name
        }

        fn is_connected(&self) -> bool {
            self.connected
        }

        fn send(&self, _message: TestMessage) -> Result<(), SendError> {
            if !self.connected {
                return Err(SendError::Disconnected);
            }
            if self.full {
                return Err(SendError::Full);
            }
            // In real implementation, would actually send the message
            // For testing, we just validate it can be sent
            Ok(())
        }
    }

    #[test]
    fn output_port_name() {
        let port = TestOutputPort::new("test_output", true);
        assert_eq!(port.name(), "test_output");
    }

    #[test]
    fn output_port_connected() {
        let port = TestOutputPort::new("test", true);
        assert!(port.is_connected());
    }

    #[test]
    fn output_port_send_success() {
        let port = TestOutputPort::new("test", true);
        let msg = TestMessage { value: 42 };

        let result = port.send(msg);
        assert!(result.is_ok());
    }

    #[test]
    fn output_port_send_disconnected() {
        let port = TestOutputPort::new("test", false);
        let msg = TestMessage { value: 42 };

        let result = port.send(msg);
        assert_eq!(result, Err(SendError::Disconnected));
    }

    #[test]
    fn output_port_send_full() {
        let port = TestOutputPort::new("test", true).with_full(true);
        let msg = TestMessage { value: 42 };

        let result = port.send(msg);
        assert_eq!(result, Err(SendError::Full));
    }

    #[test]
    fn output_port_try_send() {
        let port = TestOutputPort::new("test", true);
        let msg = TestMessage { value: 123 };

        let result = port.try_send(msg);
        assert!(result.is_ok());
    }

    #[test]
    fn output_port_consumer_count() {
        let connected = TestOutputPort::new("test", true);
        assert_eq!(connected.consumer_count(), 1);

        let disconnected = TestOutputPort::new("test", false);
        assert_eq!(disconnected.consumer_count(), 0);
    }
}
