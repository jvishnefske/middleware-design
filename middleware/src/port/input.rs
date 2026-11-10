//! Input port traits and types.
//!
//! Defines traits for input ports supporting both synchronous
//! (blocking with timeout) and asynchronous (non-blocking poll)
//! message reception.
//!
//! # Traceability
//! - REQ-PRT-010: Input ports are parameterized by message type
//! - REQ-PRT-011: Sync input ports support blocking recv with timeout
//! - REQ-PRT-012: Async input ports support non-blocking poll

#![forbid(unsafe_code)]

use core::time::Duration;
use crate::error::RecvError;
use crate::port::message::PortMessage;

/// Base trait for input ports.
///
/// Input ports receive messages of a specific type. The port
/// is parameterized by the message type to ensure type safety.
///
/// # Type Parameters
///
/// * `M` - The message type this port receives
pub trait InputPort<M: PortMessage> {
    /// Get the port name for debugging and telemetry.
    fn name(&self) -> &'static str;

    /// Check if the port is connected to a source.
    fn is_connected(&self) -> bool;
}

/// Synchronous input port with blocking receive.
///
/// Synchronous ports support blocking receive operations with
/// configurable timeout. This is suitable for components that
/// can afford to wait for data.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use middleware::{RecvError, SerializationError};
/// use middleware::port::{PortMessage, SyncInputPort};
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
/// # use middleware::port::InputPort;
/// # struct MockPort<M> { pending: Cell<Option<M>> }
/// # impl<M> MockPort<M> { fn with(msg: M) -> Self { Self { pending: Cell::new(Some(msg)) } } }
/// # impl<M: PortMessage> InputPort<M> for MockPort<M> {
/// #     fn name(&self) -> &'static str { "mock" }
/// #     fn is_connected(&self) -> bool { true }
/// # }
/// # impl<M: PortMessage> SyncInputPort<M> for MockPort<M> {
/// #     fn recv(&self, _timeout: Duration) -> Result<M, RecvError> {
/// #         self.pending.take().ok_or(RecvError::NoMessage)
/// #     }
/// # }
///
/// /// Returns the temperature, or `None` if nothing arrived within 100 ms.
/// fn read_temperature<P: SyncInputPort<SensorData>>(port: &P) -> Result<Option<f32>, RecvError> {
///     match port.recv(Duration::from_millis(100)) {
///         Ok(data) => Ok(Some(data.temperature)),
///         Err(RecvError::NoMessage) => Ok(None),
///         Err(e) => Err(e),
///     }
/// }
///
/// let port = MockPort::with(SensorData { temperature: 25.0, pressure: 101.3 });
/// assert_eq!(read_temperature(&port), Ok(Some(25.0)));
/// assert_eq!(read_temperature(&port), Ok(None));
/// ```
pub trait SyncInputPort<M: PortMessage>: InputPort<M> {
    /// Receive a message with timeout.
    ///
    /// Blocks until a message is available or the timeout expires.
    /// Returns the message on success.
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait for a message
    ///
    /// # Errors
    ///
    /// Returns `RecvError` if receive fails:
    /// - `NoMessage`: Timeout expired with no message
    /// - `Disconnected`: Port disconnected from source
    /// - `DeserializationFailed`: Message deserialization failed
    ///
    /// # Examples
    ///
    /// ```
    /// # use core::time::Duration;
    /// # use middleware::{RecvError, SerializationError};
    /// # use middleware::port::{PortMessage, SyncInputPort};
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
    /// # use middleware::port::InputPort;
    /// # struct MockPort<M> { pending: Cell<Option<M>> }
    /// # impl<M> MockPort<M> { fn with(msg: M) -> Self { Self { pending: Cell::new(Some(msg)) } } }
    /// # impl<M: PortMessage> InputPort<M> for MockPort<M> {
    /// #     fn name(&self) -> &'static str { "mock" }
    /// #     fn is_connected(&self) -> bool { true }
    /// # }
    /// # impl<M: PortMessage> SyncInputPort<M> for MockPort<M> {
    /// #     fn recv(&self, _timeout: Duration) -> Result<M, RecvError> {
    /// #         self.pending.take().ok_or(RecvError::NoMessage)
    /// #     }
    /// # }
    ///
    /// # fn main() -> Result<(), RecvError> {
    /// # let port = MockPort::with(SensorData { temperature: 25.0, pressure: 101.3 });
    /// // Wait up to 100ms for sensor data
    /// let data = port.recv(Duration::from_millis(100))?;
    /// assert_eq!(data.temperature, 25.0);
    ///
    /// // Once drained, the port reports a timeout as `NoMessage`.
    /// assert_eq!(port.recv(Duration::from_millis(100)), Err(RecvError::NoMessage));
    /// # Ok(())
    /// # }
    /// ```
    fn recv(&self, timeout: Duration) -> Result<M, RecvError>;

    /// Try to receive a message without blocking.
    ///
    /// Returns immediately with either a message or an error.
    /// Equivalent to recv with zero timeout.
    ///
    /// # Errors
    ///
    /// Returns `RecvError::NoMessage` if no message available.
    fn try_recv(&self) -> Result<M, RecvError> {
        self.recv(Duration::from_secs(0))
    }
}

/// Asynchronous input port with non-blocking poll.
///
/// Asynchronous ports support non-blocking polling for messages.
/// This is suitable for high-frequency control loops that cannot
/// afford to block.
///
/// # Examples
///
/// ```
/// use middleware::SerializationError;
/// use middleware::port::{AsyncInputPort, PortMessage};
///
/// # use core::cell::Cell;
/// # use middleware::port::InputPort;
/// # #[derive(Debug, Clone, Copy, PartialEq)]
/// # enum ControlCommand { Enable, Disable }
/// # impl PortMessage for ControlCommand {
/// #     fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
/// #         let b = buffer.first_mut().ok_or(SerializationError::BufferTooSmall)?;
/// #         *b = match self { ControlCommand::Enable => 1, ControlCommand::Disable => 0 };
/// #         Ok(1)
/// #     }
/// #     fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
/// #         match buffer.first() {
/// #             Some(1) => Ok(ControlCommand::Enable),
/// #             Some(0) => Ok(ControlCommand::Disable),
/// #             _ => Err(SerializationError::InvalidFormat),
/// #         }
/// #     }
/// # }
/// # struct MockPort { pending: Cell<Option<ControlCommand>> }
/// # impl MockPort { fn with(cmd: ControlCommand) -> Self { Self { pending: Cell::new(Some(cmd)) } } }
/// # impl InputPort<ControlCommand> for MockPort {
/// #     fn name(&self) -> &'static str { "commands" }
/// #     fn is_connected(&self) -> bool { true }
/// # }
/// # impl AsyncInputPort<ControlCommand> for MockPort {
/// #     fn poll(&self) -> Option<ControlCommand> { self.pending.take() }
/// # }
///
/// let port = MockPort::with(ControlCommand::Enable);
///
/// let mut executed = 0;
/// if let Some(cmd) = port.poll() {
///     assert_eq!(cmd, ControlCommand::Enable);
///     executed += 1;
/// } else {
///     // No command available, continue
/// }
/// assert_eq!(executed, 1);
///
/// // Polling never blocks: an empty port yields `None` immediately.
/// assert_eq!(port.poll(), None);
/// ```
pub trait AsyncInputPort<M: PortMessage>: InputPort<M> {
    /// Poll for a message without blocking.
    ///
    /// Returns `Some(message)` if a message is available, or `None`
    /// if no message is ready. Never blocks.
    ///
    /// # Examples
    ///
    /// ```
    /// # use middleware::SerializationError;
    /// # use middleware::port::{AsyncInputPort, PortMessage};
    ///
    /// # use core::cell::Cell;
    /// # use middleware::port::InputPort;
    /// # #[derive(Debug, Clone, Copy, PartialEq)]
    /// # enum ControlCommand { Enable, Disable }
    /// # impl PortMessage for ControlCommand {
    /// #     fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
    /// #         let b = buffer.first_mut().ok_or(SerializationError::BufferTooSmall)?;
    /// #         *b = match self { ControlCommand::Enable => 1, ControlCommand::Disable => 0 };
    /// #         Ok(1)
    /// #     }
    /// #     fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
    /// #         match buffer.first() {
    /// #             Some(1) => Ok(ControlCommand::Enable),
    /// #             Some(0) => Ok(ControlCommand::Disable),
    /// #             _ => Err(SerializationError::InvalidFormat),
    /// #         }
    /// #     }
    /// # }
    /// # struct MockPort { pending: Cell<Option<ControlCommand>> }
    /// # impl MockPort { fn with(cmd: ControlCommand) -> Self { Self { pending: Cell::new(Some(cmd)) } } }
    /// # impl InputPort<ControlCommand> for MockPort {
    /// #     fn name(&self) -> &'static str { "commands" }
    /// #     fn is_connected(&self) -> bool { true }
    /// # }
    /// # impl AsyncInputPort<ControlCommand> for MockPort {
    /// #     fn poll(&self) -> Option<ControlCommand> { self.pending.take() }
    /// # }
    ///
    /// # let port = MockPort::with(ControlCommand::Disable);
    /// // Check for new command
    /// if let Some(cmd) = port.poll() {
    ///     assert_eq!(cmd, ControlCommand::Disable);
    /// }
    /// assert!(port.poll().is_none());
    /// ```
    fn poll(&self) -> Option<M>;

    /// Check if a message is available without consuming it.
    ///
    /// Returns `true` if `poll()` would return `Some`.
    fn has_message(&self) -> bool {
        false // Default: conservative assumption
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
            if buffer.len() < 4 {
                return Err(SerializationError::BufferTooSmall);
            }
            buffer[0..4].copy_from_slice(&self.value.to_le_bytes());
            Ok(4)
        }

        fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
            if buffer.len() < 4 {
                return Err(SerializationError::InvalidFormat);
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&buffer[0..4]);
            Ok(Self {
                value: u32::from_le_bytes(bytes),
            })
        }
    }

    struct TestSyncPort {
        name: &'static str,
        connected: bool,
        message: Option<TestMessage>,
    }

    impl InputPort<TestMessage> for TestSyncPort {
        fn name(&self) -> &'static str {
            self.name
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    impl SyncInputPort<TestMessage> for TestSyncPort {
        fn recv(&self, _timeout: Duration) -> Result<TestMessage, RecvError> {
            if !self.connected {
                return Err(RecvError::Disconnected);
            }
            self.message.clone().ok_or(RecvError::NoMessage)
        }
    }

    struct TestAsyncPort {
        name: &'static str,
        connected: bool,
        message: Option<TestMessage>,
    }

    impl InputPort<TestMessage> for TestAsyncPort {
        fn name(&self) -> &'static str {
            self.name
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    impl AsyncInputPort<TestMessage> for TestAsyncPort {
        fn poll(&self) -> Option<TestMessage> {
            if self.connected {
                self.message.clone()
            } else {
                None
            }
        }

        fn has_message(&self) -> bool {
            self.message.is_some()
        }
    }

    #[test]
    fn sync_port_name() {
        let port = TestSyncPort {
            name: "test_port",
            connected: true,
            message: None,
        };
        assert_eq!(port.name(), "test_port");
    }

    #[test]
    fn sync_port_connected() {
        let port = TestSyncPort {
            name: "test",
            connected: true,
            message: None,
        };
        assert!(port.is_connected());
    }

    #[test]
    fn sync_port_recv_success() {
        let port = TestSyncPort {
            name: "test",
            connected: true,
            message: Some(TestMessage { value: 42 }),
        };

        let msg = port.recv(Duration::from_millis(10)).unwrap();
        assert_eq!(msg.value, 42);
    }

    #[test]
    fn sync_port_recv_no_message() {
        let port = TestSyncPort {
            name: "test",
            connected: true,
            message: None,
        };

        let result = port.recv(Duration::from_millis(10));
        assert_eq!(result, Err(RecvError::NoMessage));
    }

    #[test]
    fn sync_port_recv_disconnected() {
        let port = TestSyncPort {
            name: "test",
            connected: false,
            message: None,
        };

        let result = port.recv(Duration::from_millis(10));
        assert_eq!(result, Err(RecvError::Disconnected));
    }

    #[test]
    fn sync_port_try_recv() {
        let port = TestSyncPort {
            name: "test",
            connected: true,
            message: Some(TestMessage { value: 123 }),
        };

        let msg = port.try_recv().unwrap();
        assert_eq!(msg.value, 123);
    }

    #[test]
    fn async_port_poll_success() {
        let port = TestAsyncPort {
            name: "test",
            connected: true,
            message: Some(TestMessage { value: 99 }),
        };

        let msg = port.poll().unwrap();
        assert_eq!(msg.value, 99);
    }

    #[test]
    fn async_port_poll_no_message() {
        let port = TestAsyncPort {
            name: "test",
            connected: true,
            message: None,
        };

        assert!(port.poll().is_none());
    }

    #[test]
    fn async_port_poll_disconnected() {
        let port = TestAsyncPort {
            name: "test",
            connected: false,
            message: Some(TestMessage { value: 99 }),
        };

        assert!(port.poll().is_none());
    }

    #[test]
    fn async_port_has_message() {
        let port = TestAsyncPort {
            name: "test",
            connected: true,
            message: Some(TestMessage { value: 1 }),
        };

        assert!(port.has_message());
    }
}
