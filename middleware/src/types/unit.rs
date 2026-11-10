//! # Physical Unit Types
//!
//! Zero-cost type-safe physical unit markers for dimensional analysis.
//!
//! Each marker is a zero-sized type that derives `Copy`, `Clone`, `Debug`,
//! `Default`, `PartialEq`, `Eq`, `PartialOrd` and `Ord`. Markers carry no data,
//! so every value of a marker type compares equal to every other.
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
/// Every unit marker is a zero-sized type. The `PartialEq` and `PartialOrd`
/// supertraits are required because `Quantity<T, U>` derives those traits,
/// and a derive only applies when every type parameter (including `U`)
/// implements the trait. Without them a `Quantity` could not be compared.
///
/// # Traceability
///
/// - REQ-TYP-001: Type-safe unit system
/// - REQ-TYP-004: Copy, Clone, Debug, PartialEq, PartialOrd on every unit
pub trait Unit:
    private::Sealed + Copy + Clone + core::fmt::Debug + Default + PartialEq + PartialOrd
{
}

mod private {
    pub trait Sealed {}
}

/// Time unit: seconds
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seconds;

impl private::Sealed for Seconds {}
impl Unit for Seconds {}

/// Time unit: milliseconds
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Milliseconds;

impl private::Sealed for Milliseconds {}
impl Unit for Milliseconds {}

/// Angle unit: radians
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Radians;

impl private::Sealed for Radians {}
impl Unit for Radians {}

/// Angle unit: degrees
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Degrees;

impl private::Sealed for Degrees {}
impl Unit for Degrees {}

/// Velocity unit: meters per second
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetersPerSecond;

impl private::Sealed for MetersPerSecond {}
impl Unit for MetersPerSecond {}

/// Angular velocity unit: radians per second
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct RadiansPerSecond;

impl private::Sealed for RadiansPerSecond {}
impl Unit for RadiansPerSecond {}

/// Dimensionless quantity (scalar)
///
/// # Traceability
///
/// - REQ-TYP-001: Physical unit type
/// - REQ-TYP-004: Required trait implementations
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Dimensionless;

impl private::Sealed for Dimensionless {}
impl Unit for Dimensionless {}

#[cfg(test)]
mod tests {
    use super::*;
    use core::any::TypeId;
    use core::cmp::Ordering;

    extern crate std;

    /// Type ids of every unit marker the crate provides.
    fn all_unit_ids() -> [TypeId; 7] {
        [
            TypeId::of::<Seconds>(),
            TypeId::of::<Milliseconds>(),
            TypeId::of::<Radians>(),
            TypeId::of::<Degrees>(),
            TypeId::of::<MetersPerSecond>(),
            TypeId::of::<RadiansPerSecond>(),
            TypeId::of::<Dimensionless>(),
        ]
    }

    /// Compiles only if `U` satisfies the `Unit` bound used by `Quantity`.
    fn is_unit<U: Unit>(_unit: U) -> bool {
        true
    }

    /// Compiles only if `T` has every trait REQ-TYP-004 requires.
    fn has_required_traits<T>(_value: &T) -> bool
    where
        T: Copy + Clone + core::fmt::Debug + PartialEq + PartialOrd,
    {
        true
    }

    /// The seven named units each satisfy `Unit` and are pairwise distinct
    /// types, so a value tagged with one cannot stand in for another.
    ///
    /// Verifies: REQ-TYP-001
    #[test]
    fn test_units_are_distinct_unit_types() {
        assert!(is_unit(Seconds));
        assert!(is_unit(Milliseconds));
        assert!(is_unit(Radians));
        assert!(is_unit(Degrees));
        assert!(is_unit(MetersPerSecond));
        assert!(is_unit(RadiansPerSecond));
        assert!(is_unit(Dimensionless));

        let ids = all_unit_ids();
        for (i, a) in ids.iter().enumerate() {
            for b in ids.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    /// Unit markers occupy no memory and have alignment one, so tagging a
    /// value with a unit adds nothing at run time.
    ///
    /// Verifies: REQ-TYP-002
    #[test]
    fn test_unit_markers_are_zero_sized() {
        const _: () = assert!(core::mem::size_of::<Seconds>() == 0);
        assert_eq!(core::mem::size_of::<Seconds>(), 0);
        assert_eq!(core::mem::size_of::<Milliseconds>(), 0);
        assert_eq!(core::mem::size_of::<Radians>(), 0);
        assert_eq!(core::mem::size_of::<Degrees>(), 0);
        assert_eq!(core::mem::size_of::<MetersPerSecond>(), 0);
        assert_eq!(core::mem::size_of::<RadiansPerSecond>(), 0);
        assert_eq!(core::mem::size_of::<Dimensionless>(), 0);
        assert_eq!(core::mem::align_of::<Seconds>(), 1);
        assert_eq!(core::mem::align_of::<Dimensionless>(), 1);
    }

    /// Every unit marker implements Copy, Clone, Debug, PartialEq and
    /// PartialOrd (checked by the trait bound at compile time) and those
    /// impls behave as expected for a data-free marker.
    ///
    /// Verifies: REQ-TYP-004
    #[test]
    fn test_unit_markers_have_required_traits() {
        assert!(has_required_traits(&Seconds));
        assert!(has_required_traits(&Milliseconds));
        assert!(has_required_traits(&Radians));
        assert!(has_required_traits(&Degrees));
        assert!(has_required_traits(&MetersPerSecond));
        assert!(has_required_traits(&RadiansPerSecond));
        assert!(has_required_traits(&Dimensionless));

        let s = Seconds;
        let copied = s;
        assert_eq!(s, copied);
        assert_eq!(Clone::clone(&Radians), Radians);
        assert_eq!(Seconds.partial_cmp(&Seconds), Some(Ordering::Equal));
        assert_eq!(Degrees.partial_cmp(&Degrees), Some(Ordering::Equal));
        assert_eq!(std::format!("{:?}", MetersPerSecond), "MetersPerSecond");
        assert_eq!(std::format!("{:?}", RadiansPerSecond), "RadiansPerSecond");
    }

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
