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
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, PidControllerBuilder};
///
/// let pid = PidControllerBuilder::new()
///     .gains(2.0, 0.5, 0.1)
///     .limits(-100.0, 100.0, 50.0)
///     .build()
///     .expect("gains and limits are both set");
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

        // Compute error
        let error = setpoint - measurement;

        // Proportional term
        let p_term = self.gains.kp * error;

        // Integral term with anti-windup
        let integral = self.state.integral + error * dt;
        let integral = integral.clamp(-self.limits.integral_max, self.limits.integral_max);
        let i_term = self.gains.ki * integral;

        // Derivative term (on measurement to avoid setpoint kick). With no
        // previous sample there is no rate to estimate, so it is zero.
        let derivative = match self.state.prev_measurement {
            Some(prev) => (measurement - prev) / dt,
            None => 0.0,
        };
        let d_term = -self.gains.kd * derivative; // Negative because we're using derivative of measurement

        // Compute output
        let output = p_term + i_term + d_term;

        // Saturate output
        let output = output.clamp(self.limits.output_min, self.limits.output_max);

        // Check for numerical errors
        if !output.is_finite() {
            return Err(ControlError::NumericalError);
        }

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

/// Builder for PID controllers.
///
/// # Examples
///
/// ```
/// use middleware::control::{PidControllerBuilder, PidGains, PidLimits};
///
/// let pid = PidControllerBuilder::new()
///     .gains(2.0, 0.5, 0.1)
///     .limits(-100.0, 100.0, 50.0)
///     .build()
///     .expect("gains and limits are both set");
/// assert_eq!(pid.gains(), PidGains::new(2.0, 0.5, 0.1));
/// assert_eq!(pid.limits(), PidLimits::new(-100.0, 100.0, 50.0));
///
/// // `build` returns `None` until both gains and limits are supplied.
/// assert!(PidControllerBuilder::new().gains(2.0, 0.5, 0.1).build().is_none());
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
    /// # Returns
    /// A new PID controller, or None if gains or limits were not set
    pub fn build(self) -> Option<PidController> {
        let gains = self.gains?;
        let limits = self.limits?;
        Some(PidController::new(gains, limits))
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
    use crate::test_support::assert_close;

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

    #[test]
    fn test_pid_controller_new() {
        let gains = PidGains::new(1.0, 0.5, 0.1);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        assert_eq!(pid.gains(), gains);
        assert_eq!(pid.limits(), limits);
        assert_eq!(pid.state(), PidState::new());
    }

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
    #[test]
    fn test_pid_first_update_has_no_derivative_kick() {
        let gains = PidGains::new(2.0, 0.5, 0.1);
        let limits = PidLimits::new(-100.0, 100.0, 50.0);
        let pid = PidController::new(gains, limits);

        let (setpoint, measurement, dt) = (100.0, 95.0, 0.01);
        let (_, output) = pid.update((setpoint, measurement), dt).unwrap();

        // error = 5.0; P = 2.0 * 5.0 = 10.0; integral = 5.0 * 0.01 = 0.05;
        // I = 0.5 * 0.05 = 0.025; D = 0 (no previous sample).
        assert!(output > 0.0, "first output must not kick negative: {output}");
        assert!((output - 10.025).abs() < 1e-4, "output = {output}");
    }

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

    #[test]
    fn test_pid_builder_incomplete() {
        let pid = PidControllerBuilder::new().gains(1.0, 0.5, 0.1).build();
        assert!(pid.is_none());

        let pid = PidControllerBuilder::new()
            .limits(-100.0, 100.0, 50.0)
            .build();
        assert!(pid.is_none());
    }
}
