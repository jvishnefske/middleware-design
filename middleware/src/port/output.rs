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

    /// Single-slot output port that serializes each message into a frame
    /// of `frame_len` bytes, as a transport would before transmitting.
    struct SlotOutputPort<M> {
        connected: bool,
        frame_len: usize,
        slot: Cell<Option<([u8; SLOT], usize)>>,
        _message: PhantomData<M>,
    }

    impl<M: PortMessage> SlotOutputPort<M> {
        fn new(connected: bool, frame_len: usize) -> Self {
            Self {
                connected,
                frame_len,
                slot: Cell::new(None),
                _message: PhantomData,
            }
        }

        /// Removes and decodes the pending frame, as the consumer would.
        fn take(&self) -> Option<Result<M, SerializationError>> {
            let (buf, len) = self.slot.take()?;
            Some(
                buf.get(..len)
                    .map_or(Err(SerializationError::InvalidFormat), M::deserialize),
            )
        }
    }

    impl<M: PortMessage> OutputPort<M> for SlotOutputPort<M> {
        fn name(&self) -> &'static str {
            "slot"
        }

        fn is_connected(&self) -> bool {
            self.connected
        }

        fn send(&self, message: M) -> Result<(), SendError> {
            if !self.connected {
                return Err(SendError::Disconnected);
            }
            if let Some(pending) = self.slot.take() {
                self.slot.set(Some(pending));
                return Err(SendError::Full);
            }
            let mut buf = [0u8; SLOT];
            let frame = buf
                .get_mut(..self.frame_len)
                .ok_or(SendError::SerializationFailed)?;
            let len = message
                .serialize(frame)
                .map_err(|_| SendError::SerializationFailed)?;
            self.slot.set(Some((buf, len)));
            Ok(())
        }
    }

    fn send_as<M: PortMessage, P: OutputPort<M>>(port: &P, message: M) -> Result<(), SendError> {
        port.send(message)
    }

    /// The same port implementation, instantiated for two message types,
    /// accepts and transmits exactly the type it is parameterized by.
    ///
    /// Verifies: REQ-PRT-020
    #[test]
    fn output_ports_are_parameterized_by_message_type() {
        let message_port = SlotOutputPort::<TestMessage>::new(true, 4);
        let tick_port = SlotOutputPort::<Tick>::new(true, 1);

        assert_eq!(
            send_as(&message_port, TestMessage { value: 0xDEAD_BEEF }),
            Ok(())
        );
        assert_eq!(send_as(&tick_port, Tick(200)), Ok(()));
        assert_eq!(
            message_port.take(),
            Some(Ok(TestMessage { value: 0xDEAD_BEEF }))
        );
        assert_eq!(tick_port.take(), Some(Ok(Tick(200))));
        assert_eq!(tick_port.take(), None);

        assert_eq!(OutputPort::<Tick>::name(&tick_port), "slot");
        assert!(OutputPort::<TestMessage>::is_connected(&message_port));
    }

    /// `send` has the signature `Result<(), SendError>` (checked by the
    /// function-pointer binding) and every injected fault is a distinct
    /// error: full port (with the pending message preserved), no consumer,
    /// and a frame too small to serialize into (zero bytes and one byte
    /// short). `try_send` forwards the same outcomes.
    ///
    /// Verifies: REQ-PRT-021
    #[test]
    fn send_returns_result_with_send_error() {
        let _send: fn(&SlotOutputPort<TestMessage>, TestMessage) -> Result<(), SendError> =
            <SlotOutputPort<TestMessage> as OutputPort<TestMessage>>::send;

        let port = SlotOutputPort::<TestMessage>::new(true, 4);
        assert_eq!(port.send(TestMessage { value: 1 }), Ok(()));
        assert_eq!(port.send(TestMessage { value: 2 }), Err(SendError::Full));
        assert_eq!(
            port.try_send(TestMessage { value: 3 }),
            Err(SendError::Full)
        );
        assert_eq!(port.take(), Some(Ok(TestMessage { value: 1 })));
        assert_eq!(port.try_send(TestMessage { value: 4 }), Ok(()));
        assert_eq!(port.take(), Some(Ok(TestMessage { value: 4 })));

        let disconnected = SlotOutputPort::<TestMessage>::new(false, 4);
        assert_eq!(
            disconnected.send(TestMessage { value: 1 }),
            Err(SendError::Disconnected)
        );
        assert_eq!(
            disconnected.try_send(TestMessage { value: 1 }),
            Err(SendError::Disconnected)
        );
        assert_eq!(disconnected.take(), None);

        let one_short = SlotOutputPort::<TestMessage>::new(true, 3);
        assert_eq!(
            one_short.send(TestMessage { value: 1 }),
            Err(SendError::SerializationFailed)
        );
        assert_eq!(one_short.take(), None);
        let zero = SlotOutputPort::<Tick>::new(true, 0);
        assert_eq!(zero.send(Tick(1)), Err(SendError::SerializationFailed));
        let oversized = SlotOutputPort::<Tick>::new(true, SLOT + 1);
        assert_eq!(oversized.send(Tick(1)), Err(SendError::SerializationFailed));
    }

    #[test]
    fn output_port_consumer_count() {
        let connected = SlotOutputPort::<TestMessage>::new(true, 4);
        assert_eq!(connected.consumer_count(), 1);

        let disconnected = SlotOutputPort::<TestMessage>::new(false, 4);
        assert_eq!(disconnected.consumer_count(), 0);
    }

    #[test]
    fn consumer_decodes_invalid_frame_as_error() {
        let port = SlotOutputPort::<TestMessage>::new(true, 4);
        port.slot.set(Some(([0u8; SLOT], 2)));
        assert_eq!(port.take(), Some(Err(SerializationError::InvalidFormat)));
        port.slot.set(Some(([0u8; SLOT], SLOT + 1)));
        assert_eq!(port.take(), Some(Err(SerializationError::InvalidFormat)));
    }
}
