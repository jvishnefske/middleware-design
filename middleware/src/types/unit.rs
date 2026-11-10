//! # Physical Unit Types
//!
//! Zero-cost type-safe physical unit markers for dimensional analysis.
//!
//! # Traceability
//!
//! - REQ-TYP-001: Type-safe physical unit wrappers
//! - REQ-TYP-002: Zero-cost abstractions using #[repr(transparent)]
//! - REQ-TYP-003: Compile-time dimensional correctness
//! - REQ-TYP-004: Copy, Clone, Debug, PartialEq, PartialOrd implementations

/// Sealed trait for unit markers.
///
/// This trait cannot be implemented outside this module, ensuring
/// that only valid unit types can be used with `Quantity<T, U>`.
///
/// # Traceability
///
/// - REQ-TYP-001: Type-safe unit system
pub trait Unit: private::Sealed + Copy + Clone + core::fmt::Debug + Default {}

mod private {
    pub trait Sealed {}
}

/// Time unit: seconds
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Seconds;

impl private::Sealed for Seconds {}
impl Unit for Seconds {}

/// Time unit: milliseconds
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Milliseconds;

impl private::Sealed for Milliseconds {}
impl Unit for Milliseconds {}

/// Angle unit: radians
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Radians;

impl private::Sealed for Radians {}
impl Unit for Radians {}

/// Angle unit: degrees
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Degrees;

impl private::Sealed for Degrees {}
impl Unit for Degrees {}

/// Velocity unit: meters per second
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MetersPerSecond;

impl private::Sealed for MetersPerSecond {}
impl Unit for MetersPerSecond {}

/// Angular velocity unit: radians per second
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct RadiansPerSecond;

impl private::Sealed for RadiansPerSecond {}
impl Unit for RadiansPerSecond {}

/// Dimensionless quantity (scalar)
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Dimensionless;

impl private::Sealed for Dimensionless {}
impl Unit for Dimensionless {}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;

    #[test]
    fn test_unit_default() {
        // Exercise `Default` through the `Unit` bound, as generic code does.
        fn default_unit<U: Unit>() -> U {
            U::default()
        }
        assert_eq!(default_unit::<Seconds>(), Seconds);
        assert_eq!(default_unit::<Milliseconds>(), Milliseconds);
        assert_eq!(default_unit::<Radians>(), Radians);
        assert_eq!(default_unit::<Degrees>(), Degrees);
        assert_eq!(default_unit::<MetersPerSecond>(), MetersPerSecond);
        assert_eq!(default_unit::<RadiansPerSecond>(), RadiansPerSecond);
        assert_eq!(default_unit::<Dimensionless>(), Dimensionless);
    }

    #[test]
    fn test_unit_clone() {
        let s1 = Seconds;
        let s2 = Clone::clone(&s1);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_unit_debug() {
        let s = Seconds;
        let debug_str = std::format!("{:?}", s);
        assert!(debug_str.contains("Seconds"));
    }
}
