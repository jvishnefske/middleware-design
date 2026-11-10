//! Guard conditions for state transitions.
//!
//! Guards are predicates that determine whether a transition should occur.
//! This module provides common guard implementations for control systems.
//!
//! # Traceability
//! - REQ-FSM-030: ThresholdGuard for control-based transitions
//! - REQ-FSM-031: TimedGuard for duration-based transitions

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
/// has been met.
///
/// # Examples
///
/// ```
/// use middleware::state_machine::TimedGuard;
///
/// let guard = TimedGuard::new(5.0);
/// assert!(!guard.check());
///
/// // `update` returns the advanced guard and whether the duration is met.
/// let (guard, satisfied) = guard.update(3.0);
/// assert!(!satisfied);
/// assert!(!guard.check());
///
/// let (guard, satisfied) = guard.update(3.0);
/// assert!(satisfied);
/// assert!(guard.check());
///
/// // `reset` clears the elapsed time and keeps the required duration.
/// let guard = guard.reset();
/// assert!(!guard.check());
/// assert_eq!(guard.required_duration, 5.0);
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
    ///
    /// # Parameters
    /// - `dt`: Time delta in seconds
    ///
    /// # Returns
    /// A tuple of (new guard instance, condition satisfied)
    pub fn update(self, dt: f32) -> (Self, bool) {
        let new_elapsed = self.elapsed + dt;
        let satisfied = new_elapsed >= self.required_duration;
        (
            Self {
                required_duration: self.required_duration,
                elapsed: new_elapsed,
            },
            satisfied,
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_guard_above() {
        let guard = ThresholdGuard::new(100.0, ThresholdDirection::Above);

        assert!(!guard.check(50.0));
        assert!(!guard.check(100.0)); // Exactly at threshold
        assert!(guard.check(150.0));
    }

    #[test]
    fn test_threshold_guard_below() {
        let guard = ThresholdGuard::new(100.0, ThresholdDirection::Below);

        assert!(guard.check(50.0));
        assert!(!guard.check(100.0)); // Exactly at threshold
        assert!(!guard.check(150.0));
    }

    #[test]
    fn test_timed_guard_new() {
        let guard = TimedGuard::new(5.0);
        assert_eq!(guard.required_duration, 5.0);
        assert_eq!(guard.elapsed, 0.0);
        assert!(!guard.check());
    }

    #[test]
    fn test_timed_guard_update() {
        let guard = TimedGuard::new(5.0);

        let (guard, satisfied) = guard.update(2.0);
        assert!(!satisfied);
        assert_eq!(guard.elapsed, 2.0);

        let (guard, satisfied) = guard.update(2.0);
        assert!(!satisfied);
        assert_eq!(guard.elapsed, 4.0);

        let (guard, satisfied) = guard.update(2.0);
        assert!(satisfied);
        assert_eq!(guard.elapsed, 6.0);
    }

    #[test]
    fn test_timed_guard_reset() {
        let guard = TimedGuard::new(5.0);
        let (guard, _) = guard.update(3.0);
        assert_eq!(guard.elapsed, 3.0);

        let guard = guard.reset();
        assert_eq!(guard.elapsed, 0.0);
        assert_eq!(guard.required_duration, 5.0);
    }

    #[test]
    fn test_timed_guard_exact_duration() {
        let guard = TimedGuard::new(5.0);
        let (guard, satisfied) = guard.update(5.0);
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
}
