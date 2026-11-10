//! Fixed-size telemetry buffers for no_std environments.
//!
//! This module provides stack-allocated buffers for encoding telemetry
//! messages without heap allocation.
//!
//! # Traceability
//!
//! - REQ-SER-021: Fixed-size TelemetryBuffer for encoding

use minicbor::{encode, Encode};

/// Error type for telemetry buffer operations.
///
/// This enum represents errors that can occur during buffer operations,
/// particularly during CBOR encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// Buffer capacity exceeded during encoding
    BufferOverflow,
    /// CBOR encoding error
    EncodeError,
}

/// Fixed-size buffer for encoding telemetry messages.
///
/// This structure provides a stack-allocated buffer for encoding messages
/// to CBOR format without requiring heap allocation. It is parameterized
/// by the buffer size at compile time using const generics.
///
/// # Traceability
///
/// - REQ-SER-021: Fixed-size TelemetryBuffer<N>
///
/// # Type Parameters
///
/// - `SIZE`: The size of the buffer in bytes (const generic)
///
/// # Example
///
/// ```
/// use middleware::telemetry::{ControlTelemetry, TelemetryBuffer};
///
/// let mut buffer = TelemetryBuffer::<128>::new();
/// let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
/// buffer.encode(&msg).expect("Failed to encode");
/// let bytes = buffer.as_bytes();
/// assert!(!bytes.is_empty());
///
/// // The bytes are CBOR and decode back to the original message.
/// let decoded: ControlTelemetry = minicbor::decode(bytes).unwrap();
/// assert_eq!(decoded, msg);
/// ```
#[derive(Debug)]
pub struct TelemetryBuffer<const SIZE: usize> {
    data: [u8; SIZE],
    len: usize,
}

impl<const SIZE: usize> TelemetryBuffer<SIZE> {
    /// Creates a new empty TelemetryBuffer.
    ///
    /// # Returns
    ///
    /// A new TelemetryBuffer instance with zero length.
    pub fn new() -> Self {
        Self {
            data: [0u8; SIZE],
            len: 0,
        }
    }

    /// Encodes a message into the buffer.
    ///
    /// This method serializes the given message to CBOR format and stores
    /// it in the internal buffer. The buffer is reset before encoding.
    ///
    /// On success, [`len`](Self::len) is the exact number of CBOR bytes
    /// produced for `msg`, independent of `SIZE`, and
    /// [`as_bytes`](Self::as_bytes) returns exactly those bytes with no
    /// trailing padding. On failure the length is left at zero.
    ///
    /// # Type Parameters
    ///
    /// - `T`: Type that implements minicbor::Encode
    ///
    /// # Arguments
    ///
    /// - `msg`: Reference to the message to encode
    ///
    /// # Returns
    ///
    /// - `Ok(())` if encoding succeeds
    /// - `Err(BufferError)` if encoding fails (buffer overflow or encode error)
    ///
    /// # Traceability
    ///
    /// - REQ-SER-021: encode() method returning Result
    pub fn encode<T: Encode<()>>(&mut self, msg: &T) -> Result<(), BufferError> {
        // Reset buffer
        self.len = 0;

        // minicbor's `Write for &mut [u8]` advances the slice past each write,
        // so after encoding the writer holds the *unwritten* tail of the
        // buffer. Bytes written is therefore capacity minus what remains.
        let mut encoder = encode::Encoder::new(&mut self.data[..]);
        encoder.encode(msg).map_err(|_| BufferError::EncodeError)?;
        let remaining = encoder.writer().len();
        self.len = SIZE.saturating_sub(remaining);

        Ok(())
    }

    /// Returns the encoded data as a byte slice.
    ///
    /// This method returns only the portion of the buffer that contains
    /// valid encoded data, not the entire buffer.
    ///
    /// # Returns
    ///
    /// A byte slice containing the encoded data.
    ///
    /// # Traceability
    ///
    /// - REQ-SER-021: as_bytes() method
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// Returns the length of the encoded data in bytes.
    ///
    /// # Returns
    ///
    /// The number of bytes of encoded data in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Checks if the buffer is empty.
    ///
    /// # Returns
    ///
    /// `true` if the buffer contains no encoded data, `false` otherwise.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the total capacity of the buffer.
    ///
    /// # Returns
    ///
    /// The size of the buffer in bytes.
    pub fn capacity(&self) -> usize {
        SIZE
    }

    /// Clears the buffer.
    ///
    /// This resets the length to zero, effectively discarding any encoded data.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const SIZE: usize> Default for TelemetryBuffer<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::messages::ControlTelemetry;

    #[test]
    fn test_buffer_new() {
        let buffer = TelemetryBuffer::<128>::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 128);
    }

    #[test]
    fn test_buffer_encode() {
        let mut buffer = TelemetryBuffer::<128>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        let result = buffer.encode(&msg);
        assert!(result.is_ok());
        assert!(!buffer.is_empty());
        assert!(buffer.len() > 0);
        assert!(buffer.len() <= 128);
    }

    #[test]
    fn test_buffer_as_bytes() {
        let mut buffer = TelemetryBuffer::<128>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        buffer.encode(&msg).unwrap();
        let bytes = buffer.as_bytes();
        assert_eq!(bytes.len(), buffer.len());
    }

    #[test]
    fn test_buffer_clear() {
        let mut buffer = TelemetryBuffer::<128>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        buffer.encode(&msg).unwrap();
        assert!(!buffer.is_empty());

        buffer.clear();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
    }

    /// Independent oracle: a minicbor writer that only counts the bytes
    /// handed to it, so the expected length does not depend on how
    /// `TelemetryBuffer` measures its own output.
    struct CountingWriter {
        count: usize,
    }

    impl minicbor::encode::Write for CountingWriter {
        type Error = core::convert::Infallible;

        fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
            self.count += buf.len();
            Ok(())
        }
    }

    fn encoded_len<T: Encode<()>>(msg: &T) -> usize {
        let mut writer = CountingWriter { count: 0 };
        let result = minicbor::encode(msg, &mut writer);
        assert!(result.is_ok());
        writer.count
    }

    #[test]
    fn test_len_independent_of_capacity() {
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
        let mut small = TelemetryBuffer::<64>::new();
        let mut large = TelemetryBuffer::<128>::new();
        small.encode(&msg).unwrap();
        large.encode(&msg).unwrap();
        assert_eq!(small.len(), large.len());
        assert_eq!(small.as_bytes(), large.as_bytes());
    }

    #[test]
    fn test_len_matches_bytes_written() {
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
        let mut buffer = TelemetryBuffer::<128>::new();
        buffer.encode(&msg).unwrap();
        assert_eq!(buffer.len(), encoded_len(&msg));
    }

    #[test]
    fn test_as_bytes_has_no_trailing_padding() {
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
        let mut buffer = TelemetryBuffer::<128>::new();
        buffer.encode(&msg).unwrap();
        let bytes = buffer.as_bytes();
        assert_eq!(bytes.len(), encoded_len(&msg));

        // Exactly one CBOR item spans the whole slice: decoding consumes
        // every byte, leaving nothing after the message.
        let mut decoder = minicbor::Decoder::new(bytes);
        let decoded: ControlTelemetry = decoder.decode().unwrap();
        assert_eq!(decoded, msg);
        assert_eq!(decoder.position(), bytes.len());
    }

    #[test]
    fn test_reencode_shorter_message_shrinks_len() {
        let long = ControlTelemetry::new(u64::MAX, 10.0, 9.5, 2.5, 0.5);
        let short = ControlTelemetry::new(0, 10.0, 9.5, 2.5, 0.5);
        let mut buffer = TelemetryBuffer::<128>::new();
        buffer.encode(&long).unwrap();
        assert_eq!(buffer.len(), encoded_len(&long));
        buffer.encode(&short).unwrap();
        assert_eq!(buffer.len(), encoded_len(&short));
        assert!(encoded_len(&short) < encoded_len(&long));
    }

    #[test]
    fn test_buffer_overflow() {
        let mut buffer = TelemetryBuffer::<4>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        let result = buffer.encode(&msg);
        assert!(result.is_err());
    }
}
