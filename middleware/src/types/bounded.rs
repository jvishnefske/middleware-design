//! # Bounded Collections
//!
//! Fixed-capacity collections for no_std environments.
//!
//! # Traceability
//!
//! - REQ-TYP-020: BoundedVec for fixed-capacity vectors
//! - REQ-TYP-021: BoundedString for fixed-capacity strings
//! - REQ-TYP-022: Result on capacity overflow

use crate::error::SerializeError;

/// Fixed-capacity vector without heap allocation.
///
/// # Traceability
///
/// - REQ-TYP-020: Fixed-capacity vector
/// - REQ-TYP-022: Returns Result on overflow
/// - REQ-PLT-002: No heap allocation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundedVec<T, const N: usize> {
    data: [Option<T>; N],
    len: usize,
}

impl<T, const N: usize> BoundedVec<T, N> {
    /// Create a new empty bounded vector.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec construction
    pub const fn new() -> Self
    where
        T: Copy,
    {
        Self {
            data: [None; N],
            len: 0,
        }
    }

    /// Push an element to the end of the vector.
    ///
    /// Returns an error if the capacity is exceeded.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-022: Result on overflow
    pub fn push(&mut self, value: T) -> Result<(), SerializeError> {
        if self.len >= N {
            return Err(SerializeError::BufferTooSmall);
        }
        self.data[self.len] = Some(value);
        self.len += 1;
        Ok(())
    }

    /// Pop an element from the end of the vector.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.data[self.len].take()
    }

    /// Get the current length of the vector.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Check if the vector is empty.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the capacity of the vector.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Clear all elements from the vector.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub fn clear(&mut self) {
        for i in 0..self.len {
            self.data[i] = None;
        }
        self.len = 0;
    }

    /// Get a reference to an element at the given index.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            self.data[index].as_ref()
        } else {
            None
        }
    }

    /// Get a mutable reference to an element at the given index.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            self.data[index].as_mut()
        } else {
            None
        }
    }

    /// Get an iterator over the elements.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data[..self.len].iter().filter_map(|x| x.as_ref())
    }

    /// Get a mutable iterator over the elements.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-020: BoundedVec operations
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.data[..self.len].iter_mut().filter_map(|x| x.as_mut())
    }
}

impl<T: Copy, const N: usize> Default for BoundedVec<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Fixed-capacity string without heap allocation.
///
/// # Traceability
///
/// - REQ-TYP-021: Fixed-capacity string
/// - REQ-TYP-022: Returns Result on overflow
/// - REQ-PLT-002: No heap allocation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundedString<const N: usize> {
    data: [u8; N],
    len: usize,
}

impl<const N: usize> BoundedString<N> {
    /// Create a new empty bounded string.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString construction
    pub const fn new() -> Self {
        Self {
            data: [0; N],
            len: 0,
        }
    }

    /// Create a bounded string from a string slice.
    ///
    /// Returns an error if the string is too long.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-022: Result on overflow
    pub fn from_str(s: &str) -> Result<Self, SerializeError> {
        let bytes = s.as_bytes();
        if bytes.len() > N {
            return Err(SerializeError::BufferTooSmall);
        }
        let mut result = Self::new();
        result.data[..bytes.len()].copy_from_slice(bytes);
        result.len = bytes.len();
        Ok(result)
    }

    /// Push a character to the end of the string.
    ///
    /// Returns an error if the capacity is exceeded.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-022: Result on overflow
    pub fn push(&mut self, ch: char) -> Result<(), SerializeError> {
        let mut buf = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut buf);
        let ch_bytes = ch_str.as_bytes();

        if self.len + ch_bytes.len() > N {
            return Err(SerializeError::BufferTooSmall);
        }

        self.data[self.len..self.len + ch_bytes.len()].copy_from_slice(ch_bytes);
        self.len += ch_bytes.len();
        Ok(())
    }

    /// Push a string slice to the end of the string.
    ///
    /// Returns an error if the capacity is exceeded.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-022: Result on overflow
    pub fn push_str(&mut self, s: &str) -> Result<(), SerializeError> {
        let bytes = s.as_bytes();
        if self.len + bytes.len() > N {
            return Err(SerializeError::BufferTooSmall);
        }

        self.data[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }

    /// Get the current length of the string in bytes.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString operations
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Check if the string is empty.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString operations
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the capacity of the string in bytes.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString operations
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Clear the string.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString operations
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Get the string as a string slice.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString operations
    pub fn as_str(&self) -> &str {
        // Safety: We only insert valid UTF-8 via push and push_str
        core::str::from_utf8(&self.data[..self.len]).unwrap()
    }

    /// Get the string as a byte slice.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-021: BoundedString operations
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

impl<const N: usize> Default for BoundedString<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> core::fmt::Display for BoundedString<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;

    #[test]
    fn test_bounded_vec_new() {
        let vec: BoundedVec<i32, 10> = BoundedVec::new();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 10);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_bounded_vec_push() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();
        assert!(vec.push(1).is_ok());
        assert!(vec.push(2).is_ok());
        assert!(vec.push(3).is_ok());
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.push(4), Err(SerializeError::BufferTooSmall));
    }

    #[test]
    fn test_bounded_vec_pop() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.pop(), None);
    }

    #[test]
    fn test_bounded_vec_get() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();
        vec.push(10).unwrap();
        vec.push(20).unwrap();
        assert_eq!(vec.get(0), Some(&10));
        assert_eq!(vec.get(1), Some(&20));
        assert_eq!(vec.get(2), None);
    }

    #[test]
    fn test_bounded_vec_get_mut() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();
        vec.push(10).unwrap();
        if let Some(val) = vec.get_mut(0) {
            *val = 99;
        }
        assert_eq!(vec.get(0), Some(&99));
    }

    #[test]
    fn test_bounded_vec_clear() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        vec.clear();
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_bounded_vec_iter() {
        let mut vec: BoundedVec<i32, 3> = BoundedVec::new();
        vec.push(1).unwrap();
        vec.push(2).unwrap();
        vec.push(3).unwrap();
        let sum: i32 = vec.iter().sum();
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_bounded_string_new() {
        let s: BoundedString<10> = BoundedString::new();
        assert_eq!(s.len(), 0);
        assert_eq!(s.capacity(), 10);
        assert!(s.is_empty());
    }

    #[test]
    fn test_bounded_string_from_str() {
        let s: BoundedString<10> = BoundedString::from_str("hello").unwrap();
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.len(), 5);

        let result: Result<BoundedString<3>, _> = BoundedString::from_str("hello");
        assert_eq!(result, Err(SerializeError::BufferTooSmall));
    }

    #[test]
    fn test_bounded_string_push() {
        let mut s: BoundedString<10> = BoundedString::new();
        assert!(s.push('a').is_ok());
        assert!(s.push('b').is_ok());
        assert_eq!(s.as_str(), "ab");
    }

    #[test]
    fn test_bounded_string_push_str() {
        let mut s: BoundedString<10> = BoundedString::new();
        assert!(s.push_str("hello").is_ok());
        assert_eq!(s.as_str(), "hello");
        assert_eq!(s.push_str(" world"), Err(SerializeError::BufferTooSmall));
    }

    #[test]
    fn test_bounded_string_clear() {
        let mut s: BoundedString<10> = BoundedString::from_str("hello").unwrap();
        s.clear();
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
    }

    #[test]
    fn test_bounded_string_display() {
        let s: BoundedString<10> = BoundedString::from_str("test").unwrap();
        let display_str = std::format!("{}", s);
        assert_eq!(display_str, "test");
    }

    #[test]
    fn test_bounded_string_unicode() {
        let mut s: BoundedString<20> = BoundedString::new();
        assert!(s.push('λ').is_ok());
        assert!(s.push_str("🦀").is_ok());
        assert!(s.as_str().contains('λ'));
        assert!(s.as_str().contains('🦀'));
    }
}
