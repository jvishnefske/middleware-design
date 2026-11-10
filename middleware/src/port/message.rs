//! Port message trait and serialization.
//!
//! Defines the trait for messages that can be sent through ports
//! with fixed-size buffer serialization for no_std compatibility.
//!
//! # Traceability
//! - REQ-PRT-001: Port messages implement serialize/deserialize to/from byte buffers
//! - REQ-PRT-002: Serialization errors are returned as Result

#![forbid(unsafe_code)]

use crate::error::SerializationError;

/// Maximum message payload size in bytes.
///
/// This is a conservative limit suitable for embedded systems.
/// Larger messages should be fragmented or use alternative transport.
pub const MAX_MESSAGE_SIZE: usize = 256;

/// Trait for messages that can be transmitted through ports.
///
/// Port messages must support serialization to and from fixed-size
/// byte buffers. This enables no_std compatibility and predictable
/// memory usage.
///
/// # Examples
///
/// ```
/// use middleware::SerializationError;
/// use middleware::port::PortMessage;
///
/// #[derive(Debug, Clone, PartialEq)]
/// struct SensorData {
///     temperature: f32,
///     pressure: f32,
/// }
///
/// impl PortMessage for SensorData {
///     fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
///         if buffer.len() < 8 {
///             return Err(SerializationError::BufferTooSmall);
///         }
///
///         buffer[0..4].copy_from_slice(&self.temperature.to_le_bytes());
///         buffer[4..8].copy_from_slice(&self.pressure.to_le_bytes());
///         Ok(8)
///     }
///
///     fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
///         if buffer.len() < 8 {
///             return Err(SerializationError::InvalidFormat);
///         }
///
///         let mut temp_bytes = [0u8; 4];
///         let mut pres_bytes = [0u8; 4];
///         temp_bytes.copy_from_slice(&buffer[0..4]);
///         pres_bytes.copy_from_slice(&buffer[4..8]);
///
///         Ok(Self {
///             temperature: f32::from_le_bytes(temp_bytes),
///             pressure: f32::from_le_bytes(pres_bytes),
///         })
///     }
/// }
///
/// let msg = SensorData { temperature: 25.0, pressure: 101.3 };
/// let mut buffer = [0u8; 8];
/// assert_eq!(msg.serialize(&mut buffer), Ok(8));
/// assert_eq!(SensorData::deserialize(&buffer), Ok(msg.clone()));
///
/// assert_eq!(msg.serialize(&mut [0u8; 4]), Err(SerializationError::BufferTooSmall));
/// assert_eq!(SensorData::deserialize(&[0u8; 4]), Err(SerializationError::InvalidFormat));
/// ```
pub trait PortMessage: Sized {
    /// Serialize the message into a byte buffer.
    ///
    /// The buffer must be large enough to hold the serialized message.
    /// Returns the number of bytes written.
    ///
    /// # Errors
    ///
    /// Returns `SerializationError` if serialization fails:
    /// - `BufferTooSmall`: Buffer insufficient for message
    /// - `EncodingError`: Message encoding failed
    ///
    /// # Examples
    ///
    /// ```
    /// # use middleware::SerializationError;
    /// # use middleware::port::PortMessage;
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
    /// # fn main() -> Result<(), SerializationError> {
    /// let msg = SensorData { temperature: 25.0, pressure: 101.3 };
    /// let mut buffer = [0u8; 256];
    /// let bytes_written = msg.serialize(&mut buffer)?;
    /// assert_eq!(bytes_written, 8);
    /// # Ok(())
    /// # }
    /// ```
    fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError>;

    /// Deserialize a message from a byte buffer.
    ///
    /// The buffer should contain a complete serialized message.
    ///
    /// # Errors
    ///
    /// Returns `SerializationError` if deserialization fails:
    /// - `InvalidFormat`: Buffer format invalid
    /// - `DecodingError`: Message decoding failed
    ///
    /// # Examples
    ///
    /// ```
    /// # use middleware::SerializationError;
    /// # use middleware::port::PortMessage;
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
    /// # fn main() -> Result<(), SerializationError> {
    /// # let original = SensorData { temperature: 25.0, pressure: 101.3 };
    /// # let mut buffer = [0u8; 256];
    /// # let bytes_written = original.serialize(&mut buffer)?;
    /// let msg = SensorData::deserialize(&buffer[..bytes_written])?;
    /// assert_eq!(msg, original);
    /// # Ok(())
    /// # }
    /// ```
    fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError>;

    /// Get the maximum serialized size of this message type.
    ///
    /// Used to validate buffer sizes before serialization.
    /// Default implementation returns MAX_MESSAGE_SIZE.
    fn max_size() -> usize {
        MAX_MESSAGE_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestMessage {
        value: u32,
        flag: bool,
    }

    impl PortMessage for TestMessage {
        fn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
            if buffer.len() < 5 {
                return Err(SerializationError::BufferTooSmall);
            }

            buffer[0..4].copy_from_slice(&self.value.to_le_bytes());
            buffer[4] = if self.flag { 1 } else { 0 };
            Ok(5)
        }

        fn deserialize(buffer: &[u8]) -> Result<Self, SerializationError> {
            if buffer.len() < 5 {
                return Err(SerializationError::InvalidFormat);
            }

            let mut value_bytes = [0u8; 4];
            value_bytes.copy_from_slice(&buffer[0..4]);
            let value = u32::from_le_bytes(value_bytes);
            let flag = buffer[4] != 0;

            Ok(Self { value, flag })
        }

        fn max_size() -> usize {
            5
        }
    }

    #[test]
    fn serialize_success() {
        let msg = TestMessage {
            value: 42,
            flag: true,
        };

        let mut buffer = [0u8; 256];
        let size = msg.serialize(&mut buffer).expect("Serialization should succeed");

        assert_eq!(size, 5);
        assert_eq!(buffer[0..4], 42u32.to_le_bytes());
        assert_eq!(buffer[4], 1);
    }

    #[test]
    fn serialize_buffer_too_small() {
        let msg = TestMessage {
            value: 42,
            flag: true,
        };

        let mut buffer = [0u8; 4];
        let result = msg.serialize(&mut buffer);

        assert_eq!(result, Err(SerializationError::BufferTooSmall));
    }

    #[test]
    fn deserialize_success() {
        let buffer = [42u8, 0, 0, 0, 1];
        let msg = TestMessage::deserialize(&buffer).expect("Deserialization should succeed");

        assert_eq!(msg.value, 42);
        assert_eq!(msg.flag, true);
    }

    #[test]
    fn deserialize_invalid_format() {
        let buffer = [42u8, 0, 0];
        let result = TestMessage::deserialize(&buffer);

        assert_eq!(result, Err(SerializationError::InvalidFormat));
    }

    #[test]
    fn roundtrip() {
        let original = TestMessage {
            value: 12345,
            flag: false,
        };

        let mut buffer = [0u8; 256];
        let size = original.serialize(&mut buffer).unwrap();
        let deserialized = TestMessage::deserialize(&buffer[..size]).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn max_size() {
        assert_eq!(TestMessage::max_size(), 5);
    }

    #[test]
    fn default_max_size() {
        struct DefaultSizeMsg;

        impl PortMessage for DefaultSizeMsg {
            fn serialize(&self, _buffer: &mut [u8]) -> Result<usize, SerializationError> {
                Ok(0)
            }

            fn deserialize(_buffer: &[u8]) -> Result<Self, SerializationError> {
                Ok(Self)
            }
        }

        assert_eq!(DefaultSizeMsg::max_size(), MAX_MESSAGE_SIZE);
    }
}
