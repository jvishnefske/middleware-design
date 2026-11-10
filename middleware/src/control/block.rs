//! Control block trait definition.
//!
//! This module defines the core trait for immutable control blocks and the
//! error type every block's `update` returns.
//!
//! Every block in [`crate::control`] validates its arguments in the same
//! way: a non-finite input is [`ControlError::InvalidInput`], a time step
//! that is zero, negative, NaN or infinite is [`ControlError::InvalidDelta`],
//! a configuration that cannot be evaluated without panicking or silently
//! misbehaving (for example `min > max`, or a NaN limit) is
//! [`ControlError::InvalidConfig`], and a computation that overflows to a
//! non-finite value is [`ControlError::NumericalError`].
//!
//! # Traceability
//! - REQ-CTL-001: Control blocks implement update(self, input, dt) -> Result<(Self, Output), Error>
//! - REQ-CTL-002: Control blocks implement reset(self) -> Self
//! - REQ-CTL-003: Control block state is immutable (updates return new instances)
//! - REQ-CTL-004: Non-finite inputs, non-positive or non-finite time steps and
//!   non-finite results are reported as errors

/// Errors that can occur during control block operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlError {
    /// Invalid input value (e.g., NaN, Inf).
    InvalidInput,
    /// Invalid time delta (e.g., negative, zero, NaN).
    InvalidDelta,
    /// Numerical computation error: an intermediate or final value
    /// overflowed to infinity or became NaN.
    NumericalError,
    /// The block's configuration is invalid (e.g. a lower limit above the
    /// upper limit, a NaN limit, or a negative anti-windup bound), so the
    /// update cannot be evaluated safely.
    InvalidConfig,
}

/// Trait for immutable control blocks.
///
/// Control blocks process inputs to produce outputs while maintaining internal state.
/// All operations are immutable - updates return new instances with modified state.
///
/// # Type Parameters
/// - `Input`: The input type
/// - `Output`: The output type
///
/// # Examples
///
/// Simple gain block:
/// ```
/// use middleware::control::{ControlBlock, ControlError};
///
/// struct Gain {
///     k: f32,
/// }
///
/// impl ControlBlock for Gain {
///     type Input = f32;
///     type Output = f32;
///
///     fn update(self, input: Self::Input, _dt: f32) -> Result<(Self, Self::Output), ControlError> {
///         if !input.is_finite() {
///             return Err(ControlError::InvalidInput);
///         }
///         let output = self.k * input;
///         Ok((self, output))
///     }
///
///     fn reset(self) -> Self {
///         self
///     }
/// }
///
/// let gain = Gain { k: 2.5 };
/// let (gain, output) = gain.update(4.0, 0.01).unwrap();
/// assert_eq!(output, 10.0);
/// assert!(matches!(gain.update(f32::NAN, 0.01), Err(ControlError::InvalidInput)));
/// ```
pub trait ControlBlock {
    /// The input type for this control block.
    type Input;

    /// The output type for this control block.
    type Output;

    /// Updates the control block with a new input.
    ///
    /// This method is immutable - it consumes self and returns a new instance
    /// with updated state.
    ///
    /// # Parameters
    /// - `input`: The input value
    /// - `dt`: The time delta in seconds since the last update
    ///
    /// # Returns
    /// `Ok((new_block, output))` where `new_block` is the updated control block
    /// and `output` is the computed output value.
    /// `Err(error)` if the operation failed.
    ///
    /// # Errors
    /// - `ControlError::InvalidInput`: Input value is NaN or Inf
    /// - `ControlError::InvalidDelta`: dt is negative, zero, NaN, or Inf
    /// - `ControlError::NumericalError`: Computation resulted in invalid value
    /// - `ControlError::InvalidConfig`: The block's configuration cannot be
    ///   evaluated (blocks with limits check them on every update)
    ///
    /// On error no new block is returned; the caller's copy of the block is
    /// unchanged.
    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError>
    where
        Self: Sized;

    /// Resets the control block to its initial state.
    ///
    /// This method is immutable - it returns a new instance with reset state.
    ///
    /// # Returns
    /// A new control block instance with state reset to initial values
    fn reset(self) -> Self
    where
        Self: Sized;
}

/// Fault-injection vectors shared by the control block test modules.
#[cfg(test)]
pub(crate) mod fault_vectors {
    /// Non-finite input values: every block must reject each of these with
    /// `ControlError::InvalidInput`.
    pub(crate) const NON_FINITE: [f32; 3] = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];

    /// Invalid time steps (zero, signed zero, negative, NaN, both
    /// infinities): every block must reject each of these with
    /// `ControlError::InvalidDelta`.
    pub(crate) const BAD_DT: [f32; 7] = [
        0.0,
        -0.0,
        -0.01,
        -f32::MIN_POSITIVE,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];

    /// Smallest positive normal time step: it must be accepted (boundary of
    /// the valid `dt > 0` class).
    pub(crate) const TINY_DT: f32 = f32::MIN_POSITIVE;
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::float_cmp))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{
        DiscreteStateSpace, HighPassFilter, LowPassFilter, NotchFilter, PidController, PidGains,
        PidLimits, RateLimiter, Saturator,
    };
    use crate::test_support::assert_close;
    use nalgebra::{SMatrix, SVector};

    /// Compile-time check: `B` implements `ControlBlock` with
    /// `update(self, Input, f32) -> Result<(Self, Output), ControlError>` and
    /// `reset(self) -> Self`. Binding the trait methods to these exact
    /// function-pointer types fails to compile if either signature drifts.
    type UpdateResult<B> = Result<(B, <B as ControlBlock>::Output), ControlError>;

    fn assert_block_signature<B: ControlBlock>() {
        let _update: fn(B, B::Input, f32) -> UpdateResult<B> = B::update;
        let _reset: fn(B) -> B = B::reset;
    }

    fn sample_ss() -> DiscreteStateSpace<1, 1, 1> {
        DiscreteStateSpace::new(
            SMatrix::<f32, 1, 1>::identity(),
            SMatrix::<f32, 1, 1>::identity(),
            SMatrix::<f32, 1, 1>::identity(),
            SMatrix::<f32, 1, 1>::zeros(),
            SVector::<f32, 1>::zeros(),
        )
    }

    /// Every runtime control block satisfies the trait signatures required
    /// by REQ-CTL-001/002; this is compile-time evidence, the test body only
    /// instantiates the generic checks.
    /// Verifies: REQ-CTL-001, REQ-CTL-002
    #[test]
    fn test_all_blocks_implement_control_block_signature() {
        assert_block_signature::<PidController>();
        assert_block_signature::<LowPassFilter>();
        assert_block_signature::<HighPassFilter>();
        assert_block_signature::<NotchFilter>();
        assert_block_signature::<DiscreteStateSpace<2, 1, 1>>();
        assert_block_signature::<RateLimiter>();
        assert_block_signature::<Saturator>();
    }

    /// Updating a block returns a new instance carrying the new state, while
    /// the caller's copy taken before the update is unchanged, both after a
    /// successful update and after a rejected one.
    /// Verifies: REQ-CTL-003
    #[test]
    fn test_updates_return_new_instances_and_leave_original_unchanged() {
        let pid = PidController::new(
            PidGains::new(1.0, 1.0, 0.0),
            PidLimits::new(-10.0, 10.0, 10.0),
        );
        let before = pid;
        let (after, _) = pid.update((1.0, 0.0), 0.1).unwrap();
        assert_eq!(pid, before);
        assert_ne!(after, before);
        assert!(pid.update((f32::NAN, 0.0), 0.1).is_err());
        assert_eq!(pid, before);

        let lpf = LowPassFilter::new(0.5);
        let (after, _) = lpf.update(1.0, 0.1).unwrap();
        assert_eq!(lpf.state(), 0.0);
        assert_eq!(after.state(), 0.5);

        let hpf = HighPassFilter::new(0.5);
        let (after, _) = hpf.update(1.0, 0.1).unwrap();
        assert_eq!(hpf, HighPassFilter::new(0.5));
        assert_ne!(after, hpf);

        let notch = NotchFilter::from_frequency(50.0, 5.0, 1000.0).unwrap();
        let before = notch;
        let (after, _) = notch.update(1.0, 0.001).unwrap();
        assert_eq!(notch, before);
        assert_ne!(after, before);

        let ss = sample_ss();
        let (after, _) = ss.update(SVector::<f32, 1>::new(1.0), 0.1).unwrap();
        assert_eq!(ss.state().x, 0.0);
        assert_eq!(after.state().x, 1.0);

        let limiter = RateLimiter::new(1.0, -1.0, 0.0);
        let (after, _) = limiter.update(5.0, 1.0).unwrap();
        assert_eq!(limiter.prev_output(), 0.0);
        assert_eq!(after.prev_output(), 1.0);

        // A saturator has no state: the returned instance equals the input.
        let sat = Saturator::new(-1.0, 1.0);
        let (after, _) = sat.update(5.0, 1.0).unwrap();
        assert_eq!(after, sat);
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Gain {
        k: f32,
    }

    impl ControlBlock for Gain {
        type Input = f32;
        type Output = f32;

        fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
            if !input.is_finite() {
                return Err(ControlError::InvalidInput);
            }
            if dt <= 0.0 || !dt.is_finite() {
                return Err(ControlError::InvalidDelta);
            }
            Ok((self, self.k * input))
        }

        fn reset(self) -> Self {
            self
        }
    }

    #[test]
    fn test_control_block_update() {
        let gain = Gain { k: 2.5 };
        let (gain, output) = gain.update(4.0, 0.01).unwrap();
        assert_close(output, 10.0);
        assert_eq!(gain.k, 2.5);
    }

    #[test]
    fn test_control_block_invalid_input() {
        let gain = Gain { k: 2.5 };
        assert_eq!(
            gain.update(f32::NAN, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
        assert_eq!(
            gain.update(f32::INFINITY, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
    }

    #[test]
    fn test_control_block_invalid_delta() {
        let gain = Gain { k: 2.5 };
        assert_eq!(
            gain.update(1.0, 0.0).unwrap_err(),
            ControlError::InvalidDelta
        );
        assert_eq!(
            gain.update(1.0, -0.01).unwrap_err(),
            ControlError::InvalidDelta
        );
        assert_eq!(
            gain.update(1.0, f32::NAN).unwrap_err(),
            ControlError::InvalidDelta
        );
    }

    #[test]
    fn test_control_block_reset() {
        let gain = Gain { k: 2.5 };
        let gain_reset = gain.reset();
        assert_eq!(gain.k, gain_reset.k);
    }

    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Integrator {
        state: f32,
    }

    impl ControlBlock for Integrator {
        type Input = f32;
        type Output = f32;

        fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
            if !input.is_finite() {
                return Err(ControlError::InvalidInput);
            }
            if dt <= 0.0 || !dt.is_finite() {
                return Err(ControlError::InvalidDelta);
            }

            let new_state = self.state + input * dt;
            if !new_state.is_finite() {
                return Err(ControlError::NumericalError);
            }

            Ok((Integrator { state: new_state }, new_state))
        }

        fn reset(self) -> Self {
            Integrator { state: 0.0 }
        }
    }

    #[test]
    fn test_integrator_block() {
        let integrator = Integrator { state: 0.0 };
        let (integrator, output) = integrator.update(10.0, 0.1).unwrap();
        assert_close(output, 1.0);

        let (integrator, output) = integrator.update(10.0, 0.1).unwrap();
        assert_close(output, 2.0);

        let integrator = integrator.reset();
        assert_eq!(integrator.state, 0.0);
        let (_, output) = integrator.update(10.0, 0.1).unwrap();
        assert_close(output, 1.0);
    }

    /// The reference integrator (a model of the error contract every block
    /// follows) rejects bad inputs and time steps, and reports overflow.
    #[test]
    fn test_integrator_block_errors() {
        let integrator = Integrator { state: f32::MAX };
        assert_eq!(
            integrator.update(f32::NAN, 0.1),
            Err(ControlError::InvalidInput)
        );
        assert_eq!(integrator.update(1.0, 0.0), Err(ControlError::InvalidDelta));
        assert_eq!(
            integrator.update(1.0, f32::INFINITY),
            Err(ControlError::InvalidDelta)
        );
        assert_eq!(
            integrator.update(f32::MAX, 1.0),
            Err(ControlError::NumericalError)
        );
    }
}
