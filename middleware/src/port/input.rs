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

use crate::error::RecvError;
use crate::port::message::PortMessage;
use core::time::Duration;

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
    /// An implementation that overrides this returns `true` exactly when
    /// `poll()` would return `Some`. The default implementation cannot peek
    /// and always returns `false`, so callers must treat a default `false`
    /// as "unknown" and still call `poll()`; a `true` is always reliable.
    fn has_message(&self) -> bool {
        false // Default: conservative assumption
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SerializationError;
    use core::cell::Cell;
    use core::marker::PhantomData;

    /// Four-byte little-endian `u32` message.
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

    /// One-byte message, a second type to show ports are typed per message.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Tick(u8);

    impl PortMessage for Tick {
        fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
            *buffer
                .first_mut()
                .ok_or(SerializationError::BufferTooSmall)? = self.0;
            Ok(1)
        }

        fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
            buffer
                .first()
                .map(|b| Tick(*b))
                .ok_or(SerializationError::InvalidFormat)
        }
    }

    const SLOT: usize = 8;

    /// Raw bytes of one pending message as received off a link.
    type Payload = Cell<Option<([u8; SLOT], usize)>>;

    fn payload_from(bytes: &[u8]) -> Payload {
        let mut buf = [0u8; SLOT];
        buf.get_mut(..bytes.len()).unwrap().copy_from_slice(bytes);
        Cell::new(Some((buf, bytes.len())))
    }

    /// Takes the pending payload and decodes it as `M`, mapping a missing
    /// payload to `NoMessage` and any decode error to
    /// `DeserializationFailed`, as a real transport would.
    fn take_decoded<M: PortMessage>(payload: &Payload) -> Result<M, RecvError> {
        let (buf, len) = payload.take().ok_or(RecvError::NoMessage)?;
        let bytes = buf.get(..len).ok_or(RecvError::DeserializationFailed)?;
        M::deserialize(bytes).map_err(|_| RecvError::DeserializationFailed)
    }

    /// Single-slot sync port backed by raw bytes; records the last timeout.
    struct BytesSyncPort<M> {
        name: &'static str,
        connected: bool,
        payload: Payload,
        last_timeout: Cell<Option<Duration>>,
        _message: PhantomData<M>,
    }

    impl<M> BytesSyncPort<M> {
        fn new(connected: bool, bytes: Option<&[u8]>) -> Self {
            Self {
                name: "sync",
                connected,
                payload: bytes.map_or(Cell::new(None), payload_from),
                last_timeout: Cell::new(None),
                _message: PhantomData,
            }
        }
    }

    impl<M: PortMessage> InputPort<M> for BytesSyncPort<M> {
        fn name(&self) -> &'static str {
            self.name
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    impl<M: PortMessage> SyncInputPort<M> for BytesSyncPort<M> {
        fn recv(&self, timeout: Duration) -> Result<M, RecvError> {
            self.last_timeout.set(Some(timeout));
            if !self.connected {
                return Err(RecvError::Disconnected);
            }
            take_decoded(&self.payload)
        }
    }

    /// Single-slot async port backed by raw bytes; overrides `has_message`.
    struct BytesAsyncPort<M> {
        connected: bool,
        payload: Payload,
        _message: PhantomData<M>,
    }

    impl<M> BytesAsyncPort<M> {
        fn new(connected: bool, bytes: Option<&[u8]>) -> Self {
            Self {
                connected,
                payload: bytes.map_or(Cell::new(None), payload_from),
                _message: PhantomData,
            }
        }
    }

    impl<M: PortMessage> InputPort<M> for BytesAsyncPort<M> {
        fn name(&self) -> &'static str {
            "async"
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    impl<M: PortMessage> AsyncInputPort<M> for BytesAsyncPort<M> {
        fn poll(&self) -> Option<M> {
            if !self.connected {
                return None;
            }
            take_decoded(&self.payload).ok()
        }

        fn has_message(&self) -> bool {
            let pending = self.payload.take();
            let present = pending.is_some();
            self.payload.set(pending);
            self.connected && present
        }
    }

    /// Async port that relies on the default `has_message`.
    struct MinimalAsyncPort {
        pending: Cell<Option<Tick>>,
    }

    impl InputPort<Tick> for MinimalAsyncPort {
        fn name(&self) -> &'static str {
            "minimal"
        }

        fn is_connected(&self) -> bool {
            true
        }
    }

    impl AsyncInputPort<Tick> for MinimalAsyncPort {
        fn poll(&self) -> Option<Tick> {
            self.pending.take()
        }
    }

    fn describe<M: PortMessage, P: InputPort<M>>(port: &P) -> (&'static str, bool) {
        (port.name(), port.is_connected())
    }

    fn recv_as<M: PortMessage, P: SyncInputPort<M>>(port: &P) -> Result<M, RecvError> {
        port.recv(Duration::from_millis(1))
    }

    fn poll_as<M: PortMessage, P: AsyncInputPort<M>>(port: &P) -> Option<M> {
        port.poll()
    }

    /// The same port implementation, instantiated for two message types,
    /// decodes identical bytes into the type it is parameterized by. The
    /// result types are fixed by the generic bounds at compile time.
    ///
    /// Verifies: REQ-PRT-010
    #[test]
    fn input_ports_are_parameterized_by_message_type() {
        let bytes = 0x0403_0201u32.to_le_bytes();

        let as_message = BytesSyncPort::<TestMessage>::new(true, Some(&bytes));
        let as_tick = BytesSyncPort::<Tick>::new(true, Some(&bytes));
        let message: Result<TestMessage, RecvError> = recv_as(&as_message);
        let tick: Result<Tick, RecvError> = recv_as(&as_tick);
        assert_eq!(message, Ok(TestMessage { value: 0x0403_0201 }));
        assert_eq!(tick, Ok(Tick(0x01)));

        let async_message = BytesAsyncPort::<TestMessage>::new(true, Some(&bytes));
        let async_tick = BytesAsyncPort::<Tick>::new(true, Some(&bytes));
        assert_eq!(
            poll_as(&async_message),
            Some(TestMessage { value: 0x0403_0201 })
        );
        assert_eq!(poll_as(&async_tick), Some(Tick(0x01)));

        assert_eq!(describe::<TestMessage, _>(&as_message), ("sync", true));
        assert_eq!(describe::<Tick, _>(&async_tick), ("async", true));
        assert_eq!(
            describe::<Tick, _>(&MinimalAsyncPort {
                pending: Cell::new(None)
            }),
            ("minimal", true)
        );
    }

    /// `recv` takes the caller's timeout and passes it to the
    /// implementation unchanged (including the zero and maximum boundaries),
    /// `try_recv` is `recv` with a zero timeout, and every failure (empty
    /// port, disconnected, short or empty payload) is a distinct `RecvError`.
    ///
    /// Verifies: REQ-PRT-011
    #[test]
    fn sync_recv_honours_timeout_and_reports_faults() {
        let port = BytesSyncPort::<TestMessage>::new(true, Some(&42u32.to_le_bytes()));
        assert_eq!(
            port.recv(Duration::from_millis(100)),
            Ok(TestMessage { value: 42 })
        );
        assert_eq!(port.last_timeout.get(), Some(Duration::from_millis(100)));

        // Empty port: the timeout expires with no message.
        assert_eq!(port.recv(Duration::MAX), Err(RecvError::NoMessage));
        assert_eq!(port.last_timeout.get(), Some(Duration::MAX));
        assert_eq!(port.try_recv(), Err(RecvError::NoMessage));
        assert_eq!(port.last_timeout.get(), Some(Duration::ZERO));

        let ready = BytesSyncPort::<TestMessage>::new(true, Some(&7u32.to_le_bytes()));
        assert_eq!(ready.try_recv(), Ok(TestMessage { value: 7 }));
        assert_eq!(ready.last_timeout.get(), Some(Duration::ZERO));

        let disconnected = BytesSyncPort::<TestMessage>::new(false, Some(&[1, 2, 3, 4]));
        assert!(!disconnected.is_connected());
        assert_eq!(
            disconnected.recv(Duration::from_millis(10)),
            Err(RecvError::Disconnected)
        );
        assert_eq!(disconnected.try_recv(), Err(RecvError::Disconnected));

        let short = BytesSyncPort::<TestMessage>::new(true, Some(&[1, 2, 3]));
        assert_eq!(short.try_recv(), Err(RecvError::DeserializationFailed));
        // The bad payload is consumed; the port is then empty.
        assert_eq!(short.try_recv(), Err(RecvError::NoMessage));

        let empty_payload = BytesSyncPort::<Tick>::new(true, Some(&[]));
        assert_eq!(
            empty_payload.try_recv(),
            Err(RecvError::DeserializationFailed)
        );
    }

    /// `poll` has no timeout parameter and returns `Option` (checked by the
    /// function-pointer binding), so it returns immediately: `Some` once per
    /// pending message, then `None`. Disconnection and undecodable payloads
    /// also yield `None`. `has_message` agrees with `poll` when overridden
    /// and is `false` by default.
    ///
    /// Verifies: REQ-PRT-012
    #[test]
    fn async_poll_is_non_blocking_and_consumes() {
        let _poll: fn(&BytesAsyncPort<TestMessage>) -> Option<TestMessage> =
            <BytesAsyncPort<TestMessage> as AsyncInputPort<TestMessage>>::poll;

        let port = BytesAsyncPort::<TestMessage>::new(true, Some(&99u32.to_le_bytes()));
        assert!(port.has_message());
        assert_eq!(port.poll(), Some(TestMessage { value: 99 }));
        assert!(!port.has_message());
        assert_eq!(port.poll(), None);

        let empty = BytesAsyncPort::<TestMessage>::new(true, None);
        assert!(!empty.has_message());
        assert_eq!(empty.poll(), None);

        let disconnected = BytesAsyncPort::<TestMessage>::new(false, Some(&[0; 4]));
        assert!(!disconnected.is_connected());
        assert!(!disconnected.has_message());
        assert_eq!(disconnected.poll(), None);

        let invalid = BytesAsyncPort::<TestMessage>::new(true, Some(&[0; 2]));
        assert!(invalid.has_message());
        assert_eq!(invalid.poll(), None);

        let minimal = MinimalAsyncPort {
            pending: Cell::new(Some(Tick(5))),
        };
        assert!(!minimal.has_message());
        assert_eq!(minimal.poll(), Some(Tick(5)));
        assert_eq!(minimal.poll(), None);
    }

    #[test]
    fn test_messages_serialize() {
        let mut buf = [0u8; 4];
        assert_eq!(TestMessage { value: 9 }.serialize(&mut buf), Ok(4));
        assert_eq!(buf, 9u32.to_le_bytes());
        assert_eq!(
            TestMessage { value: 9 }.serialize(&mut [0u8; 3]),
            Err(SerializationError::BufferTooSmall)
        );
        assert_eq!(Tick(3).serialize(&mut buf), Ok(1));
        assert_eq!(
            Tick(3).serialize(&mut []),
            Err(SerializationError::BufferTooSmall)
        );
    }
}
