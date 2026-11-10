//! Guard conditions for state transitions.
//!
//! Guards are predicates that determine whether a transition should occur.
//! This module provides common guard implementations for control systems.
//!
//! # Traceability
//! - REQ-FSM-030: ThresholdGuard for control-based transitions
//! - REQ-FSM-031: TimedGuard for duration-based transitions
//! - REQ-FSM-032: TimedGuard rejects a negative or non-finite time step
//!   with [`StateMachineError::InvalidTimeStep`]

use crate::error::StateMachineError;

/// Direction for threshold comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdDirection {
    /// Value must be above the threshold.
    Above,
    /// Value must be below the threshold.
    Below,
}

/// Guard that checks if a value crosses a threshold.
///
/// # Examples
///
/// ```
/// use middleware::state_machine::ThresholdGuard;
/// use middleware::state_machine::guards::ThresholdDirection;
///
/// let guard = ThresholdGuard::new(100.0, ThresholdDirection::Above);
/// assert!(!guard.check(50.0));
/// assert!(!guard.check(100.0)); // the comparison is strict
/// assert!(guard.check(150.0));
///
/// let guard = ThresholdGuard::new(10.0, ThresholdDirection::Below);
/// assert!(guard.check(5.0));
/// assert!(!guard.check(15.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThresholdGuard {
    /// The threshold value.
    pub threshold: f32,
    /// The direction of comparison.
    pub direction: ThresholdDirection,
}

impl ThresholdGuard {
    /// Creates a new threshold guard.
    ///
    /// # Parameters
    /// - `threshold`: The threshold value
    /// - `direction`: Whether to check above or below the threshold
    ///
    /// # Returns
    /// A new threshold guard
    pub const fn new(threshold: f32, direction: ThresholdDirection) -> Self {
        Self {
            threshold,
            direction,
        }
    }

    /// Checks if the value satisfies the threshold condition.
    ///
    /// # Parameters
    /// - `value`: The value to check
    ///
    /// # Returns
    /// `true` if the condition is satisfied, `false` otherwise
    pub fn check(&self, value: f32) -> bool {
        match self.direction {
            ThresholdDirection::Above => value > self.threshold,
            ThresholdDirection::Below => value < self.threshold,
        }
    }
}

/// Guard that checks if sufficient time has elapsed.
///
/// This guard tracks elapsed time and returns true once the required duration
/// has been met. Elapsed time only moves forward: a negative or non-finite
/// time step is rejected.
///
/// # Examples
///
/// ```
/// use middleware::StateMachineError;
/// use middleware::state_machine::TimedGuard;
///
/// let guard = TimedGuard::new(5.0);
/// assert!(!guard.check());
///
/// // `update` returns the advanced guard and whether the duration is met.
/// let (guard, satisfied) = guard.update(3.0)?;
/// assert!(!satisfied);
/// assert!(!guard.check());
///
/// // A negative or non-finite step is an error; `guard` is unchanged.
/// assert_eq!(guard.update(-1.0), Err(StateMachineError::InvalidTimeStep));
/// assert_eq!(guard.elapsed, 3.0);
///
/// let (guard, satisfied) = guard.update(3.0)?;
/// assert!(satisfied);
/// assert!(guard.check());
///
/// // `reset` clears the elapsed time and keeps the required duration.
/// let guard = guard.reset();
/// assert!(!guard.check());
/// assert_eq!(guard.required_duration, 5.0);
/// # Ok::<(), StateMachineError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedGuard {
    /// Required duration in seconds.
    pub required_duration: f32,
    /// Elapsed time in seconds.
    pub elapsed: f32,
}

impl TimedGuard {
    /// Creates a new timed guard.
    ///
    /// # Parameters
    /// - `required_duration`: The required duration in seconds
    ///
    /// # Returns
    /// A new timed guard with elapsed time set to 0.0
    pub const fn new(required_duration: f32) -> Self {
        Self {
            required_duration,
            elapsed: 0.0,
        }
    }

    /// Updates the elapsed time and checks if the duration has been met.
    ///
    /// This method is immutable - it returns a new instance with updated elapsed time.
    /// A zero `dt` is valid and leaves the elapsed time as it was.
    ///
    /// # Parameters
    /// - `dt`: Time delta in seconds, finite and `>= 0`
    ///
    /// # Returns
    /// A tuple of (new guard instance, condition satisfied)
    ///
    /// # Errors
    /// [`StateMachineError::InvalidTimeStep`] if `dt` is negative, NaN or
    /// infinite. No new guard is produced, so the caller's guard (a `Copy`
    /// value) keeps its elapsed time.
    pub fn update(self, dt: f32) -> Result<(Self, bool), StateMachineError> {
        if !dt.is_finite() || dt < 0.0 {
            return Err(StateMachineError::InvalidTimeStep);
        }
        let new_elapsed = self.elapsed + dt;
        let satisfied = new_elapsed >= self.required_duration;
        Ok((
            Self {
                required_duration: self.required_duration,
                elapsed: new_elapsed,
            },
            satisfied,
        ))
    }

    /// Resets the elapsed time to zero.
    ///
    /// # Returns
    /// A new guard instance with elapsed time reset
    pub fn reset(self) -> Self {
        Self {
            required_duration: self.required_duration,
            elapsed: 0.0,
        }
    }

    /// Checks if the required duration has elapsed.
    ///
    /// # Returns
    /// `true` if elapsed time >= required duration, `false` otherwise
    pub fn check(&self) -> bool {
        self.elapsed >= self.required_duration
    }
}

/// Trait for guards based on control system state.
///
/// This trait allows guards to access control block outputs or other system state.
///
/// # Examples
///
/// ```
/// use middleware::state_machine::ControlGuard;
///
/// struct MyContext {
///     control_output: f32,
/// }
///
/// struct ControlValueGuard {
///     threshold: f32,
/// }
///
/// impl ControlGuard for ControlValueGuard {
///     type Context = MyContext;
///
///     fn evaluate(&self, context: &Self::Context) -> bool {
///         context.control_output > self.threshold
///     }
/// }
///
/// let guard = ControlValueGuard { threshold: 10.0 };
/// assert!(guard.evaluate(&MyContext { control_output: 12.0 }));
/// assert!(!guard.evaluate(&MyContext { control_output: 8.0 }));
/// ```
pub trait ControlGuard {
    /// Context type containing control system state.
    type Context;

    /// Evaluates the guard condition.
    ///
    /// # Parameters
    /// - `context`: Reference to the context
    ///
    /// # Returns
    /// `true` if the guard condition is satisfied, `false` otherwise
    fn evaluate(&self, context: &Self::Context) -> bool;
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::float_cmp, clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    /// Verifies: REQ-FSM-030
    #[test]
    fn test_threshold_guard_above() {
        let guard = ThresholdGuard::new(100.0, ThresholdDirection::Above);

        assert!(!guard.check(50.0));
        assert!(!guard.check(100.0)); // Exactly at threshold
        assert!(guard.check(150.0));
    }

    /// Verifies: REQ-FSM-030
    #[test]
    fn test_threshold_guard_below() {
        let guard = ThresholdGuard::new(100.0, ThresholdDirection::Below);

        assert!(guard.check(50.0));
        assert!(!guard.check(100.0)); // Exactly at threshold
        assert!(!guard.check(150.0));
    }

    /// Verifies: REQ-FSM-031
    #[test]
    fn test_timed_guard_new() {
        let guard = TimedGuard::new(5.0);
        assert_eq!(guard.required_duration, 5.0);
        assert_eq!(guard.elapsed, 0.0);
        assert!(!guard.check());
    }

    /// Verifies: REQ-FSM-031
    #[test]
    fn test_timed_guard_update() {
        let guard = TimedGuard::new(5.0);

        let (guard, satisfied) = guard.update(2.0).unwrap();
        assert!(!satisfied);
        assert_close(guard.elapsed, 2.0);

        let (guard, satisfied) = guard.update(2.0).unwrap();
        assert!(!satisfied);
        assert_close(guard.elapsed, 4.0);

        let (guard, satisfied) = guard.update(2.0).unwrap();
        assert!(satisfied);
        assert_close(guard.elapsed, 6.0);
    }

    /// Verifies: REQ-FSM-031
    #[test]
    fn test_timed_guard_reset() {
        let guard = TimedGuard::new(5.0);
        let (guard, _) = guard.update(3.0).unwrap();
        assert_close(guard.elapsed, 3.0);

        let guard = guard.reset();
        assert_eq!(guard.elapsed, 0.0);
        assert_eq!(guard.required_duration, 5.0);
    }

    /// Verifies: REQ-FSM-031
    #[test]
    fn test_timed_guard_exact_duration() {
        let guard = TimedGuard::new(5.0);
        let (guard, satisfied) = guard.update(5.0).unwrap();
        assert!(satisfied);
        assert!(guard.check());
    }

    struct TestControlGuard {
        threshold: f32,
    }

    struct TestContext {
        value: f32,
    }

    impl ControlGuard for TestControlGuard {
        type Context = TestContext;

        fn evaluate(&self, context: &Self::Context) -> bool {
            context.value > self.threshold
        }
    }

    #[test]
    fn test_control_guard() {
        let guard = TestControlGuard { threshold: 10.0 };

        let context = TestContext { value: 5.0 };
        assert!(!guard.evaluate(&context));

        let context = TestContext { value: 15.0 };
        assert!(guard.evaluate(&context));
    }

    /// Boundary values around the threshold: exactly at it, one ULP above
    /// and one ULP below, for both directions (comparison is strict).
    /// Verifies: REQ-FSM-030
    #[test]
    fn test_threshold_guard_boundaries() {
        let t = 100.0_f32;
        let above = ThresholdGuard::new(t, ThresholdDirection::Above);
        let below = ThresholdGuard::new(t, ThresholdDirection::Below);

        assert!(!above.check(t));
        assert!(above.check(t.next_up()));
        assert!(!above.check(t.next_down()));

        assert!(!below.check(t));
        assert!(below.check(t.next_down()));
        assert!(!below.check(t.next_up()));

        // Negative and zero thresholds behave the same way.
        let zero = ThresholdGuard::new(0.0, ThresholdDirection::Above);
        assert!(!zero.check(0.0));
        assert!(!zero.check(-0.0));
        assert!(zero.check(f32::from_bits(1))); // smallest positive subnormal
        let neg = ThresholdGuard::new(-5.0, ThresholdDirection::Below);
        let n = -5.0_f32;
        assert!(neg.check(n.next_down()));
        assert!(!neg.check(n));
        assert!(!neg.check(n.next_up()));
    }

    /// Error guessing: non-finite inputs. NaN never satisfies either
    /// direction (as value or as threshold); infinities compare as ordered
    /// extremes.
    /// Verifies: REQ-FSM-030
    #[test]
    fn test_threshold_guard_non_finite() {
        let above = ThresholdGuard::new(100.0, ThresholdDirection::Above);
        let below = ThresholdGuard::new(100.0, ThresholdDirection::Below);
        assert!(!above.check(f32::NAN));
        assert!(!below.check(f32::NAN));
        assert!(above.check(f32::INFINITY));
        assert!(!above.check(f32::NEG_INFINITY));
        assert!(below.check(f32::NEG_INFINITY));
        assert!(!below.check(f32::INFINITY));

        let nan_above = ThresholdGuard::new(f32::NAN, ThresholdDirection::Above);
        let nan_below = ThresholdGuard::new(f32::NAN, ThresholdDirection::Below);
        for v in [0.0, f32::MAX, f32::MIN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(!nan_above.check(v));
            assert!(!nan_below.check(v));
        }
    }

    /// Boundary values of the elapsed time: exactly the duration (reached
    /// in one step and by exact accumulation), one ULP short, and a zero
    /// duration that is satisfied immediately.
    /// Verifies: REQ-FSM-031
    #[test]
    fn test_timed_guard_boundaries() {
        let (g, satisfied) = TimedGuard::new(5.0).update(5.0_f32.next_down()).unwrap();
        assert!(!satisfied);
        assert!(!g.check());
        let (g, satisfied) = g.update(5.0 - g.elapsed).unwrap(); // exactly one ULP of 5.0
        assert!(satisfied);
        assert!(g.check());

        // 2.5 is exactly representable, so 2.5 + 2.5 == 5.0 exactly.
        let (g, first) = TimedGuard::new(5.0).update(2.5).unwrap();
        let (g, second) = g.update(2.5).unwrap();
        assert!(!first);
        assert!(second);
        assert_eq!(g.elapsed, 5.0);

        let zero = TimedGuard::new(0.0);
        assert!(zero.check());
        let (_, satisfied) = zero.update(0.0).unwrap();
        assert!(satisfied);
    }

    /// Boundary dt = 0: valid, no time passes, the guard is unchanged, and
    /// the smallest positive subnormal step is accepted too.
    /// Verifies: REQ-FSM-032
    #[test]
    fn test_timed_guard_zero_dt_is_valid() {
        let (g, _) = TimedGuard::new(5.0).update(3.0).unwrap();
        for zero in [0.0_f32, -0.0_f32] {
            let (g0, satisfied) = g.update(zero).unwrap();
            assert!(!satisfied);
            assert_eq!(g0, g);
        }
        let (g1, _) = g.update(f32::from_bits(1)).unwrap();
        assert_eq!(g1.elapsed, 3.0); // absorbed by rounding, but accepted
    }

    /// Error classes negative and non-finite dt (fault injection): each is
    /// rejected with `InvalidTimeStep`, the guard's elapsed time is left
    /// unchanged, and the guard still works normally afterwards.
    /// Verifies: REQ-FSM-032
    #[test]
    fn test_timed_guard_abnormal_dt() {
        let (g, _) = TimedGuard::new(5.0).update(3.0).unwrap();
        for bad in [
            -f32::from_bits(1),
            -f32::MIN_POSITIVE,
            -1.0,
            f32::MIN,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NAN,
            -f32::NAN,
        ] {
            assert_eq!(
                g.update(bad),
                Err(StateMachineError::InvalidTimeStep),
                "dt = {bad}"
            );
            assert_eq!(g.elapsed, 3.0);
            assert!(!g.check());
        }
        // Rejection on a fresh guard and on an already-satisfied guard.
        assert_eq!(
            TimedGuard::new(5.0).update(f32::NAN),
            Err(StateMachineError::InvalidTimeStep)
        );
        let (done, satisfied) = g.update(2.0).unwrap();
        assert!(satisfied);
        assert_eq!(done.update(-1.0), Err(StateMachineError::InvalidTimeStep));
        assert!(done.check());

        // The guard is not poisoned: a valid step after a rejection advances it.
        let (g, satisfied) = g.update(2.0).unwrap();
        assert!(satisfied);
        assert_eq!(g.elapsed, 5.0);
    }
}
