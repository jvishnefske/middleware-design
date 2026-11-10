//! # Typed Primitive Wrappers
//!
//! Newtype wrappers for primitive types with unit parameters.
//!
//! # Traceability
//!
//! - REQ-TYP-010: Newtype wrappers with unit parameters
//! - REQ-TYP-011: Support for all primitive types
//! - REQ-TYP-012: From/Into conversions

use crate::types::unit::Unit;
use core::ops::{Add, Sub, Mul, Div, Neg};

/// Generic quantity with type and unit parameters.
///
/// This is a zero-cost wrapper that provides compile-time dimensional analysis.
///
/// # Traceability
///
/// - REQ-TYP-010: Newtype wrapper
/// - REQ-TYP-002: #[repr(transparent)] for zero-cost
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Quantity<T, U: Unit> {
    value: T,
    _unit: core::marker::PhantomData<U>,
}

impl<T, U: Unit> Quantity<T, U> {
    /// Create a new quantity with the given value and unit.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-010: Newtype construction
    pub const fn new(value: T) -> Self {
        Self {
            value,
            _unit: core::marker::PhantomData,
        }
    }

    /// Extract the raw value.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-012: Value extraction
    pub fn value(self) -> T {
        self.value
    }
}

impl<T, U: Unit> From<T> for Quantity<T, U> {
    /// Create a quantity from a raw value.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-012: From conversion
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T, U: Unit> Quantity<T, U> {
    /// Convert the quantity into its raw value.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-012: Into conversion
    pub fn into_inner(self) -> T {
        self.value
    }
}

// Arithmetic operations for numeric types
impl<T: Add<Output = T>, U: Unit> Add for Quantity<T, U> {
    type Output = Self;

    /// Add two quantities with the same unit.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-003: Unit-safe arithmetic
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.value + rhs.value)
    }
}

impl<T: Sub<Output = T>, U: Unit> Sub for Quantity<T, U> {
    type Output = Self;

    /// Subtract two quantities with the same unit.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-003: Unit-safe arithmetic
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.value - rhs.value)
    }
}

impl<T: Mul<Output = T> + Copy, U: Unit> Mul<T> for Quantity<T, U> {
    type Output = Self;

    /// Multiply a quantity by a scalar.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-003: Unit-safe arithmetic
    fn mul(self, rhs: T) -> Self::Output {
        Self::new(self.value * rhs)
    }
}

impl<T: Div<Output = T> + Copy, U: Unit> Div<T> for Quantity<T, U> {
    type Output = Self;

    /// Divide a quantity by a scalar.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-003: Unit-safe arithmetic
    fn div(self, rhs: T) -> Self::Output {
        Self::new(self.value / rhs)
    }
}

impl<T: Neg<Output = T>, U: Unit> Neg for Quantity<T, U> {
    type Output = Self;

    /// Negate a quantity.
    ///
    /// # Traceability
    ///
    /// - REQ-TYP-003: Unit-safe arithmetic
    fn neg(self) -> Self::Output {
        Self::new(-self.value)
    }
}

// Type aliases for common primitive/unit combinations

/// 32-bit floating point quantity
///
/// # Traceability
///
/// - REQ-TYP-011: f32 support
pub type F32<U> = Quantity<f32, U>;

/// 64-bit floating point quantity
///
/// # Traceability
///
/// - REQ-TYP-011: f64 support
pub type F64<U> = Quantity<f64, U>;

/// 32-bit signed integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: i32 support
pub type I32<U> = Quantity<i32, U>;

/// 32-bit unsigned integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: u32 support
pub type U32<U> = Quantity<u32, U>;

/// 16-bit signed integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: i16 support
pub type I16<U> = Quantity<i16, U>;

/// 16-bit unsigned integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: u16 support
pub type U16<U> = Quantity<u16, U>;

/// 8-bit signed integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: i8 support
pub type I8<U> = Quantity<i8, U>;

/// 8-bit unsigned integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: u8 support
pub type U8<U> = Quantity<u8, U>;

/// 64-bit signed integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: i64 support
pub type I64<U> = Quantity<i64, U>;

/// 64-bit unsigned integer quantity
///
/// # Traceability
///
/// - REQ-TYP-011: u64 support
pub type U64<U> = Quantity<u64, U>;

/// Boolean quantity
///
/// # Traceability
///
/// - REQ-TYP-011: bool support
pub type Bool<U> = Quantity<bool, U>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::unit::{Seconds, MetersPerSecond};

    extern crate std;

    #[test]
    fn test_quantity_new() {
        let time = F32::<Seconds>::new(1.5);
        assert_eq!(time.value(), 1.5);
    }

    #[test]
    fn test_quantity_from() {
        let time: F32<Seconds> = 2.5.into();
        assert_eq!(time.value(), 2.5);
    }

    #[test]
    fn test_quantity_into_inner() {
        let time = F32::<Seconds>::new(3.5);
        let value = time.into_inner();
        assert_eq!(value, 3.5);
    }

    #[test]
    fn test_quantity_add() {
        let t1 = F32::<Seconds>::new(1.0);
        let t2 = F32::<Seconds>::new(2.0);
        let sum = t1 + t2;
        assert_eq!(sum.value(), 3.0);
    }

    #[test]
    fn test_quantity_sub() {
        let t1 = F32::<Seconds>::new(5.0);
        let t2 = F32::<Seconds>::new(2.0);
        let diff = t1 - t2;
        assert_eq!(diff.value(), 3.0);
    }

    #[test]
    fn test_quantity_mul_scalar() {
        let time = F32::<Seconds>::new(2.0);
        let scaled = time * 3.0;
        assert_eq!(scaled.value(), 6.0);
    }

    #[test]
    fn test_quantity_div_scalar() {
        let time = F32::<Seconds>::new(6.0);
        let scaled = time / 2.0;
        assert_eq!(scaled.value(), 3.0);
    }

    #[test]
    fn test_quantity_neg() {
        let vel = F32::<MetersPerSecond>::new(5.0);
        let neg_vel = -vel;
        assert_eq!(neg_vel.value(), -5.0);
    }

    #[test]
    fn test_integer_quantity() {
        let count = I32::<crate::types::unit::Dimensionless>::new(42);
        assert_eq!(count.value(), 42);
    }

    #[test]
    fn test_quantity_clone() {
        let t1 = F32::<Seconds>::new(1.5);
        let t2 = t1.clone();
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_quantity_debug() {
        let time = F32::<Seconds>::new(1.5);
        let debug_str = std::format!("{:?}", time);
        assert!(debug_str.contains("1.5"));
    }
}
