//! PID controller implementation.
//!
//! Provides a proportional-integral-derivative controller with anti-windup
//! and derivative-on-measurement.
//!
//! The derivative needs two samples. Before the first update (and after a
//! reset) there is no previous measurement, which [`PidState`] records as
//! `prev_measurement: None`; the derivative term is zero on that first
//! sample, so a controller started far from zero does not kick.
//!
//! # Traceability
//! - REQ-CTL-010: PID supports configurable Kp, Ki, Kd gains
//! - REQ-CTL-011: PID implements anti-windup via integral clamping
//! - REQ-CTL-012: PID supports output saturation limits
//! - REQ-CTL-013: PID uses derivative-on-measurement to avoid setpoint kick
//! - REQ-CTL-014: PID is constructable via Builder pattern

use super::{ControlBlock, ControlError};

/// PID controller gains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidGains {
    /// Proportional gain.
    pub kp: f32,
    /// Integral gain.
    pub ki: f32,
    /// Derivative gain.
    pub kd: f32,
}

impl PidGains {
    /// Creates new PID gains.
    ///
    /// # Parameters
    /// - `kp`: Proportional gain
    /// - `ki`: Integral gain
    /// - `kd`: Derivative gain
    pub const fn new(kp: f32, ki: f32, kd: f32) -> Self {
        Self { kp, ki, kd }
    }
}

/// PID controller output and integral limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidLimits {
    /// Minimum output value.
    pub output_min: f32,
    /// Maximum output value.
    pub output_max: f32,
    /// Maximum absolute integral value (anti-windup).
    pub integral_max: f32,
}

impl PidLimits {
    /// Creates new PID limits.
    ///
    /// # Parameters
    /// - `output_min`: Minimum output value
    /// - `output_max`: Maximum output value
    /// - `integral_max`: Maximum absolute integral value
    pub const fn new(output_min: f32, output_max: f32, integral_max: f32) -> Self {
        Self {
            output_min,
            output_max,
            integral_max,
        }
    }
}

/// PID controller internal state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidState {
    /// Integral accumulator.
    pub integral: f32,
    /// Error from the most recent update (`0.0` before the first update).
    pub prev_error: f32,
    /// Measurement from the most recent update, used for
    /// derivative-on-measurement. `None` means no sample has been taken yet
    /// (new or reset controller), in which case the next update's derivative
    /// term is zero.
    pub prev_measurement: Option<f32>,
}

impl PidState {
    /// Creates the initial state: zero integral and error, and no previous
    /// measurement.
    pub const fn new() -> Self {
        Self {
            integral: 0.0,
            prev_error: 0.0,
            prev_measurement: None,
        }
    }
}

impl Default for PidState {
    fn default() -> Self {
        Self::new()
    }
}

/// PID controller.
///
/// Implements a discrete-time PID controller with:
/// - Configurable gains (Kp, Ki, Kd)
/// - Anti-windup via integral clamping
/// - Output saturation
/// - Derivative-on-measurement to avoid setpoint kick
///
/// The controller uses the form:
/// ```text
/// u(t) = Kp * e(t) + Ki * integral(e) - Kd * d(measurement)/dt
/// ```
///
/// On the first update after construction or [`ControlBlock::reset`] there
/// is no previous measurement, so the derivative term is zero for that
/// sample.
///
/// # Errors
/// `update` returns, in this order of checking:
/// - `InvalidInput` if the setpoint or measurement is not finite;
/// - `InvalidDelta` if `dt` is zero, negative, NaN or infinite;
/// - `InvalidConfig` if `output_min > output_max`, `integral_max < 0`, or
///   any of these limits is NaN;
/// - `NumericalError` if the unclamped integral or the unsaturated output is
///   not finite (overflow, or a NaN gain). Overflow is reported before
///   clamping, so it is never masked by the anti-windup or output limits.
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, PidControllerBuilder};
///
/// let pid = PidControllerBuilder::new()
///     .gains(2.0, 0.5, 0.1)
///     .limits(-100.0, 100.0, 50.0)
///     .build()
///     .expect("gains and limits are set and valid");
///
/// let setpoint = 100.0;
/// let measurement = 95.0;
/// let dt = 0.01;
///
/// // First update: no previous measurement, so the derivative term is zero.
/// // output = Kp * 5.0 + Ki * 0.05 = 10.0 + 0.025
/// let (pid, output) = pid.update((setpoint, measurement), dt).unwrap();
/// assert!((output - 10.025).abs() < 1e-4);
/// assert!((pid.state().integral - 0.05).abs() < 1e-6); // error 5.0 * dt
/// assert_eq!(pid.state().prev_measurement, Some(measurement));
///
/// // With a steady measurement the derivative term is zero:
/// // output = Kp * 5.0 + Ki * (2 * 0.05) = 10.0 + 0.05
/// let (_pid, output) = pid.update((setpoint, measurement), dt).unwrap();
/// assert!((output - 10.05).abs() < 1e-4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidController {
    gains: PidGains,
    limits: PidLimits,
    state: PidState,
}

impl PidController {
    /// Creates a new PID controller.
    ///
    /// # Parameters
    /// - `gains`: The PID gains
    /// - `limits`: Output and integral limits
    ///
    /// # Returns
    /// A new PID controller with state initialized to zero
    pub const fn new(gains: PidGains, limits: PidLimits) -> Self {
        Self {
            gains,
            limits,
            state: PidState::new(),
        }
    }

    /// Returns the current gains.
    pub fn gains(&self) -> PidGains {
        self.gains
    }

    /// Returns the current limits.
    pub fn limits(&self) -> PidLimits {
        self.limits
    }

    /// Returns the current state.
    pub fn state(&self) -> PidState {
        self.state
    }
}

impl ControlBlock for PidController {
    type Input = (f32, f32); // (setpoint, measurement)
    type Output = f32;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        let (setpoint, measurement) = input;

        // Validate inputs
        if !setpoint.is_finite() || !measurement.is_finite() {
            return Err(ControlError::InvalidInput);
        }
        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        // Validate limits: `f32::clamp` panics when `min > max` or a bound is
        // NaN, so reject such configurations before clamping. The builder
        // already rejects them; this repeats the check as defence in depth
        // for controllers made with `PidController::new`.
        validate_limits(&self.limits)?;

        // Compute error
        let error = setpoint - measurement;

        // Integral with anti-windup. The unclamped sum is checked first so
        // that an overflow is reported rather than hidden by the clamp.
        let integral = self.state.integral + error * dt;
        if !integral.is_finite() {
            return Err(ControlError::NumericalError);
        }
        let integral = integral.clamp(-self.limits.integral_max, self.limits.integral_max);

        // Derivative term (on measurement to avoid setpoint kick). With no
        // previous sample there is no rate to estimate, so it is zero.
        let derivative = match self.state.prev_measurement {
            Some(prev) => (measurement - prev) / dt,
            None => 0.0,
        };

        // Unsaturated output. Negative D because we use the derivative of
        // the measurement. Checked before saturation so that an overflow (or
        // a NaN gain) is reported rather than clamped to a finite limit.
        let output = self.gains.kp * error + self.gains.ki * integral - self.gains.kd * derivative;
        if !output.is_finite() {
            return Err(ControlError::NumericalError);
        }

        // Saturate output. A finite value clamped to ordered, non-NaN
        // bounds is finite, so no further check is needed.
        let output = output.clamp(self.limits.output_min, self.limits.output_max);

        // Create new state
        let new_state = PidState {
            integral,
            prev_error: error,
            prev_measurement: Some(measurement),
        };

        Ok((
            Self {
                gains: self.gains,
                limits: self.limits,
                state: new_state,
            },
            output,
        ))
    }

    fn reset(self) -> Self {
        Self {
            gains: self.gains,
            limits: self.limits,
            state: PidState::new(),
        }
    }
}

/// Checks that `limits` can be applied by `f32::clamp` without panicking.
///
/// Infinite bounds are valid (they mean "unlimited").
///
/// # Errors
/// [`ControlError::InvalidConfig`] if any bound is NaN, if
/// `output_min > output_max`, or if `integral_max < 0`.
fn validate_limits(limits: &PidLimits) -> Result<(), ControlError> {
    if limits.output_min.is_nan()
        || limits.output_max.is_nan()
        || limits.integral_max.is_nan()
        || limits.output_min > limits.output_max
        || limits.integral_max < 0.0
    {
        Err(ControlError::InvalidConfig)
    } else {
        Ok(())
    }
}

/// Builder for PID controllers.
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlError, PidControllerBuilder, PidGains, PidLimits};
///
/// let pid = PidControllerBuilder::new()
///     .gains(2.0, 0.5, 0.1)
///     .limits(-100.0, 100.0, 50.0)
///     .build()?;
/// assert_eq!(pid.gains(), PidGains::new(2.0, 0.5, 0.1));
/// assert_eq!(pid.limits(), PidLimits::new(-100.0, 100.0, 50.0));
///
/// // `build` fails until both gains and limits are supplied...
/// assert_eq!(
///     PidControllerBuilder::new().gains(2.0, 0.5, 0.1).build(),
///     Err(ControlError::InvalidConfig)
/// );
/// // ...and when the limits cannot be applied.
/// assert_eq!(
///     PidControllerBuilder::new().gains(2.0, 0.5, 0.1).limits(1.0, -1.0, 5.0).build(),
///     Err(ControlError::InvalidConfig)
/// );
/// # Ok::<(), ControlError>(())
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PidControllerBuilder {
    gains: Option<PidGains>,
    limits: Option<PidLimits>,
}

impl PidControllerBuilder {
    /// Creates a new PID controller builder.
    pub const fn new() -> Self {
        Self {
            gains: None,
            limits: None,
        }
    }

    /// Sets the PID gains.
    ///
    /// # Parameters
    /// - `kp`: Proportional gain
    /// - `ki`: Integral gain
    /// - `kd`: Derivative gain
    pub fn gains(mut self, kp: f32, ki: f32, kd: f32) -> Self {
        self.gains = Some(PidGains::new(kp, ki, kd));
        self
    }

    /// Sets the PID limits.
    ///
    /// # Parameters
    /// - `output_min`: Minimum output value
    /// - `output_max`: Maximum output value
    /// - `integral_max`: Maximum absolute integral value
    pub fn limits(mut self, output_min: f32, output_max: f32, integral_max: f32) -> Self {
        self.limits = Some(PidLimits::new(output_min, output_max, integral_max));
        self
    }

    /// Builds the PID controller.
    ///
    /// The limits are validated with the same rule that `update` applies, so
    /// an unusable configuration is caught at construction rather than on
    /// the first update. Gains are not validated: a NaN gain is reported by
    /// `update` as `NumericalError`.
    ///
    /// # Errors
    /// [`ControlError::InvalidConfig`] if gains or limits were not set, if
    /// any limit is NaN, if `output_min > output_max`, or if
    /// `integral_max < 0`.
    pub fn build(self) -> Result<PidController, ControlError> {
        let gains = self.gains.ok_or(ControlError::InvalidConfig)?;
        let limits = self.limits.ok_or(ControlError::InvalidConfig)?;
        validate_limits(&limits)?;
        Ok(PidController::new(gains, limits))
    }
}

impl Default for PidControllerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::float_cmp))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::block::fault_vectors::{BAD_DT, NON_FINITE, TINY_DT};
    use crate::test_support::assert_close;

    fn pid(kp: f32, ki: f32, kd: f32, limits: PidLimits) -> PidController {
        PidController::new(PidGains::new(kp, ki, kd), limits)
    }

    fn wide() -> PidLimits {
        PidLimits::new(-100.0, 100.0, 50.0)
    }

    /// Verifies: REQ-CTL-010
    #[test]
    fn test_pid_gains_new() {
        let gains = PidGains::new(1.0, 2.0, 3.0);
        assert_eq!(gains.kp, 1.0);
        assert_eq!(gains.ki, 2.0);
        assert_eq!(gains.kd, 3.0);
    }

    #[test]
    fn test_pid_limits_new() {
        let limits = PidLimits::new(-10.0, 10.0, 5.0);
        assert_eq!(limits.output_min, -10.0);
        assert_eq!(limits.output_max, 10.0);
        assert_eq!(limits.integral_max, 5.0);
    }

    #[test]
    fn test_pid_state_new() {
        let state = PidState::new();
        assert_eq!(state.integral, 0.0);
        assert_eq!(state.prev_error, 0.0);
        assert_eq!(state.prev_measurement, None);
    }

    /// Verifies: REQ-CTL-010
    #[test]
    fn test_pid_controller_new() {
        let gains = PidGains::new(1.0, 0.5, 0.1);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        assert_eq!(pid.gains(), gains);
        assert_eq!(pid.limits(), limits);
        assert_eq!(pid.state(), PidState::new());
    }

    /// Verifies: REQ-CTL-010
    #[test]
    fn test_pid_proportional_only() {
        let gains = PidGains::new(2.0, 0.0, 0.0);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        let setpoint = 100.0;
        let measurement = 95.0;
        let dt = 0.01;

        let (_, output) = pid.update((setpoint, measurement), dt).unwrap();
        // Error = 5.0, P-term = 2.0 * 5.0 = 10.0
        assert_close(output, 10.0);
    }

    /// Verifies: REQ-CTL-010
    #[test]
    fn test_pid_integral_accumulation() {
        let gains = PidGains::new(0.0, 1.0, 0.0);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        let setpoint = 100.0;
        let measurement = 90.0;
        let dt = 0.1;

        // First update: integral = 10.0 * 0.1 = 1.0, output = 1.0
        let (pid, output) = pid.update((setpoint, measurement), dt).unwrap();
        assert_close(output, 1.0);

        // Second update: integral = 1.0 + 10.0 * 0.1 = 2.0, output = 2.0
        let (_, output) = pid.update((setpoint, measurement), dt).unwrap();
        assert_close(output, 2.0);
    }

    /// Verifies: REQ-CTL-011
    #[test]
    fn test_pid_integral_clamping() {
        let gains = PidGains::new(0.0, 1.0, 0.0);
        let limits = PidLimits::new(-100.0, 100.0, 5.0);
        let pid = PidController::new(gains, limits);

        let setpoint = 100.0;
        let measurement = 0.0;
        let dt = 1.0;

        // Error = 100.0, integral would be 100.0 but clamped to 5.0
        let (pid, output) = pid.update((setpoint, measurement), dt).unwrap();
        assert_close(output, 5.0);
        assert_eq!(pid.state().integral, 5.0);
    }

    /// Verifies: REQ-CTL-012
    #[test]
    fn test_pid_output_saturation() {
        let gains = PidGains::new(10.0, 0.0, 0.0);
        let limits = PidLimits::new(-50.0, 50.0, 100.0);
        let pid = PidController::new(gains, limits);

        let setpoint = 100.0;
        let measurement = 0.0;
        let dt = 0.01;

        // P-term = 10.0 * 100.0 = 1000.0, but saturated to 50.0
        let (_, output) = pid.update((setpoint, measurement), dt).unwrap();
        assert_eq!(output, 50.0);
    }

    /// Verifies: REQ-CTL-010, REQ-CTL-013
    #[test]
    fn test_pid_derivative_on_measurement() {
        let gains = PidGains::new(0.0, 0.0, 1.0);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        let setpoint = 100.0;
        let dt = 0.1;

        // First update: no previous measurement, so D-term = 0.0
        let (pid, output) = pid.update((setpoint, 90.0), dt).unwrap();
        assert_close(output, 0.0);
        assert_eq!(pid.state().prev_measurement, Some(90.0));

        // Second update: measurement changes from 90.0 to 95.0
        // derivative = (95.0 - 90.0) / 0.1 = 50.0
        // D-term = -1.0 * 50.0 = -50.0 (negative because we want to oppose the change)
        let (_, output) = pid.update((setpoint, 95.0), dt).unwrap();
        assert_close(output, -50.0);
    }

    /// REQ-CTL-013: the first update has no previous sample, so it must not
    /// produce a derivative kick. Output equals the P+I contribution.
    /// Verifies: REQ-CTL-013
    #[test]
    fn test_pid_first_update_has_no_derivative_kick() {
        let gains = PidGains::new(2.0, 0.5, 0.1);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        let (setpoint, measurement, dt) = (100.0, 95.0, 0.01);
        let (_, output) = pid.update((setpoint, measurement), dt).unwrap();

        // error = 5.0; P = 2.0 * 5.0 = 10.0; integral = 5.0 * 0.01 = 0.05;
        // I = 0.5 * 0.05 = 0.025; D = 0 (no previous sample).
        assert!(
            output > 0.0,
            "first output must not kick negative: {output}"
        );
        assert!((output - 10.025).abs() < 1e-4, "output = {output}");
    }

    /// Verifies: REQ-CTL-002, REQ-CTL-013
    #[test]
    fn test_pid_reset() {
        let gains = PidGains::new(1.0, 1.0, 1.0);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        // Run a few updates to accumulate state
        let (pid, _) = pid.update((100.0, 90.0), 0.1).unwrap();
        let (pid, _) = pid.update((100.0, 92.0), 0.1).unwrap();

        // Reset returns to "no previous sample"
        let pid = pid.reset();
        assert_eq!(pid.state(), PidState::new());
        assert_eq!(pid.state().prev_measurement, None);

        // So the first update after reset has no derivative kick either:
        // P = 1.0 * 10.0, I = 1.0 * (10.0 * 0.1), D = 0
        let (_, output) = pid.update((100.0, 90.0), 0.1).unwrap();
        assert!((output - 11.0).abs() < 1e-4, "output = {output}");
    }

    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_invalid_inputs() {
        let gains = PidGains::new(1.0, 0.5, 0.1);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        assert_eq!(
            pid.update((f32::NAN, 0.0), 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
        assert_eq!(
            pid.update((0.0, f32::INFINITY), 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
        assert_eq!(
            pid.update((0.0, 0.0), 0.0).unwrap_err(),
            ControlError::InvalidDelta
        );
        assert_eq!(
            pid.update((0.0, 0.0), -0.01).unwrap_err(),
            ControlError::InvalidDelta
        );
    }

    /// Verifies: REQ-CTL-014
    #[test]
    fn test_pid_builder() {
        let pid = PidControllerBuilder::new()
            .gains(2.0, 0.5, 0.1)
            .limits(-100.0, 100.0, 50.0)
            .build()
            .unwrap();

        assert_eq!(pid.gains().kp, 2.0);
        assert_eq!(pid.gains().ki, 0.5);
        assert_eq!(pid.gains().kd, 0.1);
        assert_eq!(pid.limits().output_min, -100.0);
        assert_eq!(pid.limits().output_max, 100.0);
        assert_eq!(pid.limits().integral_max, 50.0);
    }

    /// Error class "missing configuration": a builder with no gains, no
    /// limits, or neither is rejected with `InvalidConfig` (no panic).
    /// Verifies: REQ-CTL-014, REQ-PLT-010, REQ-CTL-005
    #[test]
    fn test_pid_builder_incomplete() {
        let pid = PidControllerBuilder::new().gains(1.0, 0.5, 0.1).build();
        assert_eq!(pid, Err(ControlError::InvalidConfig));

        let pid = PidControllerBuilder::new()
            .limits(-100.0, 100.0, 50.0)
            .build();
        assert_eq!(pid, Err(ControlError::InvalidConfig));

        assert_eq!(
            PidControllerBuilder::new().build(),
            Err(ControlError::InvalidConfig)
        );
    }

    /// Every invalid limit class (misordered output limits, NaN in each
    /// bound, negative integral bound, including the smallest negative
    /// value) is rejected by the builder with `InvalidConfig`, so the
    /// controller cannot be constructed. Boundary limits (equal output
    /// limits, zero and infinite bounds) build successfully.
    /// Verifies: REQ-CTL-014, REQ-PLT-010, REQ-CTL-005
    #[test]
    fn test_pid_builder_rejects_invalid_limits() {
        for (min, max, imax) in [
            (1.0, -1.0, 10.0),
            (1.0_f32.next_up(), 1.0, 10.0),
            (f32::INFINITY, f32::NEG_INFINITY, 10.0),
            (f32::NAN, 1.0, 10.0),
            (-1.0, f32::NAN, 10.0),
            (-1.0, 1.0, f32::NAN),
            (-1.0, 1.0, -0.5),
            (-1.0, 1.0, -f32::from_bits(1)),
            (-1.0, 1.0, f32::NEG_INFINITY),
        ] {
            let built = PidControllerBuilder::new()
                .gains(1.0, 1.0, 0.0)
                .limits(min, max, imax)
                .build();
            assert_eq!(
                built,
                Err(ControlError::InvalidConfig),
                "({min}, {max}, {imax})"
            );
        }
        for (min, max, imax) in [
            (3.0, 3.0, 0.0),
            (-1.0, 1.0, -0.0),
            (f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY),
        ] {
            let built = PidControllerBuilder::new()
                .gains(1.0, 1.0, 0.0)
                .limits(min, max, imax)
                .build()
                .unwrap();
            assert_eq!(built.limits(), PidLimits::new(min, max, imax));
        }
    }

    /// Every non-finite setpoint and measurement is rejected with
    /// `InvalidInput`, also on a controller that already has a previous
    /// sample.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_rejects_every_non_finite_input() {
        let fresh = pid(1.0, 0.5, 0.1, wide());
        let (primed, _) = fresh.update((1.0, 0.0), 0.01).unwrap();
        for c in [fresh, primed] {
            for bad in NON_FINITE {
                assert_eq!(c.update((bad, 0.0), 0.01), Err(ControlError::InvalidInput));
                assert_eq!(c.update((0.0, bad), 0.01), Err(ControlError::InvalidInput));
            }
        }
    }

    /// Every zero, negative or non-finite time step is rejected with
    /// `InvalidDelta`; the smallest positive normal step is accepted.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_rejects_every_invalid_dt() {
        let c = pid(1.0, 0.5, 0.0, wide());
        for dt in BAD_DT {
            assert_eq!(
                c.update((1.0, 0.0), dt),
                Err(ControlError::InvalidDelta),
                "dt = {dt}"
            );
        }
        let (_, out) = c.update((1.0, 0.0), TINY_DT).unwrap();
        assert_close(out, 1.0);
    }

    /// A non-finite input is reported before an invalid dt.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_input_checked_before_dt() {
        let c = pid(1.0, 0.0, 0.0, wide());
        assert_eq!(
            c.update((f32::NAN, 0.0), 0.0),
            Err(ControlError::InvalidInput)
        );
    }

    /// Overflow of the proportional term is reported as `NumericalError`
    /// even though output saturation would otherwise clamp it to a finite
    /// limit.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_proportional_overflow_is_numerical_error() {
        let c = pid(1.0e30, 0.0, 0.0, wide());
        assert_eq!(
            c.update((1.0e30, 0.0), 0.01),
            Err(ControlError::NumericalError)
        );
    }

    /// The error itself overflows (MAX - (-MAX)); the unclamped integral is
    /// infinite, which is reported rather than clamped to `integral_max`.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_integral_overflow_is_numerical_error() {
        let c = pid(0.0, 1.0, 0.0, wide());
        assert_eq!(
            c.update((f32::MAX, -f32::MAX), 0.01),
            Err(ControlError::NumericalError)
        );
        // error finite but error * dt overflows
        assert_eq!(
            c.update((1.0e30, 0.0), 1.0e30),
            Err(ControlError::NumericalError)
        );
    }

    /// A measurement jump over a tiny dt overflows the derivative term.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_derivative_overflow_is_numerical_error() {
        let c = pid(0.0, 0.0, 1.0, wide());
        let (c, _) = c.update((0.0, -1.0e30), 1.0).unwrap();
        assert_eq!(
            c.update((0.0, 1.0e30), TINY_DT),
            Err(ControlError::NumericalError)
        );
    }

    /// A NaN gain makes the output NaN, which is reported.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_pid_nan_gain_is_numerical_error() {
        for g in [
            PidGains::new(f32::NAN, 0.0, 0.0),
            PidGains::new(0.0, f32::NAN, 0.0),
            PidGains::new(0.0, 0.0, f32::NAN),
        ] {
            // kd NaN on the first sample multiplies a zero derivative:
            // NaN * 0 is NaN, so every case fails on the first update.
            let c = PidController::new(g, wide());
            assert_eq!(
                c.update((1.0, 0.0), 0.1).map(|_| ()),
                Err(ControlError::NumericalError),
                "{g:?}"
            );
        }
    }

    /// Limits that `f32::clamp` would panic on are rejected with
    /// `InvalidConfig` instead (fault injection on the configuration). This
    /// covers controllers made with `PidController::new`, which bypasses the
    /// builder's validation.
    /// Verifies: REQ-CTL-005
    #[test]
    fn test_pid_invalid_limits_are_rejected() {
        for limits in [
            PidLimits::new(1.0, -1.0, 10.0),
            PidLimits::new(f32::NAN, 1.0, 10.0),
            PidLimits::new(-1.0, f32::NAN, 10.0),
            PidLimits::new(-1.0, 1.0, -0.5),
            PidLimits::new(-1.0, 1.0, f32::NAN),
        ] {
            let c = pid(1.0, 1.0, 0.0, limits);
            assert_eq!(
                c.update((1.0, 0.0), 0.1),
                Err(ControlError::InvalidConfig),
                "{limits:?}"
            );
        }
    }

    /// Boundary configurations are valid: equal output limits pin the
    /// output, a zero integral bound disables the integral, and infinite
    /// bounds mean "unlimited".
    /// Verifies: REQ-CTL-011, REQ-CTL-012
    #[test]
    fn test_pid_boundary_limits_are_valid() {
        let c = pid(1.0, 1.0, 0.0, PidLimits::new(3.0, 3.0, 0.0));
        let (c, out) = c.update((10.0, 0.0), 0.1).unwrap();
        assert_eq!(out, 3.0);
        assert_eq!(c.state().integral, 0.0);

        let inf = PidLimits::new(f32::NEG_INFINITY, f32::INFINITY, f32::INFINITY);
        let c = pid(2.0, 0.0, 0.0, inf);
        let (_, out) = c.update((1.0e20, 0.0), 0.1).unwrap();
        assert_close(out, 2.0e20);
    }

    /// Under sustained positive error the integral rises to exactly
    /// `integral_max` and stays there; when the error reverses it recovers
    /// immediately from the clamp (no wound-up excess to unwind), and it
    /// clamps at exactly `-integral_max` on the negative side. All values
    /// are exactly representable, so they are compared with `==`.
    /// Verifies: REQ-CTL-011
    #[test]
    fn test_pid_anti_windup_sustained_error_and_recovery() {
        let mut c = pid(0.0, 1.0, 0.0, PidLimits::new(-100.0, 100.0, 5.0));
        // error 4.0, dt 0.5: integral steps 2, 4, then clamps at 5.
        let expected = [2.0, 4.0, 5.0];
        for e in expected {
            let (n, out) = c.update((4.0, 0.0), 0.5).unwrap();
            assert_eq!(n.state().integral, e);
            assert_eq!(out, e);
            c = n;
        }
        for _ in 0..1000 {
            let (n, out) = c.update((4.0, 0.0), 0.5).unwrap();
            assert_eq!(n.state().integral, 5.0);
            assert_eq!(out, 5.0);
            c = n;
        }
        // Recovery: one step of error -4.0 takes 5.0 to 3.0 at once.
        let (n, out) = c.update((0.0, 4.0), 0.5).unwrap();
        assert_eq!(n.state().integral, 3.0);
        assert_eq!(out, 3.0);
        c = n;
        for _ in 0..1000 {
            let (n, _) = c.update((0.0, 4.0), 0.5).unwrap();
            c = n;
        }
        assert_eq!(c.state().integral, -5.0);
    }

    /// The output equals each limit exactly when the unsaturated value is
    /// exactly at the limit, and when it is beyond the limit.
    /// Verifies: REQ-CTL-012
    #[test]
    fn test_pid_output_exactly_at_limits() {
        let limits = PidLimits::new(-20.0, 50.0, 100.0);
        let c = pid(10.0, 0.0, 0.0, limits);
        assert_eq!(c.update((5.0, 0.0), 0.01).unwrap().1, 50.0);
        assert_eq!(c.update((5.5, 0.0), 0.01).unwrap().1, 50.0);
        assert_eq!(c.update((4.5, 0.0), 0.01).unwrap().1, 45.0);
        assert_eq!(c.update((-2.0, 0.0), 0.01).unwrap().1, -20.0);
        assert_eq!(c.update((-2.5, 0.0), 0.01).unwrap().1, -20.0);
        assert_eq!(c.update((-1.5, 0.0), 0.01).unwrap().1, -15.0);
    }

    /// All three gains act together: output = Kp*e + Ki*integral - Kd*dmeas/dt.
    /// Verifies: REQ-CTL-010
    #[test]
    fn test_pid_combined_gains() {
        let c = pid(2.0, 0.5, 0.25, wide());
        let (c, _) = c.update((10.0, 0.0), 0.5).unwrap();
        // e = 8, integral = 5 + 4 = 9, dmeas/dt = 2/0.5 = 4
        let (_, out) = c.update((10.0, 2.0), 0.5).unwrap();
        assert_close(out, 2.0 * 8.0 + 0.5 * 9.0 - 0.25 * 4.0);
    }

    /// The builder's `Default` matches `new`, and so does `PidState`'s.
    /// Verifies: REQ-CTL-014
    #[test]
    fn test_pid_defaults() {
        assert_eq!(
            PidControllerBuilder::default().build(),
            Err(ControlError::InvalidConfig)
        );
        let built = PidControllerBuilder::default()
            .limits(-1.0, 1.0, 1.0)
            .gains(1.0, 2.0, 3.0)
            .build()
            .unwrap();
        assert_eq!(
            built,
            PidController::new(PidGains::new(1.0, 2.0, 3.0), PidLimits::new(-1.0, 1.0, 1.0))
        );
        assert_eq!(PidState::default(), PidState::new());
    }
}
