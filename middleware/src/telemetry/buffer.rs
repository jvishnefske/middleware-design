//! Fixed-size telemetry buffers for no_std environments.
//!
//! This module provides stack-allocated buffers for encoding telemetry
//! messages without heap allocation.
//!
//! # Traceability
//!
//! - REQ-SER-021: Fixed-size TelemetryBuffer for encoding
//! - REQ-SER-022: Encoding into a too-small buffer returns
//!   `BufferError::BufferOverflow` and leaves the buffer empty

use minicbor::{encode, Encode};

/// Error type for telemetry buffer operations.
///
/// This enum represents errors that can occur during buffer operations,
/// particularly during CBOR encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// Buffer capacity exceeded during encoding: the CBOR writer reached the
    /// end of the `SIZE`-byte buffer before the message was complete.
    BufferOverflow,
    /// CBOR encoding error raised by the message's `Encode` implementation
    /// itself (a custom or message error), not by running out of space.
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
/// - REQ-SER-021: Fixed-size `TelemetryBuffer<N>`
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
    ///
    /// # Errors
    ///
    /// - [`BufferError::BufferOverflow`] if the encoded message does not fit
    ///   in `SIZE` bytes (minicbor reports this as a write error,
    ///   `EndOfSlice`, from the slice writer)
    /// - [`BufferError::EncodeError`] for any other encoding failure reported
    ///   by the message's `Encode` implementation
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
        encoder.encode(msg).map_err(|e| {
            if e.is_write() {
                BufferError::BufferOverflow
            } else {
                BufferError::EncodeError
            }
        })?;
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
        self.data.get(..self.len).unwrap_or_default()
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

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::messages::ControlTelemetry;

    /// Verifies: REQ-SER-021
    #[test]
    fn test_buffer_new() {
        let buffer = TelemetryBuffer::<128>::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.capacity(), 128);
    }

    /// Verifies: REQ-SER-021
    #[test]
    fn test_buffer_encode() {
        let mut buffer = TelemetryBuffer::<128>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        let result = buffer.encode(&msg);
        assert!(result.is_ok());
        assert!(!buffer.is_empty());
        assert!(buffer.len() <= 128);
    }

    /// Verifies: REQ-SER-021
    #[test]
    fn test_buffer_as_bytes() {
        let mut buffer = TelemetryBuffer::<128>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        buffer.encode(&msg).unwrap();
        let bytes = buffer.as_bytes();
        assert_eq!(bytes.len(), buffer.len());
    }

    /// Verifies: REQ-SER-021
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

    /// Verifies: REQ-SER-021
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

    /// Verifies: REQ-SER-021
    #[test]
    fn test_len_matches_bytes_written() {
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
        let mut buffer = TelemetryBuffer::<128>::new();
        buffer.encode(&msg).unwrap();
        assert_eq!(buffer.len(), encoded_len(&msg));
    }

    /// Verifies: REQ-SER-001, REQ-SER-021
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

    /// Verifies: REQ-SER-021
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

    /// Verifies: REQ-SER-022
    #[test]
    fn test_buffer_overflow() {
        let mut buffer = TelemetryBuffer::<4>::new();
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);

        let result = buffer.encode(&msg);
        assert_eq!(result, Err(BufferError::BufferOverflow));
        assert_eq!(buffer.len(), 0);
        assert!(buffer.as_bytes().is_empty());
    }

    /// Verifies: REQ-SER-021, REQ-SER-022
    #[test]
    fn test_buffer_overflow_boundary() {
        let msg = ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5);
        let needed = encoded_len(&msg);
        assert!(needed > 8 && needed <= 64);

        // Ample capacity succeeds with exactly `needed` bytes.
        let mut ample = TelemetryBuffer::<64>::new();
        assert_eq!(ample.encode(&msg), Ok(()));
        assert_eq!(ample.len(), needed);

        // Too little capacity is reported as overflow, not a generic encode
        // error, and leaves the buffer empty even after a prior success.
        let mut small = TelemetryBuffer::<8>::new();
        assert_eq!(small.encode(&msg), Err(BufferError::BufferOverflow));
        assert!(small.is_empty());

        // The underlying minicbor slice writer signals one byte short as a
        // write error, which is what `encode` maps to `BufferOverflow`.
        let mut scratch = [0u8; 64];
        let short = scratch.get_mut(..needed - 1).unwrap();
        assert!(minicbor::encode(msg, short).is_err_and(|e| e.is_write()));
    }

    /// Verifies: REQ-SER-022
    #[test]
    fn test_overflow_after_success_resets_len() {
        let short = ControlTelemetry::new(0, 0.0, 0.0, 0.0, 0.0);
        let long = ControlTelemetry::new(u64::MAX, 10.0, 9.5, 2.5, 0.5);
        // `long` needs more bytes than `short` (wider timestamp and floats).
        let mut buffer = TelemetryBuffer::<26>::new();
        assert!(encoded_len(&short) <= 26 && encoded_len(&long) > 26);
        buffer.encode(&short).unwrap();
        assert!(!buffer.is_empty());
        assert_eq!(buffer.encode(&long), Err(BufferError::BufferOverflow));
        assert!(buffer.is_empty());
    }

    /// A message whose `Encode` implementation fails on its own, without
    /// touching the writer, so the error is not a write error.
    struct FailingMessage;

    impl Encode<()> for FailingMessage {
        fn encode<W: minicbor::encode::Write>(
            &self,
            _e: &mut minicbor::Encoder<W>,
            _ctx: &mut (),
        ) -> Result<(), minicbor::encode::Error<W::Error>> {
            Err(minicbor::encode::Error::message("deliberate failure"))
        }
    }

    /// Verifies: REQ-SER-021
    #[test]
    fn test_non_write_error_is_encode_error() {
        let mut buffer = TelemetryBuffer::<128>::new();
        assert_eq!(
            buffer.encode(&FailingMessage),
            Err(BufferError::EncodeError)
        );
        assert!(buffer.is_empty());
    }

    /// Encoded size of `EXACT_MSG`: array head (1) + `u16` timestamp 1000
    /// (3) + four `f32` values (5 each). Checked against the counting oracle.
    const EXACT_LEN: usize = 24;

    fn exact_msg() -> ControlTelemetry {
        ControlTelemetry::new(1000, 10.0, 9.5, 2.5, 0.5)
    }

    /// Boundary: a buffer whose capacity is exactly the encoded size holds
    /// the message completely (`len == SIZE`, bytes decode back).
    /// Verifies: REQ-SER-021
    #[test]
    fn test_encode_at_exact_capacity() {
        let msg = exact_msg();
        assert_eq!(encoded_len(&msg), EXACT_LEN);
        let mut buffer = TelemetryBuffer::<EXACT_LEN>::new();
        assert_eq!(buffer.encode(&msg), Ok(()));
        assert_eq!(buffer.len(), EXACT_LEN);
        assert_eq!(buffer.len(), buffer.capacity());
        let decoded: ControlTelemetry = minicbor::decode(buffer.as_bytes()).unwrap();
        assert_eq!(decoded, msg);
    }

    /// Boundary: one byte below the encoded size (`SIZE = N - 1`) is an
    /// overflow and leaves the buffer empty, also when it held a previous
    /// message that did fit.
    /// Verifies: REQ-SER-022
    #[test]
    fn test_encode_one_byte_short_overflows() {
        let msg = exact_msg();
        assert_eq!(encoded_len(&msg), EXACT_LEN);
        let mut buffer = TelemetryBuffer::<{ EXACT_LEN - 1 }>::new();
        let fits = ControlTelemetry::new(0, 0.0, 0.0, 0.0, 0.0);
        assert!(encoded_len(&fits) < EXACT_LEN - 1);
        buffer.encode(&fits).unwrap();
        assert!(!buffer.is_empty());
        assert_eq!(buffer.encode(&msg), Err(BufferError::BufferOverflow));
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert!(buffer.as_bytes().is_empty());
    }

    /// Boundary: a zero-capacity buffer cannot hold even a one-byte message
    /// and reports overflow with an empty buffer.
    /// Verifies: REQ-SER-021, REQ-SER-022
    #[test]
    fn test_zero_capacity_buffer() {
        let mut buffer = TelemetryBuffer::<0>::new();
        assert_eq!(buffer.capacity(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.encode(&0u8), Err(BufferError::BufferOverflow));
        assert!(buffer.is_empty());
        assert!(buffer.as_bytes().is_empty());
    }

    /// Fixed-size storage: `TelemetryBuffer<N>` is a plain value whose size
    /// is a compile-time constant of at least `N` bytes (the inline array),
    /// with capacity `N`, and `Default` gives the same empty buffer as `new`.
    /// Verifies: REQ-SER-021
    #[test]
    fn test_fixed_size_storage_and_default() {
        // Evaluated at compile time: a violation fails the build.
        const SMALL: usize = core::mem::size_of::<TelemetryBuffer<16>>();
        const LARGE: usize = core::mem::size_of::<TelemetryBuffer<256>>();
        const _: () = assert!(SMALL >= 16 && LARGE >= 256);
        assert_eq!(LARGE - SMALL, 240);
        let buffer: TelemetryBuffer<16> = TelemetryBuffer::default();
        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), 16);
        assert!(buffer.as_bytes().is_empty());
    }
}
