//! Digital filter implementations.
//!
//! Provides first-order and second-order digital filters for embedded control systems.
//!
//! # Traceability
//! - REQ-CTL-020: First-order low-pass filter
//! - REQ-CTL-021: First-order high-pass filter
//! - REQ-CTL-022: Second-order notch filter
//! - REQ-CTL-023: Filter coefficients computed from cutoff frequency

use super::{ControlBlock, ControlError};

// Use libm for sin/cos in no_std environments
#[allow(unused_imports)]
use nalgebra::ComplexField;

/// First-order low-pass filter.
///
/// Implements a discrete-time low-pass filter using exponential smoothing:
/// ```text
/// y[k] = alpha * x[k] + (1 - alpha) * y[k-1]
/// ```
///
/// where alpha is computed from the cutoff frequency and sample rate.
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, LowPassFilter};
///
/// let cutoff_hz = 10.0;
/// let sample_rate_hz = 1000.0;
/// let lpf = LowPassFilter::from_cutoff(cutoff_hz, sample_rate_hz);
///
/// let (lpf, output) = lpf.update(1.0, 0.001).unwrap();
/// // The first step of a unit input moves only part of the way.
/// assert!(output > 0.0 && output < 0.1);
/// assert_eq!(lpf.state(), output);
///
/// // A sustained step converges towards the input.
/// let mut lpf = lpf;
/// let mut output = output;
/// for _ in 0..1000 {
///     (lpf, output) = lpf.update(1.0, 0.001).unwrap();
/// }
/// assert!((output - 1.0).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LowPassFilter {
    /// Smoothing factor (0.0 to 1.0).
    alpha: f32,
    /// Previous output value.
    state: f32,
}

impl LowPassFilter {
    /// Creates a new low-pass filter with specified alpha.
    ///
    /// # Parameters
    /// - `alpha`: Smoothing factor (0.0 to 1.0)
    ///
    /// # Returns
    /// A new low-pass filter with state initialized to 0.0
    pub const fn new(alpha: f32) -> Self {
        Self { alpha, state: 0.0 }
    }

    /// Creates a low-pass filter from cutoff frequency and sample rate.
    ///
    /// # Parameters
    /// - `cutoff_hz`: Cutoff frequency in Hz
    /// - `sample_rate_hz`: Sample rate in Hz
    ///
    /// # Returns
    /// A new low-pass filter
    ///
    /// # Formula
    /// ```text
    /// RC = 1 / (2 * pi * cutoff_hz)
    /// dt = 1 / sample_rate_hz
    /// alpha = dt / (RC + dt)
    /// ```
    pub fn from_cutoff(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        let rc = 1.0 / (2.0 * core::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        let alpha = dt / (rc + dt);
        Self::new(alpha)
    }

    /// Returns the current state.
    pub fn state(&self) -> f32 {
        self.state
    }
}

impl ControlBlock for LowPassFilter {
    type Input = f32;
    type Output = f32;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        if !input.is_finite() {
            return Err(ControlError::InvalidInput);
        }
        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        let output = self.alpha * input + (1.0 - self.alpha) * self.state;

        if !output.is_finite() {
            return Err(ControlError::NumericalError);
        }

        Ok((
            Self {
                alpha: self.alpha,
                state: output,
            },
            output,
        ))
    }

    fn reset(self) -> Self {
        Self {
            alpha: self.alpha,
            state: 0.0,
        }
    }
}

/// First-order high-pass filter.
///
/// Implements a discrete-time high-pass filter:
/// ```text
/// y[k] = alpha * (y[k-1] + x[k] - x[k-1])
/// ```
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, HighPassFilter};
///
/// let cutoff_hz = 10.0;
/// let sample_rate_hz = 1000.0;
/// let hpf = HighPassFilter::from_cutoff(cutoff_hz, sample_rate_hz);
///
/// let (hpf, output) = hpf.update(1.0, 0.001).unwrap();
/// // The edge of a step passes almost unattenuated.
/// assert!(output > 0.9 && output < 1.0);
///
/// // A constant (DC) input decays towards zero.
/// let mut hpf = hpf;
/// let mut output = output;
/// for _ in 0..1000 {
///     (hpf, output) = hpf.update(1.0, 0.001).unwrap();
/// }
/// assert!(output.abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighPassFilter {
    /// Smoothing factor (0.0 to 1.0).
    alpha: f32,
    /// Previous input value.
    prev_input: f32,
    /// Previous output value.
    prev_output: f32,
}

impl HighPassFilter {
    /// Creates a new high-pass filter with specified alpha.
    ///
    /// # Parameters
    /// - `alpha`: Smoothing factor (0.0 to 1.0)
    ///
    /// # Returns
    /// A new high-pass filter with state initialized to 0.0
    pub const fn new(alpha: f32) -> Self {
        Self {
            alpha,
            prev_input: 0.0,
            prev_output: 0.0,
        }
    }

    /// Creates a high-pass filter from cutoff frequency and sample rate.
    ///
    /// # Parameters
    /// - `cutoff_hz`: Cutoff frequency in Hz
    /// - `sample_rate_hz`: Sample rate in Hz
    ///
    /// # Returns
    /// A new high-pass filter
    pub fn from_cutoff(cutoff_hz: f32, sample_rate_hz: f32) -> Self {
        let rc = 1.0 / (2.0 * core::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        let alpha = rc / (rc + dt);
        Self::new(alpha)
    }
}

impl ControlBlock for HighPassFilter {
    type Input = f32;
    type Output = f32;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        if !input.is_finite() {
            return Err(ControlError::InvalidInput);
        }
        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        let output = self.alpha * (self.prev_output + input - self.prev_input);

        if !output.is_finite() {
            return Err(ControlError::NumericalError);
        }

        Ok((
            Self {
                alpha: self.alpha,
                prev_input: input,
                prev_output: output,
            },
            output,
        ))
    }

    fn reset(self) -> Self {
        Self {
            alpha: self.alpha,
            prev_input: 0.0,
            prev_output: 0.0,
        }
    }
}

/// Second-order notch filter coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BiquadCoefficients {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Second-order notch filter state.
#[derive(Debug, Clone, Copy, PartialEq)]
struct BiquadState {
    x1: f32, // x[k-1]
    x2: f32, // x[k-2]
    y1: f32, // y[k-1]
    y2: f32, // y[k-2]
}

/// Second-order notch filter.
///
/// Implements a discrete-time notch filter using biquad structure.
/// The transfer function is:
/// ```text
/// H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
/// ```
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, NotchFilter};
///
/// let notch_hz = 60.0;  // Remove 60 Hz noise
/// let bandwidth_hz = 5.0;
/// let sample_rate_hz = 1000.0;
/// let notch = NotchFilter::from_frequency(notch_hz, bandwidth_hz, sample_rate_hz);
///
/// // Feed a unit-amplitude 60 Hz sine and measure the settled output.
/// let dt = 1.0 / sample_rate_hz;
/// let mut notch = notch;
/// let mut peak: f32 = 0.0;
/// for k in 0..2000 {
///     let t = k as f32 * dt;
///     let signal = (2.0 * core::f32::consts::PI * notch_hz * t).sin();
///     let output;
///     (notch, output) = notch.update(signal, dt).unwrap();
///     if k >= 1500 {
///         peak = peak.max(output.abs());
///     }
/// }
/// assert!(peak < 0.05, "60 Hz should be strongly attenuated, peak = {peak}");
///
/// // DC passes through with unity gain.
/// let mut notch = notch.reset();
/// let mut output = 0.0;
/// for _ in 0..2000 {
///     (notch, output) = notch.update(1.0, dt).unwrap();
/// }
/// assert!((output - 1.0).abs() < 1e-3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotchFilter {
    coefficients: BiquadCoefficients,
    state: BiquadState,
}

impl NotchFilter {
    /// Creates a notch filter from center frequency, bandwidth, and sample rate.
    ///
    /// # Parameters
    /// - `notch_hz`: Center frequency to attenuate in Hz
    /// - `bandwidth_hz`: Bandwidth of the notch in Hz
    /// - `sample_rate_hz`: Sample rate in Hz
    ///
    /// # Returns
    /// A new notch filter
    pub fn from_frequency(notch_hz: f32, bandwidth_hz: f32, sample_rate_hz: f32) -> Self {
        let omega = 2.0 * core::f32::consts::PI * notch_hz / sample_rate_hz;
        let bw = 2.0 * core::f32::consts::PI * bandwidth_hz / sample_rate_hz;

        let alpha = ComplexField::sin(bw / 2.0);
        let cos_omega = ComplexField::cos(omega);

        // Notch filter coefficients
        let b0 = 1.0;
        let b1 = -2.0 * cos_omega;
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_omega / a0;
        let a2 = (1.0 - alpha) / a0;

        let coefficients = BiquadCoefficients {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1,
            a2,
        };

        let state = BiquadState {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        };

        Self {
            coefficients,
            state,
        }
    }
}

impl ControlBlock for NotchFilter {
    type Input = f32;
    type Output = f32;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        if !input.is_finite() {
            return Err(ControlError::InvalidInput);
        }
        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        let c = &self.coefficients;
        let s = &self.state;

        // Compute output using Direct Form II
        let output = c.b0 * input + c.b1 * s.x1 + c.b2 * s.x2 - c.a1 * s.y1 - c.a2 * s.y2;

        if !output.is_finite() {
            return Err(ControlError::NumericalError);
        }

        // Update state
        let new_state = BiquadState {
            x1: input,
            x2: s.x1,
            y1: output,
            y2: s.y1,
        };

        Ok((
            Self {
                coefficients: self.coefficients,
                state: new_state,
            },
            output,
        ))
    }

    fn reset(self) -> Self {
        Self {
            coefficients: self.coefficients,
            state: BiquadState {
                x1: 0.0,
                x2: 0.0,
                y1: 0.0,
                y2: 0.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-6;

    #[test]
    fn test_lowpass_new() {
        let lpf = LowPassFilter::new(0.5);
        assert_eq!(lpf.alpha, 0.5);
        assert_eq!(lpf.state(), 0.0);
    }

    #[test]
    fn test_lowpass_from_cutoff() {
        let lpf = LowPassFilter::from_cutoff(10.0, 1000.0);
        assert!(lpf.alpha > 0.0 && lpf.alpha < 1.0);
    }

    #[test]
    fn test_lowpass_filtering() {
        let lpf = LowPassFilter::new(0.5);

        let (lpf, output) = lpf.update(1.0, 0.01).unwrap();
        assert_eq!(output, 0.5); // 0.5 * 1.0 + 0.5 * 0.0

        let (_, output) = lpf.update(1.0, 0.01).unwrap();
        assert_eq!(output, 0.75); // 0.5 * 1.0 + 0.5 * 0.5
    }

    #[test]
    fn test_lowpass_reset() {
        let lpf = LowPassFilter::new(0.5);
        let (lpf, _) = lpf.update(1.0, 0.01).unwrap();
        assert_ne!(lpf.state(), 0.0);

        let lpf = lpf.reset();
        assert_eq!(lpf.state(), 0.0);
    }

    #[test]
    fn test_lowpass_invalid_input() {
        let lpf = LowPassFilter::new(0.5);
        assert_eq!(
            lpf.update(f32::NAN, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
    }

    #[test]
    fn test_highpass_new() {
        let hpf = HighPassFilter::new(0.9);
        assert_eq!(hpf.alpha, 0.9);
        assert_eq!(hpf.prev_input, 0.0);
        assert_eq!(hpf.prev_output, 0.0);
    }

    #[test]
    fn test_highpass_from_cutoff() {
        let hpf = HighPassFilter::from_cutoff(10.0, 1000.0);
        assert!(hpf.alpha > 0.0 && hpf.alpha < 1.0);
    }

    #[test]
    fn test_highpass_dc_rejection() {
        let hpf = HighPassFilter::new(0.95);

        // Apply constant DC signal
        let mut hpf = hpf;
        for _ in 0..100 {
            let (new_hpf, _) = hpf.update(1.0, 0.01).unwrap();
            hpf = new_hpf;
        }

        // Output should approach zero for DC input
        let (_, output) = hpf.update(1.0, 0.01).unwrap();
        assert!(output.abs() < 0.1);
    }

    #[test]
    fn test_highpass_reset() {
        let hpf = HighPassFilter::new(0.9);
        let (hpf, _) = hpf.update(1.0, 0.01).unwrap();

        let hpf = hpf.reset();
        assert_eq!(hpf.prev_input, 0.0);
        assert_eq!(hpf.prev_output, 0.0);
    }

    #[test]
    fn test_notch_from_frequency() {
        let notch = NotchFilter::from_frequency(60.0, 5.0, 1000.0);
        assert!(notch.coefficients.b0.is_finite());
        assert!(notch.coefficients.b1.is_finite());
        assert!(notch.coefficients.b2.is_finite());
        assert!(notch.coefficients.a1.is_finite());
        assert!(notch.coefficients.a2.is_finite());
    }

    #[test]
    fn test_notch_filtering() {
        let notch = NotchFilter::from_frequency(10.0, 2.0, 1000.0);

        // Apply a signal
        let (notch, output1) = notch.update(1.0, 0.001).unwrap();
        let (_, output2) = notch.update(0.5, 0.001).unwrap();

        assert!(output1.is_finite());
        assert!(output2.is_finite());
    }

    #[test]
    fn test_notch_reset() {
        let notch = NotchFilter::from_frequency(60.0, 5.0, 1000.0);
        let (notch, _) = notch.update(1.0, 0.001).unwrap();

        let notch = notch.reset();
        assert_eq!(notch.state.x1, 0.0);
        assert_eq!(notch.state.x2, 0.0);
        assert_eq!(notch.state.y1, 0.0);
        assert_eq!(notch.state.y2, 0.0);
    }

    #[test]
    fn test_notch_invalid_input() {
        let notch = NotchFilter::from_frequency(60.0, 5.0, 1000.0);
        assert_eq!(
            notch.update(f32::INFINITY, 0.001).unwrap_err(),
            ControlError::InvalidInput
        );
    }
}
