//! Test-only helpers shared by the crate's unit-test modules.
//!
//! Compiled only under `cfg(test)`; nothing here is part of the public API or
//! the firmware image.
//!
//! [`assert_close`] compares computed floating-point results with a
//! tolerance instead of `==`, so tests do not depend on the exact rounding
//! of intermediate arithmetic. Values that are exact by construction (stored
//! parameters read back, reset values, saturation limits) are still compared
//! with `assert_eq!` in the individual test modules.

/// Default absolute tolerance used by [`assert_close`], scaled by the
/// magnitude of the expected value when that exceeds one.
pub(crate) const EPSILON: f32 = 1e-5;

/// Returns `true` when `actual` is within [`EPSILON`] of `expected`, where the
/// tolerance is relative (`EPSILON * |expected|`) for `|expected| > 1`.
pub(crate) fn approx_eq(actual: f32, expected: f32) -> bool {
    let scale = if expected.abs() > 1.0 { expected.abs() } else { 1.0 };
    (actual - expected).abs() <= EPSILON * scale
}

/// Asserts that `actual` is approximately equal to `expected` (see
/// [`approx_eq`]); on failure the test panics, reporting both values at the
/// caller's location.
#[track_caller]
pub(crate) fn assert_close(actual: f32, expected: f32) {
    assert!(
        approx_eq(actual, expected),
        "assert_close failed: actual = {actual}, expected = {expected}"
    );
}
