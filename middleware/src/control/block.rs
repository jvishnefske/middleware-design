//! Control block trait definition.
//!
//! This module defines the core trait for immutable control blocks.
//!
//! # Traceability
//! - REQ-CTL-001: Control blocks implement update(self, input, dt) -> Result<(Self, Output), Error>
//! - REQ-CTL-002: Control blocks implement reset(self) -> Self
//! - REQ-CTL-003: Control block state is immutable (updates return new instances)

/// Errors that can occur during control block operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlError {
    /// Invalid input value (e.g., NaN, Inf).
    InvalidInput,
    /// Invalid time delta (e.g., negative, zero, NaN).
    InvalidDelta,
    /// Numerical computation error.
    NumericalError,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(output, 10.0);
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
        assert_eq!(output, 1.0);

        let (integrator, output) = integrator.update(10.0, 0.1).unwrap();
        assert_eq!(output, 2.0);

        let integrator = integrator.reset();
        let (_, output) = integrator.update(10.0, 0.1).unwrap();
        assert_eq!(output, 1.0);
    }
}
