//! Active component trait and scheduling types.
//!
//! Active components execute autonomously on a periodic schedule.
//! They have independent execution cycles and can process inputs,
//! update state, and produce outputs without external invocation.
//!
//! # Traceability
//! - REQ-CMP-020: Active components have an associated Schedule type
//! - REQ-CMP-021: Active components implement run_cycle() returning Result<CycleResult, CycleError>
//! - REQ-CMP-022: Schedule specifies rate (Duration) and priority

#![forbid(unsafe_code)]

use core::time::Duration;
use crate::error::{CycleError, InitError};

/// Scheduling parameters for active components.
///
/// Each active component has a defined execution rate and priority.
/// The scheduler uses this information to determine execution order
/// and timing.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use middleware::component::active::Schedule;
///
/// // 100 Hz control loop with high priority
/// let schedule = Schedule {
///     rate: Duration::from_millis(10),
///     priority: 10,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    /// Execution period (time between cycles)
    pub rate: Duration,

    /// Execution priority (higher values = higher priority)
    pub priority: u8,
}

impl Schedule {
    /// Create a new schedule with the given rate and priority.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::time::Duration;
    /// use middleware::component::active::Schedule;
    ///
    /// let schedule = Schedule::new(Duration::from_millis(10), 5);
    /// assert_eq!(schedule.priority, 5);
    /// ```
    pub const fn new(rate: Duration, priority: u8) -> Self {
        Self { rate, priority }
    }

    /// Create a schedule with rate in Hz.
    ///
    /// The period is `1000 / frequency_hz` milliseconds, truncated toward
    /// zero to a whole number of milliseconds (so 33.33 Hz gives 30 ms, and
    /// any frequency above 1000 Hz gives a zero period).
    ///
    /// # Errors
    ///
    /// Returns [`InitError::InvalidConfig`] if `frequency_hz` is NaN,
    /// infinite, zero or negative, or if the resulting period does not fit
    /// in a [`Duration`].
    ///
    /// # Examples
    ///
    /// ```
    /// use core::time::Duration;
    /// use middleware::InitError;
    /// use middleware::component::active::Schedule;
    ///
    /// let schedule = Schedule::from_hz(100.0, 5)?;
    /// // Executes at 100 Hz (10ms period)
    /// assert_eq!(schedule.rate, Duration::from_millis(10));
    ///
    /// assert_eq!(Schedule::from_hz(0.0, 5), Err(InitError::InvalidConfig));
    /// # Ok::<(), InitError>(())
    /// ```
    pub fn from_hz(frequency_hz: f32, priority: u8) -> Result<Self, InitError> {
        if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
            return Err(InitError::InvalidConfig);
        }
        // The period expressed in milliseconds is converted as though it were
        // seconds, so `as_secs` yields the whole-millisecond count truncated
        // toward zero. The integer part of a finite `f32` is exact, so this is
        // a lossless truncation; out-of-range values are rejected by
        // `try_from_secs_f32` rather than saturated.
        let period_ms = Duration::try_from_secs_f32(1000.0 / frequency_hz)
            .map_err(|_| InitError::InvalidConfig)?
            .as_secs();
        Ok(Self {
            rate: Duration::from_millis(period_ms),
            priority,
        })
    }
}

/// Result of an execution cycle.
///
/// Indicates the outcome of a component's run_cycle() execution.
/// This allows the scheduler to detect timing issues and adjust
/// behavior accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleResult {
    /// Cycle completed normally within time budget
    Nominal,

    /// Cycle completed early (yielded remaining time)
    YieldedEarly,

    /// Cycle completed but exceeded time budget (soft overrun)
    OverrunWarning,
}

/// Trait for active components.
///
/// Active components execute autonomously on a periodic schedule.
/// Each cycle they process inputs, update state, and produce outputs.
///
/// # Examples
///
/// ```
/// use core::time::Duration;
/// use middleware::CycleError;
/// use middleware::component::{ActiveComponent, CycleResult, Schedule};
///
/// struct MyActiveComponent {
///     cycles: u32,
/// }
///
/// impl ActiveComponent for MyActiveComponent {
///     fn schedule(&self) -> Schedule {
///         Schedule::new(Duration::from_millis(10), 5)
///     }
///
///     fn run_cycle(&mut self) -> Result<CycleResult, CycleError> {
///         // Process inputs
///         // Update state
///         // Produce outputs
///         self.cycles += 1;
///         Ok(CycleResult::Nominal)
///     }
/// }
///
/// let mut component = MyActiveComponent { cycles: 0 };
/// assert_eq!(component.schedule(), Schedule::new(Duration::from_millis(10), 5));
/// assert_eq!(component.run_cycle(), Ok(CycleResult::Nominal));
/// assert_eq!(component.cycles, 1);
/// ```
pub trait ActiveComponent {
    /// Get the component's execution schedule.
    ///
    /// This defines when and how often the component should execute.
    fn schedule(&self) -> Schedule;

    /// Execute one cycle of the component.
    ///
    /// This method is called periodically by the scheduler according
    /// to the component's schedule. It should complete within the
    /// allocated time budget.
    ///
    /// # Errors
    ///
    /// Returns `CycleError` if the cycle cannot complete:
    /// - `NotReady`: Component not initialized or ready
    /// - `InputUnavailable`: Required input data not available
    /// - `Timeout`: Cycle exceeded time budget (hard overrun)
    /// - `ComputationError`: Internal processing error
    ///
    /// # Returns
    ///
    /// Returns `CycleResult` indicating execution outcome:
    /// - `Nominal`: Normal completion
    /// - `YieldedEarly`: Completed with time remaining
    /// - `OverrunWarning`: Completed but exceeded soft deadline
    fn run_cycle(&mut self) -> Result<CycleResult, CycleError>;
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;

    struct TestActiveComponent {
        counter: u32,
        fail_next: bool,
    }

    impl TestActiveComponent {
        fn new() -> Self {
            Self {
                counter: 0,
                fail_next: false,
            }
        }
    }

    impl ActiveComponent for TestActiveComponent {
        fn schedule(&self) -> Schedule {
            Schedule::new(Duration::from_millis(10), 5)
        }

        fn run_cycle(&mut self) -> Result<CycleResult, CycleError> {
            if self.fail_next {
                return Err(CycleError::ComputationError);
            }

            self.counter += 1;

            if self.counter.is_multiple_of(10) {
                Ok(CycleResult::YieldedEarly)
            } else {
                Ok(CycleResult::Nominal)
            }
        }
    }

    #[test]
    fn schedule_new() {
        let schedule = Schedule::new(Duration::from_millis(10), 5);
        assert_eq!(schedule.rate, Duration::from_millis(10));
        assert_eq!(schedule.priority, 5);
    }

    #[test]
    fn schedule_from_hz() {
        let schedule = Schedule::from_hz(100.0, 10).unwrap();
        assert_eq!(schedule.rate, Duration::from_millis(10));
        assert_eq!(schedule.priority, 10);
    }

    #[test]
    fn schedule_from_hz_fractional() {
        let schedule = Schedule::from_hz(33.33, 1).unwrap();
        assert_eq!(schedule.rate, Duration::from_millis(30));
    }

    #[test]
    fn schedule_from_hz_truncates_sub_millisecond_period() {
        let schedule = Schedule::from_hz(2000.0, 1).unwrap();
        assert_eq!(schedule.rate, Duration::ZERO);
        let schedule = Schedule::from_hz(1.0, 1).unwrap();
        assert_eq!(schedule.rate, Duration::from_millis(1000));
    }

    #[test]
    fn schedule_from_hz_rejects_invalid_frequency() {
        for bad in [0.0, -0.0, -10.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(Schedule::from_hz(bad, 1), Err(InitError::InvalidConfig));
        }
        // A positive subnormal frequency gives a period too long for Duration.
        assert_eq!(
            Schedule::from_hz(f32::MIN_POSITIVE / 2.0, 1),
            Err(InitError::InvalidConfig)
        );
    }

    #[test]
    fn cycle_result_variants() {
        assert_ne!(CycleResult::Nominal, CycleResult::YieldedEarly);
        assert_ne!(CycleResult::Nominal, CycleResult::OverrunWarning);
    }

    #[test]
    fn active_component_nominal_cycle() {
        let mut component = TestActiveComponent::new();
        let result = component.run_cycle();

        assert_eq!(result, Ok(CycleResult::Nominal));
        assert_eq!(component.counter, 1);
    }

    #[test]
    fn active_component_yielded_cycle() {
        let mut component = TestActiveComponent::new();
        component.counter = 9;

        let result = component.run_cycle();
        assert_eq!(result, Ok(CycleResult::YieldedEarly));
    }

    #[test]
    fn active_component_error() {
        let mut component = TestActiveComponent::new();
        component.fail_next = true;

        let result = component.run_cycle();
        assert_eq!(result, Err(CycleError::ComputationError));
    }

    #[test]
    fn active_component_schedule() {
        let component = TestActiveComponent::new();
        let schedule = component.schedule();

        assert_eq!(schedule.rate, Duration::from_millis(10));
        assert_eq!(schedule.priority, 5);
    }

    #[test]
    fn schedule_is_copy() {
        let s1 = Schedule::from_hz(50.0, 3).unwrap();
        let s2 = s1;
        assert_eq!(s1, s2);
    }
}
