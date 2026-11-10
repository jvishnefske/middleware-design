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

use crate::error::{CycleError, InitError};
use core::time::Duration;

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
    /// zero to a whole number of milliseconds (so 33.33 Hz gives 30 ms).
    /// The highest accepted frequency is exactly 1000 Hz (1 ms); any higher
    /// frequency would truncate to a 0 ms period, which would silently turn
    /// a rate request into back-to-back execution, so it is rejected.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::InvalidConfig`] if `frequency_hz` is NaN,
    /// infinite, zero or negative, if the resulting period truncates to
    /// 0 ms (any frequency above 1000 Hz), or if the period does not fit in
    /// a [`Duration`].
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
    /// // Above 1000 Hz the period would be 0 ms.
    /// assert_eq!(Schedule::from_hz(2000.0, 5), Err(InitError::InvalidConfig));
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
        if period_ms == 0 {
            return Err(InitError::InvalidConfig);
        }
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

    /// 2^64, written as an exact `f32` literal (a power of two is exact).
    const TWO_POW_64: f32 = 18_446_744_073_709_551_616.0;

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

    /// A second active component whose schedule depends on its own data,
    /// so the schedule is a per-instance property, not a fixed constant.
    struct ConfigurableActive {
        schedule: Schedule,
        outcome: Result<CycleResult, CycleError>,
    }

    impl ActiveComponent for ConfigurableActive {
        fn schedule(&self) -> Schedule {
            self.schedule
        }

        fn run_cycle(&mut self) -> Result<CycleResult, CycleError> {
            self.outcome
        }
    }

    /// Reads the schedule through the trait bound only, as a scheduler would.
    fn schedule_via_trait<C: ActiveComponent>(component: &C) -> Schedule {
        component.schedule()
    }

    /// Runs one cycle through the trait bound only, as a scheduler would.
    fn cycle_via_trait<C: ActiveComponent>(component: &mut C) -> Result<CycleResult, CycleError> {
        component.run_cycle()
    }

    #[test]
    fn schedule_new() {
        let schedule = Schedule::new(Duration::from_millis(10), 5);
        assert_eq!(schedule.rate, Duration::from_millis(10));
        assert_eq!(schedule.priority, 5);
    }

    /// The `schedule()` signature is pinned by coercion to a function pointer
    /// type: a change of return type would fail to compile.
    /// Verifies: REQ-CMP-020
    #[test]
    fn active_component_exposes_schedule_through_trait() {
        let signature: fn(&TestActiveComponent) -> Schedule =
            <TestActiveComponent as ActiveComponent>::schedule;
        let component = TestActiveComponent::new();
        assert_eq!(
            signature(&component),
            Schedule::new(Duration::from_millis(10), 5)
        );

        let fast = ConfigurableActive {
            schedule: Schedule::new(Duration::from_millis(1), 255),
            outcome: Ok(CycleResult::Nominal),
        };
        assert_eq!(
            schedule_via_trait(&fast),
            Schedule::new(Duration::from_millis(1), 255)
        );
    }

    /// The `run_cycle()` signature is pinned by coercion to a function
    /// pointer type, and every `CycleResult` and `CycleError` variant is
    /// passed back unchanged to the caller.
    /// Verifies: REQ-CMP-021
    #[test]
    fn run_cycle_returns_every_result_and_error_variant() {
        let signature: fn(&mut ConfigurableActive) -> Result<CycleResult, CycleError> =
            <ConfigurableActive as ActiveComponent>::run_cycle;
        let outcomes = [
            Ok(CycleResult::Nominal),
            Ok(CycleResult::YieldedEarly),
            Ok(CycleResult::OverrunWarning),
            Err(CycleError::NotReady),
            Err(CycleError::InputUnavailable),
            Err(CycleError::Timeout),
            Err(CycleError::ComputationError),
        ];
        for outcome in outcomes {
            let mut component = ConfigurableActive {
                schedule: Schedule::new(Duration::from_millis(10), 0),
                outcome,
            };
            assert_eq!(signature(&mut component), outcome);
            assert_eq!(cycle_via_trait(&mut component), outcome);
        }
    }

    /// `rate` is a `Duration` and `priority` a `u8`: the typed bindings fail
    /// to compile if either field type changes. Priority boundaries 0 and
    /// `u8::MAX` and rate boundaries `ZERO` and `Duration::MAX` are stored
    /// unchanged.
    /// Verifies: REQ-CMP-022
    #[test]
    fn schedule_carries_duration_rate_and_u8_priority() {
        for (rate, priority) in [
            (Duration::ZERO, 0_u8),
            (Duration::from_nanos(1), 1),
            (Duration::from_millis(10), 128),
            (Duration::MAX, u8::MAX),
        ] {
            let schedule = Schedule::new(rate, priority);
            let stored_rate: Duration = schedule.rate;
            let stored_priority: u8 = schedule.priority;
            assert_eq!(stored_rate, rate);
            assert_eq!(stored_priority, priority);
        }
        // `new` is usable in const context, so schedules can be static tables.
        const STATIC_SCHEDULE: Schedule = Schedule::new(Duration::from_millis(20), 7);
        assert_eq!(STATIC_SCHEDULE.rate, Duration::from_millis(20));
        assert_eq!(STATIC_SCHEDULE.priority, 7);
    }

    /// Nominal equivalence class: periods are `1000 / f` ms truncated, and
    /// the priority is passed through.
    /// Verifies: REQ-CMP-022
    #[test]
    fn schedule_from_hz() {
        let schedule = Schedule::from_hz(100.0, 10).unwrap();
        assert_eq!(schedule.rate, Duration::from_millis(10));
        assert_eq!(schedule.priority, 10);
        assert_eq!(
            Schedule::from_hz(1.0, 0).unwrap().rate,
            Duration::from_millis(1000)
        );
        assert_eq!(
            Schedule::from_hz(3.0, 0).unwrap().rate,
            Duration::from_millis(333)
        );
        // 0.001 is not exact in f32 (it is 0.001000000047...), so the period
        // is 999_999.95 ms and truncation gives 999_999 ms, not 1_000_000.
        assert_eq!(
            Schedule::from_hz(0.001, 0).unwrap().rate,
            Duration::from_millis(999_999)
        );
        assert_eq!(Schedule::from_hz(50.0, u8::MAX).unwrap().priority, u8::MAX);
    }

    #[test]
    fn schedule_from_hz_fractional() {
        let schedule = Schedule::from_hz(33.33, 1).unwrap();
        assert_eq!(schedule.rate, Duration::from_millis(30));
    }

    /// Error class NaN: the quiet NaN, a negative NaN, a NaN with payload
    /// and a signalling NaN bit pattern are all rejected.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_rejects_nan() {
        for bad in [
            f32::NAN,
            -f32::NAN,
            f32::from_bits(0x7FC0_0001),
            f32::from_bits(0x7F80_0001),
            f32::from_bits(0xFFFF_FFFF),
        ] {
            assert!(bad.is_nan());
            assert_eq!(Schedule::from_hz(bad, 1), Err(InitError::InvalidConfig));
        }
    }

    /// Error class infinite: both signs are rejected.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_rejects_infinite() {
        for bad in [f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(Schedule::from_hz(bad, 1), Err(InitError::InvalidConfig));
        }
    }

    /// Error class zero: both signed zeros are rejected.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_rejects_zero() {
        for bad in [0.0_f32, -0.0_f32] {
            assert_eq!(Schedule::from_hz(bad, 1), Err(InitError::InvalidConfig));
        }
    }

    /// Error class negative: from the negative value closest to zero (the
    /// negative smallest subnormal) to the most negative finite value.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_rejects_negative() {
        for bad in [
            -f32::from_bits(1),
            -f32::MIN_POSITIVE,
            -1.0e-3,
            -1.0,
            -100.0,
            -1000.0,
            f32::MIN,
        ] {
            assert!(bad < 0.0);
            assert_eq!(Schedule::from_hz(bad, 1), Err(InitError::InvalidConfig));
        }
    }

    /// Boundary: the smallest positive frequencies. The smallest subnormal
    /// and the smallest normal `f32` both give a period (`1000 / f`) that
    /// overflows `f32` to infinity, so they are rejected rather than
    /// saturated.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_rejects_smallest_positive_frequencies() {
        for tiny in [
            f32::from_bits(1),
            f32::MIN_POSITIVE / 2.0,
            f32::MIN_POSITIVE,
        ] {
            assert!(tiny > 0.0);
            assert_eq!(Schedule::from_hz(tiny, 1), Err(InitError::InvalidConfig));
        }
    }

    /// Boundary: the period overflows `Duration` exactly where the
    /// whole-millisecond count reaches 2^64. `1000 / 2^64` Hz gives a period
    /// of exactly 2^64 ms and is rejected; the next representable frequency
    /// up gives the largest representable period below 2^64 ms and is
    /// accepted with that period, unsaturated.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_duration_overflow_boundary() {
        let edge = 1000.0 / TWO_POW_64;
        assert_eq!(Schedule::from_hz(edge, 1), Err(InitError::InvalidConfig));
        assert_eq!(
            Schedule::from_hz(edge.next_down(), 1),
            Err(InitError::InvalidConfig)
        );

        let just_inside = Schedule::from_hz(edge.next_up(), 1).unwrap();
        // 2^64 * (1 - 2^-23) rounds to the largest f32 below 2^64, 2^64 - 2^40.
        assert_eq!(
            just_inside.rate,
            Duration::from_millis(u64::MAX - (1 << 40) + 1)
        );
        assert_eq!(just_inside.priority, 1);

        let half = Schedule::from_hz(edge * 2.0, 1).unwrap();
        assert_eq!(half.rate, Duration::from_millis(1 << 63));
    }

    /// Boundary: around 1000 Hz the period crosses from 1 ms to 0 ms.
    /// Exactly 1000 Hz (and the next representable frequency down) gives
    /// 1 ms and is accepted; the next representable frequency up and every
    /// frequency above 1000 Hz up to `f32::MAX` would give a 0 ms period
    /// and are rejected.
    /// Verifies: REQ-CMP-023
    #[test]
    fn schedule_from_hz_rejects_sub_millisecond_period() {
        assert_eq!(
            Schedule::from_hz(1000.0_f32.next_down(), 1).unwrap().rate,
            Duration::from_millis(1)
        );
        let at_limit = Schedule::from_hz(1000.0, 9).unwrap();
        assert_eq!(at_limit.rate, Duration::from_millis(1));
        assert_eq!(at_limit.priority, 9);
        for above in [1000.0_f32.next_up(), 1001.0, 2000.0, 1.0e6, f32::MAX] {
            assert_eq!(
                Schedule::from_hz(above, 9),
                Err(InitError::InvalidConfig),
                "{above} Hz"
            );
        }
    }

    #[test]
    fn cycle_result_variants() {
        assert_ne!(CycleResult::Nominal, CycleResult::YieldedEarly);
        assert_ne!(CycleResult::Nominal, CycleResult::OverrunWarning);
        assert_ne!(CycleResult::YieldedEarly, CycleResult::OverrunWarning);
    }

    /// Verifies: REQ-CMP-021
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

    /// Fault injection: a failing cycle reports the error and does not
    /// advance component state.
    /// Verifies: REQ-CMP-021
    #[test]
    fn active_component_error() {
        let mut component = TestActiveComponent::new();
        component.fail_next = true;

        let result = component.run_cycle();
        assert_eq!(result, Err(CycleError::ComputationError));
        assert_eq!(component.counter, 0);
    }

    /// Verifies: REQ-CMP-020
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
