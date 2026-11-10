//! Rate limiters and saturators for control systems.
//!
//! Provides components to constrain signal rates and amplitudes.
//!
//! # Traceability
//! - REQ-CTL-040: RateLimiter with configurable rate limits
//! - REQ-CTL-041: Saturator with min/max bounds

use super::{ControlBlock, ControlError};

/// Rate limiter with asymmetric limits.
///
/// Constrains the rate of change of a signal, with separate limits for
/// rising and falling edges.
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, RateLimiter};
///
/// let limiter = RateLimiter::new(10.0, -5.0, 0.0);
///
/// // Signal can increase at most 10.0 per second
/// // Signal can decrease at most 5.0 per second
/// let (limiter, output) = limiter.update(100.0, 0.1).unwrap();
/// assert_eq!(output, 1.0); // Limited to 0.0 + 10.0 * 0.1
///
/// let (limiter, output) = limiter.update(-100.0, 0.1).unwrap();
/// assert_eq!(output, 0.5); // Limited to 1.0 - 5.0 * 0.1
/// assert_eq!(limiter.prev_output(), 0.5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateLimiter {
    /// Maximum rate of increase (positive value in units/second).
    pub rate_limit_up: f32,
    /// Maximum rate of decrease (negative value in units/second).
    pub rate_limit_down: f32,
    /// Previous output value.
    prev_output: f32,
}

impl RateLimiter {
    /// Creates a new rate limiter.
    ///
    /// # Parameters
    /// - `rate_limit_up`: Maximum rate of increase (units/second, positive)
    /// - `rate_limit_down`: Maximum rate of decrease (units/second, negative)
    /// - `initial_output`: Initial output value
    ///
    /// # Returns
    /// A new rate limiter
    pub const fn new(rate_limit_up: f32, rate_limit_down: f32, initial_output: f32) -> Self {
        Self {
            rate_limit_up,
            rate_limit_down,
            prev_output: initial_output,
        }
    }

    /// Returns the previous output value.
    pub fn prev_output(&self) -> f32 {
        self.prev_output
    }
}

impl ControlBlock for RateLimiter {
    type Input = f32;
    type Output = f32;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        if !input.is_finite() {
            return Err(ControlError::InvalidInput);
        }
        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        // Compute maximum allowed change
        let max_increase = self.rate_limit_up * dt;
        let max_decrease = self.rate_limit_down * dt;

        // Compute desired change
        let desired_change = input - self.prev_output;

        // Clamp change to rate limits
        let limited_change = if desired_change > 0.0 {
            desired_change.min(max_increase)
        } else {
            desired_change.max(max_decrease)
        };

        // Compute output
        let output = self.prev_output + limited_change;

        if !output.is_finite() {
            return Err(ControlError::NumericalError);
        }

        Ok((
            Self {
                rate_limit_up: self.rate_limit_up,
                rate_limit_down: self.rate_limit_down,
                prev_output: output,
            },
            output,
        ))
    }

    fn reset(self) -> Self {
        Self {
            rate_limit_up: self.rate_limit_up,
            rate_limit_down: self.rate_limit_down,
            prev_output: 0.0,
        }
    }
}

/// Signal saturator with min/max bounds.
///
/// Clamps the input signal to specified minimum and maximum values.
///
/// # Examples
///
/// ```
/// use middleware::control::{ControlBlock, Saturator};
///
/// let saturator = Saturator::new(-10.0, 10.0);
///
/// let (_, output) = saturator.update(15.0, 0.01).unwrap();
/// assert_eq!(output, 10.0); // Clamped to maximum
///
/// let (_, output) = saturator.update(-15.0, 0.01).unwrap();
/// assert_eq!(output, -10.0); // Clamped to minimum
///
/// let (_, output) = saturator.update(3.0, 0.01).unwrap();
/// assert_eq!(output, 3.0); // In range: passed through
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Saturator {
    /// Minimum allowed value.
    pub min: f32,
    /// Maximum allowed value.
    pub max: f32,
}

impl Saturator {
    /// Creates a new saturator.
    ///
    /// # Parameters
    /// - `min`: Minimum allowed value
    /// - `max`: Maximum allowed value
    ///
    /// # Returns
    /// A new saturator
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

impl ControlBlock for Saturator {
    type Input = f32;
    type Output = f32;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        if !input.is_finite() {
            return Err(ControlError::InvalidInput);
        }
        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        let output = input.clamp(self.min, self.max);

        Ok((self, output))
    }

    fn reset(self) -> Self {
        self
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::float_cmp))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    #[test]
    fn test_rate_limiter_new() {
        let limiter = RateLimiter::new(10.0, -5.0, 0.0);
        assert_eq!(limiter.rate_limit_up, 10.0);
        assert_eq!(limiter.rate_limit_down, -5.0);
        assert_eq!(limiter.prev_output(), 0.0);
    }

    #[test]
    fn test_rate_limiter_increase() {
        let limiter = RateLimiter::new(10.0, -5.0, 0.0);

        // Try to increase by 100.0, but limited to 10.0 * 0.1 = 1.0
        let (limiter, output) = limiter.update(100.0, 0.1).unwrap();
        assert_close(output, 1.0);

        // Try to increase by another 100.0, but limited to 1.0 + 1.0 = 2.0
        let (_, output) = limiter.update(100.0, 0.1).unwrap();
        assert_close(output, 2.0);
    }

    #[test]
    fn test_rate_limiter_decrease() {
        let limiter = RateLimiter::new(10.0, -5.0, 10.0);

        // Try to decrease to 0.0, but limited to -5.0 * 0.1 = -0.5
        let (limiter, output) = limiter.update(0.0, 0.1).unwrap();
        assert_close(output, 9.5);

        // Try again
        let (_, output) = limiter.update(0.0, 0.1).unwrap();
        assert_close(output, 9.0);
    }

    #[test]
    fn test_rate_limiter_within_limits() {
        let limiter = RateLimiter::new(10.0, -5.0, 5.0);

        // Small change that's within rate limits
        let (limiter, output) = limiter.update(5.5, 0.1).unwrap();
        assert_close(output, 5.5);

        let (_, output) = limiter.update(5.0, 0.1).unwrap();
        assert_close(output, 5.0);
    }

    #[test]
    fn test_rate_limiter_symmetric() {
        let limiter = RateLimiter::new(10.0, -10.0, 0.0);

        let (limiter, output) = limiter.update(100.0, 0.1).unwrap();
        assert_close(output, 1.0);

        let (_, output) = limiter.update(-100.0, 0.1).unwrap();
        assert_close(output, 0.0); // 1.0 + (-10.0 * 0.1)
    }

    #[test]
    fn test_rate_limiter_reset() {
        let limiter = RateLimiter::new(10.0, -5.0, 5.0);
        let (limiter, _) = limiter.update(100.0, 0.1).unwrap();

        assert_close(limiter.prev_output(), 6.0); // 5.0 + 10.0 * 0.1

        let limiter = limiter.reset();
        assert_eq!(limiter.prev_output(), 0.0);
    }

    #[test]
    fn test_rate_limiter_invalid_input() {
        let limiter = RateLimiter::new(10.0, -5.0, 0.0);

        assert_eq!(
            limiter.update(f32::NAN, 0.1).unwrap_err(),
            ControlError::InvalidInput
        );
        assert_eq!(
            limiter.update(f32::INFINITY, 0.1).unwrap_err(),
            ControlError::InvalidInput
        );
    }

    #[test]
    fn test_saturator_new() {
        let saturator = Saturator::new(-10.0, 10.0);
        assert_eq!(saturator.min, -10.0);
        assert_eq!(saturator.max, 10.0);
    }

    #[test]
    fn test_saturator_clamp_max() {
        let saturator = Saturator::new(-10.0, 10.0);

        let (_, output) = saturator.update(15.0, 0.01).unwrap();
        assert_eq!(output, 10.0);

        let (_, output) = saturator.update(100.0, 0.01).unwrap();
        assert_eq!(output, 10.0);
    }

    #[test]
    fn test_saturator_clamp_min() {
        let saturator = Saturator::new(-10.0, 10.0);

        let (_, output) = saturator.update(-15.0, 0.01).unwrap();
        assert_eq!(output, -10.0);

        let (_, output) = saturator.update(-100.0, 0.01).unwrap();
        assert_eq!(output, -10.0);
    }

    #[test]
    fn test_saturator_within_bounds() {
        let saturator = Saturator::new(-10.0, 10.0);

        let (_, output) = saturator.update(0.0, 0.01).unwrap();
        assert_eq!(output, 0.0);

        let (_, output) = saturator.update(5.0, 0.01).unwrap();
        assert_eq!(output, 5.0);

        let (_, output) = saturator.update(-5.0, 0.01).unwrap();
        assert_eq!(output, -5.0);
    }

    #[test]
    fn test_saturator_at_bounds() {
        let saturator = Saturator::new(-10.0, 10.0);

        let (_, output) = saturator.update(10.0, 0.01).unwrap();
        assert_eq!(output, 10.0);

        let (_, output) = saturator.update(-10.0, 0.01).unwrap();
        assert_eq!(output, -10.0);
    }

    #[test]
    fn test_saturator_reset() {
        let saturator = Saturator::new(-10.0, 10.0);
        let saturator = saturator.reset();

        assert_eq!(saturator.min, -10.0);
        assert_eq!(saturator.max, 10.0);
    }

    #[test]
    fn test_saturator_invalid_input() {
        let saturator = Saturator::new(-10.0, 10.0);

        assert_eq!(
            saturator.update(f32::NAN, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
        assert_eq!(
            saturator.update(f32::INFINITY, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
    }

    #[test]
    fn test_saturator_asymmetric() {
        let saturator = Saturator::new(0.0, 100.0);

        let (_, output) = saturator.update(-10.0, 0.01).unwrap();
        assert_eq!(output, 0.0);

        let (_, output) = saturator.update(50.0, 0.01).unwrap();
        assert_eq!(output, 50.0);

        let (_, output) = saturator.update(150.0, 0.01).unwrap();
        assert_eq!(output, 100.0);
    }
}
