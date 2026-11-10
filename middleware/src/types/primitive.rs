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
use core::ops::{Add, Div, Mul, Neg, Sub};

/// Generic quantity with type and unit parameters.
///
/// This is a zero-cost wrapper that provides compile-time dimensional
/// analysis. It is `#[repr(transparent)]` over `T`: the unit is a
/// zero-sized `PhantomData`, so `Quantity<T, U>` has the same size,
/// alignment and ABI as `T`.
///
/// # Dimensional checking
///
/// The unit is part of the type, so the compiler rejects mixing units:
///
/// - `Add`, `Sub`, comparison (`PartialEq`, `PartialOrd`) are defined only
///   between two quantities of the same `T` and the same `U`, and the
///   result keeps that unit.
/// - `Mul` and `Div` are defined only with a bare scalar `T` and keep the
///   unit. There is no quantity-by-quantity multiply or divide, so derived
///   units (for example metres per second times seconds) are not formed;
///   such a product must be computed on raw values and re-wrapped
///   explicitly.
/// - `Neg` keeps the unit.
///
/// Integer quantities use the inner type's operators unchanged, so they
/// overflow exactly as `T` does (a panic in debug builds, wrapping in
/// release builds).
///
/// Same-unit arithmetic compiles:
///
/// ```
/// use middleware::types::{F32, Seconds};
/// let total: F32<Seconds> = F32::new(1.0) + F32::new(2.0);
/// assert!(total > F32::new(2.5));
/// ```
///
/// Adding different units does not compile:
///
/// ```compile_fail,E0308
/// use middleware::types::{F32, Radians, Seconds};
/// let _ = F32::<Seconds>::new(1.0) + F32::<Radians>::new(2.0);
/// ```
///
/// Comparing different units does not compile:
///
/// ```compile_fail,E0308
/// use middleware::types::{F32, Radians, Seconds};
/// let _ = F32::<Seconds>::new(1.0) < F32::<Radians>::new(2.0);
/// ```
///
/// Multiplying two quantities does not compile:
///
/// ```compile_fail,E0308
/// use middleware::types::{F32, MetersPerSecond, Seconds};
/// let _ = F32::<MetersPerSecond>::new(1.0) * F32::<Seconds>::new(2.0);
/// ```
///
/// # Traceability
///
/// - REQ-TYP-010: Newtype wrapper
/// - REQ-TYP-002: #[repr(transparent)] for zero-cost
/// - REQ-TYP-003: Compile-time dimensional correctness
/// - REQ-TYP-004: Copy, Clone, Debug, PartialEq, PartialOrd
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

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::float_cmp))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;
    use crate::types::unit::{
        Degrees, Dimensionless, MetersPerSecond, Milliseconds, Radians, RadiansPerSecond, Seconds,
    };
    use core::any::TypeId;
    use core::cmp::Ordering;
    use core::mem::{align_of, size_of};

    extern crate std;

    /// Asserts `Quantity<T, U>` has exactly the layout of `T`.
    fn assert_same_layout<T, U: Unit>() {
        assert_eq!(size_of::<Quantity<T, U>>(), size_of::<T>());
        assert_eq!(align_of::<Quantity<T, U>>(), align_of::<T>());
    }

    /// Returns the type id of the unit carried by a quantity. The unit is
    /// inferred from the argument's static type, so this observes the type
    /// the compiler assigned to an expression.
    fn unit_of<T, U: Unit + 'static>(_q: Quantity<T, U>) -> TypeId {
        TypeId::of::<U>()
    }

    /// Compiles only if `Q` has every trait REQ-TYP-004 requires.
    fn has_required_traits<Q>(_value: &Q) -> bool
    where
        Q: Copy + Clone + core::fmt::Debug + PartialEq + PartialOrd,
    {
        true
    }

    /// Converts through `From` / `Into` generically, as caller code would.
    fn via_into<T, Q: From<T>>(raw: T) -> Q {
        raw.into()
    }

    /// `Quantity` is `repr(transparent)`: size and alignment equal those of
    /// the wrapped primitive for every supported primitive, independently of
    /// the unit. The first two checks are `const` and so fail the build.
    ///
    /// Verifies: REQ-TYP-002
    #[test]
    fn test_quantity_layout_matches_inner_primitive() {
        const _: () = assert!(size_of::<F32<Seconds>>() == size_of::<f32>());
        const _: () = assert!(align_of::<F64<Radians>>() == align_of::<f64>());

        assert_same_layout::<i8, Seconds>();
        assert_same_layout::<i16, Milliseconds>();
        assert_same_layout::<i32, Radians>();
        assert_same_layout::<i64, Degrees>();
        assert_same_layout::<u8, MetersPerSecond>();
        assert_same_layout::<u16, RadiansPerSecond>();
        assert_same_layout::<u32, Dimensionless>();
        assert_same_layout::<u64, Seconds>();
        assert_same_layout::<f32, Radians>();
        assert_same_layout::<f64, MetersPerSecond>();
        assert_same_layout::<bool, Dimensionless>();
        // Also with a composite inner type, to show nothing is padded in.
        assert_same_layout::<[u8; 3], Seconds>();
    }

    /// Same-unit Add/Sub/Neg and scalar Mul/Div are the only arithmetic
    /// operations defined, and each result carries the operand's unit. The
    /// type ascriptions are checked by the compiler; `unit_of` confirms the
    /// unit at run time. Mixed-unit expressions are rejected at compile time
    /// (see the `compile_fail` examples on [`Quantity`]).
    ///
    /// Verifies: REQ-TYP-003
    #[test]
    fn test_arithmetic_preserves_unit() {
        let a = F32::<MetersPerSecond>::new(6.0);
        let b = F32::<MetersPerSecond>::new(2.0);
        let mps = TypeId::of::<MetersPerSecond>();

        let sum: F32<MetersPerSecond> = a + b;
        let diff: F32<MetersPerSecond> = a - b;
        let scaled: F32<MetersPerSecond> = a * 0.5;
        let divided: F32<MetersPerSecond> = a / 3.0;
        let negated: F32<MetersPerSecond> = -a;

        assert_eq!(unit_of(sum), mps);
        assert_eq!(unit_of(diff), mps);
        assert_eq!(unit_of(scaled), mps);
        assert_eq!(unit_of(divided), mps);
        assert_eq!(unit_of(negated), mps);
        assert_ne!(unit_of(sum), TypeId::of::<Seconds>());

        assert_close(sum.value(), 8.0);
        assert_close(diff.value(), 4.0);
        assert_close(scaled.value(), 3.0);
        assert_close(divided.value(), 2.0);
        assert_close(negated.value(), -6.0);

        // Integer quantities follow the same rules.
        let ticks = I32::<Milliseconds>::new(10) + I32::new(-3);
        assert_eq!(unit_of(ticks), TypeId::of::<Milliseconds>());
        assert_eq!(ticks.value(), 7);
        assert_eq!((ticks * 3).value(), 21);
        assert_eq!((ticks / 2).value(), 3);
        assert_eq!((-ticks).value(), -7);
    }

    /// `Quantity` implements Copy, Clone, Debug, PartialEq and PartialOrd for
    /// every unit, and the comparisons order by value. `NaN` is unordered,
    /// following the inner `f32`.
    ///
    /// Verifies: REQ-TYP-004
    #[test]
    fn test_quantity_has_required_traits() {
        assert!(has_required_traits(&F32::<Seconds>::new(0.0)));
        assert!(has_required_traits(&F64::<Milliseconds>::new(0.0)));
        assert!(has_required_traits(&I32::<Radians>::new(0)));
        assert!(has_required_traits(&U8::<Degrees>::new(0)));
        assert!(has_required_traits(&I16::<MetersPerSecond>::new(0)));
        assert!(has_required_traits(&U64::<RadiansPerSecond>::new(0)));
        assert!(has_required_traits(&Bool::<Dimensionless>::new(false)));

        let low = F32::<Seconds>::new(1.0);
        let high = F32::<Seconds>::new(2.0);
        let copy = low;
        assert_eq!(low, copy);
        assert!(low < high);
        assert!(high > low);
        assert_eq!(low.partial_cmp(&high), Some(Ordering::Less));
        assert_eq!(low.partial_cmp(&copy), Some(Ordering::Equal));
        let nan = F32::<Seconds>::new(f32::NAN);
        assert_eq!(nan.partial_cmp(&low), None);
        let nan_copy = nan;
        assert_ne!(nan, nan_copy);
        assert!(I32::<Radians>::new(i32::MIN) < I32::new(i32::MAX));
        assert!(std::format!("{:?}", I32::<Radians>::new(-4)).contains("-4"));
    }

    /// `Quantity<T, U>` wraps a value of `T` tagged with unit `U`: the value
    /// round-trips unchanged, construction works in `const` context, and the
    /// same `T` with different units yields distinct types.
    ///
    /// Verifies: REQ-TYP-010
    #[test]
    fn test_newtype_with_unit_parameter() {
        const PERIOD: F32<Seconds> = Quantity::new(0.01);
        assert_eq!(PERIOD.value(), 0.01);

        assert_ne!(
            TypeId::of::<F32<Seconds>>(),
            TypeId::of::<F32<Milliseconds>>()
        );
        assert_ne!(TypeId::of::<F32<Radians>>(), TypeId::of::<F32<Degrees>>());
        assert_eq!(
            TypeId::of::<F32<Seconds>>(),
            TypeId::of::<Quantity<f32, Seconds>>()
        );
        assert_eq!(unit_of(F64::<Degrees>::new(90.0)), TypeId::of::<Degrees>());
    }

    /// Every listed primitive has an alias over `Quantity`, and its boundary
    /// values (MIN/MAX, both booleans, IEEE special values) are stored and
    /// returned bit-exactly.
    ///
    /// Verifies: REQ-TYP-011
    #[test]
    fn test_all_primitive_aliases_boundary_values() {
        assert_eq!(I8::<Dimensionless>::new(i8::MIN).value(), i8::MIN);
        assert_eq!(I8::<Dimensionless>::new(i8::MAX).value(), i8::MAX);
        assert_eq!(I16::<Dimensionless>::new(i16::MIN).value(), i16::MIN);
        assert_eq!(I16::<Dimensionless>::new(i16::MAX).value(), i16::MAX);
        assert_eq!(I32::<Dimensionless>::new(i32::MIN).value(), i32::MIN);
        assert_eq!(I32::<Dimensionless>::new(i32::MAX).value(), i32::MAX);
        assert_eq!(I64::<Dimensionless>::new(i64::MIN).value(), i64::MIN);
        assert_eq!(I64::<Dimensionless>::new(i64::MAX).value(), i64::MAX);
        assert_eq!(U8::<Dimensionless>::new(u8::MIN).value(), u8::MIN);
        assert_eq!(U8::<Dimensionless>::new(u8::MAX).value(), u8::MAX);
        assert_eq!(U16::<Dimensionless>::new(u16::MIN).value(), u16::MIN);
        assert_eq!(U16::<Dimensionless>::new(u16::MAX).value(), u16::MAX);
        assert_eq!(U32::<Dimensionless>::new(u32::MIN).value(), u32::MIN);
        assert_eq!(U32::<Dimensionless>::new(u32::MAX).value(), u32::MAX);
        assert_eq!(U64::<Dimensionless>::new(u64::MIN).value(), u64::MIN);
        assert_eq!(U64::<Dimensionless>::new(u64::MAX).value(), u64::MAX);
        assert!(Bool::<Dimensionless>::new(true).value());
        assert!(!Bool::<Dimensionless>::new(false).value());

        assert_eq!(F32::<Seconds>::new(f32::MAX).value(), f32::MAX);
        assert_eq!(
            F32::<Seconds>::new(f32::MIN_POSITIVE).value(),
            f32::MIN_POSITIVE
        );
        assert_eq!(F32::<Seconds>::new(f32::INFINITY).value(), f32::INFINITY);
        assert!(F32::<Seconds>::new(f32::NAN).value().is_nan());
        assert_eq!(
            F32::<Seconds>::new(-0.0).value().to_bits(),
            (-0.0f32).to_bits()
        );
        assert_eq!(F64::<Seconds>::new(f64::MIN).value(), f64::MIN);
        assert_eq!(
            F64::<Seconds>::new(f64::NEG_INFINITY).value(),
            f64::NEG_INFINITY
        );
        assert!(F64::<Seconds>::new(f64::NAN).value().is_nan());
    }

    /// `From<T>` builds a quantity, `T::into()` does the same through the
    /// blanket `Into`, and `into_inner` / `value` convert back losslessly.
    ///
    /// Verifies: REQ-TYP-012
    #[test]
    fn test_from_into_conversions() {
        let from: U16<Milliseconds> = Quantity::from(u16::MAX);
        assert_eq!(from.into_inner(), u16::MAX);

        let into: I64<Seconds> = i64::MIN.into();
        assert_eq!(into.value(), i64::MIN);

        let generic: F64<Radians> = via_into(core::f64::consts::PI);
        assert_eq!(generic.into_inner(), core::f64::consts::PI);

        let flag: Bool<Dimensionless> = via_into(true);
        assert!(flag.into_inner());

        let round_trip: f32 = F32::<Seconds>::from(2.5).into_inner();
        assert_eq!(round_trip, 2.5);
    }

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
        assert_close(sum.value(), 3.0);
    }

    #[test]
    fn test_quantity_sub() {
        let t1 = F32::<Seconds>::new(5.0);
        let t2 = F32::<Seconds>::new(2.0);
        let diff = t1 - t2;
        assert_close(diff.value(), 3.0);
    }

    #[test]
    fn test_quantity_mul_scalar() {
        let time = F32::<Seconds>::new(2.0);
        let scaled = time * 3.0;
        assert_close(scaled.value(), 6.0);
    }

    #[test]
    fn test_quantity_div_scalar() {
        let time = F32::<Seconds>::new(6.0);
        let scaled = time / 2.0;
        assert_close(scaled.value(), 3.0);
    }

    #[test]
    fn test_quantity_neg() {
        let vel = F32::<MetersPerSecond>::new(5.0);
        let neg_vel = -vel;
        assert_close(neg_vel.value(), -5.0);
    }

    #[test]
    fn test_integer_quantity() {
        let count = I32::<Dimensionless>::new(42);
        assert_eq!(count.value(), 42);
    }

    #[test]
    fn test_quantity_clone() {
        let t1 = F32::<Seconds>::new(1.5);
        let t2 = Clone::clone(&t1);
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_quantity_debug() {
        let time = F32::<Seconds>::new(1.5);
        let debug_str = std::format!("{:?}", time);
        assert!(debug_str.contains("1.5"));
    }
}
