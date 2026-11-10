//! Digital filter implementations.
//!
//! Provides first-order and second-order digital filters for embedded control systems.
//!
//! # Traceability
//! - REQ-CTL-020: First-order low-pass filter
//! - REQ-CTL-021: First-order high-pass filter
//! - REQ-CTL-022: Second-order notch filter
//! - REQ-CTL-023: Filter coefficients computed from cutoff frequency
//! - REQ-CTL-004: Non-finite inputs, invalid time steps and non-finite
//!   outputs are reported as errors
//!
//! # Configuration validation
//! The frequency-based constructors ([`LowPassFilter::from_cutoff`],
//! [`HighPassFilter::from_cutoff`], [`NotchFilter::from_frequency`]) return
//! `Err(ControlError::InvalidConfig)` unless the sample rate is finite and
//! positive and every frequency is finite, positive and at most the Nyquist
//! frequency (`sample_rate_hz / 2`). They also reject parameters whose
//! computed coefficients would leave the filter dead or marginally stable
//! (for example a cutoff so small that `alpha` rounds to zero).
//!
//! # Time step
//! Coefficients are fixed at construction from the sample rate. `update`
//! validates `dt` (it must be finite and positive) but does not otherwise use
//! it; the caller must call `update` at the configured sample rate.

use super::{ControlBlock, ControlError};

/// Returns `true` if `x` is finite and strictly positive.
fn is_positive_finite(x: f32) -> bool {
    x.is_finite() && x > 0.0
}

/// Checks that `sample_rate_hz` is finite and positive and that `freq_hz`
/// is finite, positive and no higher than the Nyquist frequency.
fn check_band(freq_hz: f32, sample_rate_hz: f32) -> Result<(), ControlError> {
    if is_positive_finite(sample_rate_hz)
        && is_positive_finite(freq_hz)
        && freq_hz <= sample_rate_hz * 0.5
    {
        Ok(())
    } else {
        Err(ControlError::InvalidConfig)
    }
}

/// Checks that a first-order smoothing factor lies strictly inside `(0, 1)`,
/// the range in which the filter is stable and neither dead nor a pure
/// pass-through. NaN fails both comparisons and is rejected.
fn check_alpha(alpha: f32) -> Result<f32, ControlError> {
    if alpha > 0.0 && alpha < 1.0 {
        Ok(alpha)
    } else {
        Err(ControlError::InvalidConfig)
    }
}

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
/// let lpf = LowPassFilter::from_cutoff(cutoff_hz, sample_rate_hz).unwrap();
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
    ///
    /// `alpha` is not validated here (this is a `const fn`); a value outside
    /// `(0, 1)` gives a dead or unstable filter, and a divergence that
    /// overflows is reported by `update` as `ControlError::NumericalError`.
    /// Prefer `from_cutoff`, which validates its parameters.
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
    /// # Errors
    /// `ControlError::InvalidConfig` if the sample rate is not finite and
    /// positive, if the cutoff is not finite, positive and at most
    /// `sample_rate_hz / 2`, or if the computed `alpha` is not strictly
    /// between 0 and 1.
    ///
    /// # Formula
    /// ```text
    /// RC = 1 / (2 * pi * cutoff_hz)
    /// dt = 1 / sample_rate_hz
    /// alpha = dt / (RC + dt)
    /// ```
    pub fn from_cutoff(cutoff_hz: f32, sample_rate_hz: f32) -> Result<Self, ControlError> {
        check_band(cutoff_hz, sample_rate_hz)?;
        let rc = 1.0 / (2.0 * core::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        let alpha = check_alpha(dt / (rc + dt))?;
        Ok(Self::new(alpha))
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
/// let hpf = HighPassFilter::from_cutoff(cutoff_hz, sample_rate_hz).unwrap();
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
    ///
    /// `alpha` is not validated here (this is a `const fn`); a value outside
    /// `(0, 1)` gives a dead or unstable filter, and a divergence that
    /// overflows is reported by `update` as `ControlError::NumericalError`.
    /// Prefer `from_cutoff`, which validates its parameters.
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
    ///
    /// # Errors
    /// `ControlError::InvalidConfig` under the same conditions as
    /// [`LowPassFilter::from_cutoff`].
    ///
    /// # Formula
    /// ```text
    /// RC = 1 / (2 * pi * cutoff_hz)
    /// dt = 1 / sample_rate_hz
    /// alpha = RC / (RC + dt)
    /// ```
    pub fn from_cutoff(cutoff_hz: f32, sample_rate_hz: f32) -> Result<Self, ControlError> {
        check_band(cutoff_hz, sample_rate_hz)?;
        let rc = 1.0 / (2.0 * core::f32::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        let alpha = check_alpha(rc / (rc + dt))?;
        Ok(Self::new(alpha))
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
/// let notch = NotchFilter::from_frequency(notch_hz, bandwidth_hz, sample_rate_hz).unwrap();
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
    ///
    /// # Errors
    /// `ControlError::InvalidConfig` if the sample rate is not finite and
    /// positive, if the notch frequency or the bandwidth is not finite,
    /// positive and at most `sample_rate_hz / 2`, or if the bandwidth is so
    /// small that `alpha` rounds to zero (poles on the unit circle).
    ///
    /// # Formula
    /// ```text
    /// omega = 2 * pi * notch_hz / sample_rate_hz
    /// alpha = sin(pi * bandwidth_hz / sample_rate_hz)
    /// a0 = 1 + alpha
    /// b0 = b2 = 1 / a0,  b1 = a1 = -2 * cos(omega) / a0,  a2 = (1 - alpha) / a0
    /// ```
    /// The band limits keep `alpha` in `(0, 1]`, so `a0` is in `(1, 2]` and
    /// `|a2| < 1`: every coefficient is finite and the poles lie inside the
    /// unit circle.
    pub fn from_frequency(
        notch_hz: f32,
        bandwidth_hz: f32,
        sample_rate_hz: f32,
    ) -> Result<Self, ControlError> {
        check_band(notch_hz, sample_rate_hz)?;
        check_band(bandwidth_hz, sample_rate_hz)?;
        let omega = 2.0 * core::f32::consts::PI * notch_hz / sample_rate_hz;
        let bw = 2.0 * core::f32::consts::PI * bandwidth_hz / sample_rate_hz;

        let alpha = ComplexField::sin(bw / 2.0);
        if alpha <= 0.0 {
            return Err(ControlError::InvalidConfig);
        }
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

        Ok(Self {
            coefficients,
            state,
        })
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

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::float_cmp))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::block::fault_vectors::{BAD_DT, NON_FINITE, TINY_DT};
    use crate::test_support::assert_close;

    const EPSILON: f32 = 1e-6;

    #[test]
    fn test_lowpass_new() {
        let lpf = LowPassFilter::new(0.5);
        assert_eq!(lpf.alpha, 0.5);
        assert_eq!(lpf.state(), 0.0);
    }

    #[test]
    fn test_lowpass_from_cutoff() {
        let lpf = LowPassFilter::from_cutoff(10.0, 1000.0).unwrap();
        assert!(lpf.alpha > 0.0 && lpf.alpha < 1.0);
    }

    /// Verifies: REQ-CTL-020
    #[test]
    fn test_lowpass_filtering() {
        let lpf = LowPassFilter::new(0.5);

        let (lpf, output) = lpf.update(1.0, 0.01).unwrap();
        assert!((output - 0.5).abs() < EPSILON); // 0.5 * 1.0 + 0.5 * 0.0

        let (_, output) = lpf.update(1.0, 0.01).unwrap();
        assert!((output - 0.75).abs() < EPSILON); // 0.5 * 1.0 + 0.5 * 0.5
    }

    /// Verifies: REQ-CTL-002, REQ-CTL-020
    #[test]
    fn test_lowpass_reset() {
        let lpf = LowPassFilter::new(0.5);
        let (lpf, _) = lpf.update(1.0, 0.01).unwrap();
        assert!((lpf.state() - 0.5).abs() < EPSILON);

        let lpf = lpf.reset();
        assert_eq!(lpf.state(), 0.0);
    }

    /// Verifies: REQ-CTL-004
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
        let hpf = HighPassFilter::from_cutoff(10.0, 1000.0).unwrap();
        assert!(hpf.alpha > 0.0 && hpf.alpha < 1.0);
    }

    /// Verifies: REQ-CTL-021
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

    /// Verifies: REQ-CTL-002, REQ-CTL-021
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
        let notch = NotchFilter::from_frequency(60.0, 5.0, 1000.0).unwrap();
        assert!(notch.coefficients.b0.is_finite());
        assert!(notch.coefficients.b1.is_finite());
        assert!(notch.coefficients.b2.is_finite());
        assert!(notch.coefficients.a1.is_finite());
        assert!(notch.coefficients.a2.is_finite());
    }

    #[test]
    fn test_notch_filtering() {
        let notch = NotchFilter::from_frequency(10.0, 2.0, 1000.0).unwrap();

        // Apply a signal
        let (notch, output1) = notch.update(1.0, 0.001).unwrap();
        let (_, output2) = notch.update(0.5, 0.001).unwrap();

        assert!(output1.is_finite());
        assert!(output2.is_finite());
    }

    /// Verifies: REQ-CTL-002, REQ-CTL-022
    #[test]
    fn test_notch_reset() {
        let notch = NotchFilter::from_frequency(60.0, 5.0, 1000.0).unwrap();
        let (notch, _) = notch.update(1.0, 0.001).unwrap();

        let notch = notch.reset();
        assert_eq!(notch.state.x1, 0.0);
        assert_eq!(notch.state.x2, 0.0);
        assert_eq!(notch.state.y1, 0.0);
        assert_eq!(notch.state.y2, 0.0);
    }

    /// Verifies: REQ-CTL-004
    #[test]
    fn test_notch_invalid_input() {
        let notch = NotchFilter::from_frequency(60.0, 5.0, 1000.0).unwrap();
        assert_eq!(
            notch.update(f32::INFINITY, 0.001).unwrap_err(),
            ControlError::InvalidInput
        );
    }

    /// Relative closeness for hand-derived coefficients.
    fn assert_coeff(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-5 * expected.abs().max(1.0),
            "actual = {actual}, expected = {expected}"
        );
    }

    /// Runs one block through every invalid input and dt class.
    fn assert_rejects_all<B: ControlBlock<Input = f32> + Copy>(b: B) {
        for bad in NON_FINITE {
            assert!(matches!(
                b.update(bad, 0.001),
                Err(ControlError::InvalidInput)
            ));
        }
        for dt in BAD_DT {
            assert!(
                matches!(b.update(1.0, dt), Err(ControlError::InvalidDelta)),
                "dt = {dt}"
            );
        }
        assert!(b.update(1.0, TINY_DT).is_ok());
    }

    /// Low-pass: every non-finite input and every invalid dt is rejected.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_lowpass_rejects_all_invalid_classes() {
        assert_rejects_all(LowPassFilter::new(0.5));
    }

    /// High-pass: every non-finite input and every invalid dt is rejected.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_highpass_rejects_all_invalid_classes() {
        assert_rejects_all(HighPassFilter::new(0.5));
    }

    /// Notch: every non-finite input and every invalid dt is rejected.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_notch_rejects_all_invalid_classes() {
        assert_rejects_all(NotchFilter::from_frequency(60.0, 5.0, 1000.0).unwrap());
    }

    /// An output overflow (here an unstable alpha of 2 times f32::MAX) is
    /// reported as `NumericalError` by the low-pass and high-pass filters.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_first_order_overflow_is_numerical_error() {
        assert_eq!(
            LowPassFilter::new(2.0).update(f32::MAX, 0.001),
            Err(ControlError::NumericalError)
        );
        assert_eq!(
            HighPassFilter::new(2.0).update(f32::MAX, 0.001),
            Err(ControlError::NumericalError)
        );
    }

    /// Notch output overflow is reported as `NumericalError`. The state is
    /// injected directly (fault injection): with y[k-1] = f32::MAX and
    /// a1 ~ -1.83, the `-a1 * y1` term overflows.
    /// Verifies: REQ-CTL-004
    #[test]
    fn test_notch_overflow_is_numerical_error() {
        let n = NotchFilter::from_frequency(60.0, 5.0, 1000.0).unwrap();
        let faulty = NotchFilter {
            coefficients: n.coefficients,
            state: BiquadState {
                x1: 0.0,
                x2: 0.0,
                y1: f32::MAX,
                y2: 0.0,
            },
        };
        assert_eq!(faulty.update(0.0, 0.001), Err(ControlError::NumericalError));
    }

    /// Hand-derived first-order coefficients, fc = 10 Hz, fs = 1 kHz:
    /// RC = 1/(20 pi) = 0.0159155, dt = 0.001,
    /// LPF alpha = dt/(RC+dt) = 0.0591174, HPF alpha = RC/(RC+dt) = 0.9408826.
    /// Verifies: REQ-CTL-023
    #[test]
    fn test_first_order_coefficients_hand_derived() {
        assert_coeff(
            LowPassFilter::from_cutoff(10.0, 1000.0).unwrap().alpha,
            0.059_117_4,
        );
        assert_coeff(
            HighPassFilter::from_cutoff(10.0, 1000.0).unwrap().alpha,
            0.940_882_6,
        );
    }

    /// Cutoff exactly at Nyquist (500 Hz at 1 kHz) is accepted:
    /// RC = 1/(1000 pi), dt = 1/1000, so LPF alpha = pi/(1+pi) = 0.7585470
    /// and HPF alpha = 1/(1+pi) = 0.2414530; 499 Hz gives LPF alpha =
    /// 0.0010 / (1/(998 pi) + 0.0010) = 0.7581801. Just above Nyquist is rejected.
    /// Verifies: REQ-CTL-023
    #[test]
    fn test_first_order_coefficients_at_nyquist() {
        assert_coeff(
            LowPassFilter::from_cutoff(500.0, 1000.0).unwrap().alpha,
            0.758_547,
        );
        assert_coeff(
            HighPassFilter::from_cutoff(500.0, 1000.0).unwrap().alpha,
            0.241_453,
        );
        assert_coeff(
            LowPassFilter::from_cutoff(499.0, 1000.0).unwrap().alpha,
            0.758_180_1,
        );
        assert_eq!(
            LowPassFilter::from_cutoff(500.1, 1000.0),
            Err(ControlError::InvalidConfig)
        );
        assert_eq!(
            HighPassFilter::from_cutoff(500.1, 1000.0),
            Err(ControlError::InvalidConfig)
        );
    }

    /// Invalid cutoff and sample-rate configurations are rejected by both
    /// first-order constructors: cutoff <= 0, non-finite cutoff, sample rate
    /// <= 0 or non-finite, and cutoffs so extreme that alpha leaves (0, 1).
    /// Verifies: REQ-CTL-023, REQ-CTL-005
    #[test]
    fn test_first_order_invalid_config_rejected() {
        let bad = [
            (0.0, 1000.0),
            (-0.0, 1000.0),
            (-10.0, 1000.0),
            (f32::NAN, 1000.0),
            (f32::INFINITY, 1000.0),
            (f32::NEG_INFINITY, 1000.0),
            (10.0, 0.0),
            (10.0, -1000.0),
            (10.0, f32::NAN),
            (10.0, f32::INFINITY),
            // subnormal cutoff: RC overflows to inf, LPF alpha = 0, HPF NaN
            (1.0e-45, 1000.0),
        ];
        for (fc, fs) in bad {
            assert_eq!(
                LowPassFilter::from_cutoff(fc, fs),
                Err(ControlError::InvalidConfig),
                "lpf {fc} {fs}"
            );
            assert_eq!(
                HighPassFilter::from_cutoff(fc, fs),
                Err(ControlError::InvalidConfig),
                "hpf {fc} {fs}"
            );
        }
        // fs = f32::MAX: dt is subnormal, HPF alpha rounds to exactly 1
        // (pure pass-through, no DC rejection) and is rejected; the LPF
        // alpha is tiny but positive and is accepted.
        assert_eq!(
            HighPassFilter::from_cutoff(1.0, f32::MAX),
            Err(ControlError::InvalidConfig)
        );
        assert!(LowPassFilter::from_cutoff(1.0, f32::MAX).is_ok());
    }

    /// Equivalence class "cutoff above Nyquist", missing from the test
    /// above: both first-order constructors accept a cutoff of exactly
    /// `fs / 2` and reject the next representable cutoff above it, and any
    /// cutoff well above it, with `InvalidConfig`.
    /// Verifies: REQ-CTL-005, REQ-CTL-023
    #[test]
    fn test_first_order_cutoff_above_nyquist_rejected() {
        for fs in [1000.0_f32, 48_000.0, 1.0] {
            let nyquist = fs * 0.5;
            assert!(
                LowPassFilter::from_cutoff(nyquist, fs).is_ok(),
                "lpf at nyquist {fs}"
            );
            assert!(
                HighPassFilter::from_cutoff(nyquist, fs).is_ok(),
                "hpf at nyquist {fs}"
            );
            for fc in [nyquist.next_up(), fs, fs * 10.0, f32::MAX] {
                assert_eq!(
                    LowPassFilter::from_cutoff(fc, fs),
                    Err(ControlError::InvalidConfig),
                    "lpf {fc} {fs}"
                );
                assert_eq!(
                    HighPassFilter::from_cutoff(fc, fs),
                    Err(ControlError::InvalidConfig),
                    "hpf {fc} {fs}"
                );
            }
        }
    }

    /// Hand-derived notch coefficients, f0 = 60 Hz, BW = 5 Hz, fs = 1 kHz:
    /// omega = 0.12 pi, cos = 0.9297765; alpha = sin(0.005 pi) = 0.0157073;
    /// a0 = 1.0157073; b0 = b2 = 0.9845356; b1 = a1 = -1.8307961;
    /// a2 = 0.9690712.
    /// Verifies: REQ-CTL-022, REQ-CTL-023
    #[test]
    fn test_notch_coefficients_hand_derived() {
        let c = NotchFilter::from_frequency(60.0, 5.0, 1000.0)
            .unwrap()
            .coefficients;
        assert_coeff(c.b0, 0.984_535_6);
        assert_coeff(c.b1, -1.830_796_1);
        assert_coeff(c.b2, 0.984_535_6);
        assert_coeff(c.a1, -1.830_796_1);
        assert_coeff(c.a2, 0.969_071_2);
    }

    /// Notch and bandwidth both at Nyquist (500 Hz at 1 kHz): cos(pi) = -1,
    /// alpha = sin(pi/2) = 1, a0 = 2, so b0 = b2 = 0.5, b1 = a1 = 1, a2 = 0.
    /// Verifies: REQ-CTL-023
    #[test]
    fn test_notch_coefficients_at_nyquist() {
        let c = NotchFilter::from_frequency(500.0, 500.0, 1000.0)
            .unwrap()
            .coefficients;
        assert_coeff(c.b0, 0.5);
        assert_coeff(c.b1, 1.0);
        assert_coeff(c.b2, 0.5);
        assert_coeff(c.a1, 1.0);
        assert!(c.a2.abs() < 1e-6);
    }

    /// Invalid notch configurations are rejected: notch or bandwidth <= 0,
    /// above Nyquist or non-finite, sample rate invalid, and a bandwidth so
    /// small that alpha rounds to zero (poles on the unit circle).
    /// Verifies: REQ-CTL-023, REQ-CTL-005
    #[test]
    fn test_notch_invalid_config_rejected() {
        let bad = [
            (0.0, 5.0, 1000.0),
            (-60.0, 5.0, 1000.0),
            (500.1, 5.0, 1000.0),
            (f32::NAN, 5.0, 1000.0),
            (f32::INFINITY, 5.0, 1000.0),
            (60.0, 0.0, 1000.0),
            (60.0, -5.0, 1000.0),
            (60.0, 500.1, 1000.0),
            (60.0, f32::NAN, 1000.0),
            (60.0, 5.0, 0.0),
            (60.0, 5.0, f32::NAN),
            (60.0, 5.0, f32::INFINITY),
            (60.0, 1.0e-45, 1000.0),
        ];
        for (f0, bw, fs) in bad {
            assert_eq!(
                NotchFilter::from_frequency(f0, bw, fs),
                Err(ControlError::InvalidConfig),
                "{f0} {bw} {fs}"
            );
        }
    }

    /// Low-pass step response follows y[k] = 1 - (1 - alpha)^k.
    /// Verifies: REQ-CTL-020
    #[test]
    fn test_lowpass_step_response_matches_closed_form() {
        let mut lpf = LowPassFilter::from_cutoff(10.0, 1000.0).unwrap();
        let alpha = lpf.alpha;
        let mut remaining = 1.0_f32;
        for _ in 0..200 {
            let (n, y) = lpf.update(1.0, 0.001).unwrap();
            remaining *= 1.0 - alpha;
            assert!((y - (1.0 - remaining)).abs() < 1e-4, "y = {y}");
            lpf = n;
        }
    }

    /// High-pass: the first sample of a unit step passes as alpha, then the
    /// output decays geometrically by alpha per sample.
    /// Verifies: REQ-CTL-021
    #[test]
    fn test_highpass_step_response_matches_closed_form() {
        let hpf = HighPassFilter::from_cutoff(10.0, 1000.0).unwrap();
        let alpha = hpf.alpha;
        let (hpf, y0) = hpf.update(1.0, 0.001).unwrap();
        assert_close(y0, alpha);
        let (_, y1) = hpf.update(1.0, 0.001).unwrap();
        assert_close(y1, alpha * alpha);
    }

    /// Settled peak amplitude of a unit sine at `freq_hz` through `notch`.
    fn settled_peak(mut notch: NotchFilter, freq_hz: f32, fs: f32) -> f32 {
        let dt = 1.0 / fs;
        let mut t = 0.0_f32;
        let mut peak = 0.0_f32;
        for k in 0..3000 {
            let x = ComplexField::sin(2.0 * core::f32::consts::PI * freq_hz * t);
            let (n, y) = notch.update(x, dt).unwrap();
            notch = n;
            if k >= 2000 {
                peak = peak.max(y.abs());
            }
            t += dt;
        }
        peak
    }

    /// The notch strongly attenuates its centre frequency while passing a
    /// frequency well outside the notch band with near-unity gain.
    /// Verifies: REQ-CTL-022
    #[test]
    fn test_notch_attenuates_centre_passes_others() {
        let n = NotchFilter::from_frequency(60.0, 5.0, 1000.0).unwrap();
        assert!(settled_peak(n, 60.0, 1000.0) < 0.05);
        assert!(settled_peak(n, 200.0, 1000.0) > 0.95);
    }
}
