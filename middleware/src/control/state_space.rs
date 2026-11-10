//! Discrete-time state-space models.
//!
//! Provides state-space representations for linear time-invariant systems.
//!
//! # Traceability
//! - REQ-CTL-030: Discrete-time state-space model
//! - REQ-CTL-031: State-space supports const generic dimensions (N states, M inputs, P outputs)
//! - REQ-CTL-032: State-space uses nalgebra for matrix operations

use super::{ControlBlock, ControlError};
use nalgebra::{SMatrix, SVector};

/// Discrete-time state-space model.
///
/// Represents a linear time-invariant system in state-space form:
/// ```text
/// x[k+1] = A * x[k] + B * u[k]
/// y[k]   = C * x[k] + D * u[k]
/// ```
///
/// where:
/// - x is the state vector (N x 1)
/// - u is the input vector (M x 1)
/// - y is the output vector (P x 1)
/// - A is the state transition matrix (N x N)
/// - B is the input matrix (N x M)
/// - C is the output matrix (P x N)
/// - D is the feedthrough matrix (P x M)
///
/// # Type Parameters
/// - `N`: Number of states
/// - `M`: Number of inputs
/// - `P`: Number of outputs
///
/// # Examples
///
/// Simple integrator (1 state, 1 input, 1 output):
/// ```
/// use middleware::control::{ControlBlock, DiscreteStateSpace};
/// use nalgebra::{SMatrix, SVector};
///
/// // x[k+1] = x[k] + u[k] * dt
/// // y[k] = x[k]
/// let dt = 0.01;
/// let a = SMatrix::<f32, 1, 1>::identity();
/// let b = SMatrix::<f32, 1, 1>::identity() * dt;
/// let c = SMatrix::<f32, 1, 1>::identity();
/// let d = SMatrix::<f32, 1, 1>::zeros();
/// let x0 = SVector::<f32, 1>::zeros();
///
/// let ss = DiscreteStateSpace::new(a, b, c, d, x0);
///
/// // `update` returns y[k] for the current state, then propagates x.
/// let u = SVector::<f32, 1>::new(2.0);
/// let (ss, y) = ss.update(u, dt).unwrap();
/// assert_eq!(y[0], 0.0);
/// assert!((ss.state()[0] - 0.02).abs() < 1e-6);
///
/// let (_ss, y) = ss.update(u, dt).unwrap();
/// assert!((y[0] - 0.02).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscreteStateSpace<const N: usize, const M: usize, const P: usize> {
    /// State transition matrix (N x N).
    a: SMatrix<f32, N, N>,
    /// Input matrix (N x M).
    b: SMatrix<f32, N, M>,
    /// Output matrix (P x N).
    c: SMatrix<f32, P, N>,
    /// Feedthrough matrix (P x M).
    d: SMatrix<f32, P, M>,
    /// Current state vector (N x 1).
    state: SVector<f32, N>,
}

impl<const N: usize, const M: usize, const P: usize> DiscreteStateSpace<N, M, P> {
    /// Creates a new discrete-time state-space model.
    ///
    /// # Parameters
    /// - `a`: State transition matrix (N x N)
    /// - `b`: Input matrix (N x M)
    /// - `c`: Output matrix (P x N)
    /// - `d`: Feedthrough matrix (P x M)
    /// - `initial_state`: Initial state vector (N x 1)
    ///
    /// # Returns
    /// A new state-space model
    pub fn new(
        a: SMatrix<f32, N, N>,
        b: SMatrix<f32, N, M>,
        c: SMatrix<f32, P, N>,
        d: SMatrix<f32, P, M>,
        initial_state: SVector<f32, N>,
    ) -> Self {
        Self {
            a,
            b,
            c,
            d,
            state: initial_state,
        }
    }

    /// Returns the current state vector.
    pub fn state(&self) -> SVector<f32, N> {
        self.state
    }

    /// Returns the A matrix.
    pub fn a(&self) -> SMatrix<f32, N, N> {
        self.a
    }

    /// Returns the B matrix.
    pub fn b(&self) -> SMatrix<f32, N, M> {
        self.b
    }

    /// Returns the C matrix.
    pub fn c(&self) -> SMatrix<f32, P, N> {
        self.c
    }

    /// Returns the D matrix.
    pub fn d(&self) -> SMatrix<f32, P, M> {
        self.d
    }

    /// Propagates the state forward one time step.
    ///
    /// Computes: x[k+1] = A * x[k] + B * u[k]
    ///
    /// # Parameters
    /// - `input`: Input vector (M x 1)
    ///
    /// # Returns
    /// A new state-space model with updated state
    fn propagate(self, input: SVector<f32, M>) -> Result<Self, ControlError> {
        let new_state = self.a * self.state + self.b * input;

        // Check for numerical errors
        if new_state.iter().any(|v| !v.is_finite()) {
            return Err(ControlError::NumericalError);
        }

        Ok(Self {
            a: self.a,
            b: self.b,
            c: self.c,
            d: self.d,
            state: new_state,
        })
    }

    /// Computes the output for the current state and input.
    ///
    /// Computes: y[k] = C * x[k] + D * u[k]
    ///
    /// # Parameters
    /// - `input`: Input vector (M x 1)
    ///
    /// # Returns
    /// Output vector (P x 1)
    fn output(&self, input: SVector<f32, M>) -> Result<SVector<f32, P>, ControlError> {
        let output = self.c * self.state + self.d * input;

        // Check for numerical errors
        if output.iter().any(|v| !v.is_finite()) {
            return Err(ControlError::NumericalError);
        }

        Ok(output)
    }
}

impl<const N: usize, const M: usize, const P: usize> ControlBlock for DiscreteStateSpace<N, M, P> {
    type Input = SVector<f32, M>;
    type Output = SVector<f32, P>;

    fn update(self, input: Self::Input, dt: f32) -> Result<(Self, Self::Output), ControlError> {
        // Validate input
        if input.iter().any(|v| !v.is_finite()) {
            return Err(ControlError::InvalidInput);
        }

        if dt <= 0.0 || !dt.is_finite() {
            return Err(ControlError::InvalidDelta);
        }

        // Compute output for current state
        let output = self.output(input)?;

        // Propagate state
        let new_model = self.propagate(input)?;

        Ok((new_model, output))
    }

    fn reset(self) -> Self {
        Self {
            a: self.a,
            b: self.b,
            c: self.c,
            d: self.d,
            state: SVector::<f32, N>::zeros(),
        }
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::assert_close;

    #[test]
    fn test_state_space_new() {
        let a = SMatrix::<f32, 1, 1>::identity();
        let b = SMatrix::<f32, 1, 1>::identity();
        let c = SMatrix::<f32, 1, 1>::identity();
        let d = SMatrix::<f32, 1, 1>::zeros();
        let x0 = SVector::<f32, 1>::zeros();

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        assert_eq!(ss.a(), a);
        assert_eq!(ss.b(), b);
        assert_eq!(ss.c(), c);
        assert_eq!(ss.d(), d);
        assert_eq!(ss.state(), x0);
    }

    #[test]
    fn test_integrator() {
        // Simple integrator: x[k+1] = x[k] + u[k] * dt
        let dt = 0.1;
        let a = SMatrix::<f32, 1, 1>::identity();
        let b = SMatrix::<f32, 1, 1>::identity() * dt;
        let c = SMatrix::<f32, 1, 1>::identity();
        let d = SMatrix::<f32, 1, 1>::zeros();
        let x0 = SVector::<f32, 1>::zeros();

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        // Input of 10.0 for one time step
        let input = SVector::<f32, 1>::from([10.0]);
        let (ss, output) = ss.update(input, dt).unwrap();

        // Output should be 0.0 (initial state)
        assert_close(output.x, 0.0);

        // State should be 1.0 (0.0 + 10.0 * 0.1)
        assert_close(ss.state().x, 1.0);

        // Next update
        let (ss, output) = ss.update(input, dt).unwrap();

        // Output should be 1.0 (previous state)
        assert_close(output.x, 1.0);

        // State should be 2.0 (1.0 + 10.0 * 0.1)
        assert_close(ss.state().x, 2.0);
    }

    #[test]
    fn test_feedthrough() {
        // System with feedthrough: y = x + u
        let a = SMatrix::<f32, 1, 1>::identity();
        let b = SMatrix::<f32, 1, 1>::zeros();
        let c = SMatrix::<f32, 1, 1>::identity();
        let d = SMatrix::<f32, 1, 1>::identity();
        let x0 = SVector::<f32, 1>::from([5.0]);

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        let input = SVector::<f32, 1>::from([3.0]);
        let (_, output) = ss.update(input, 0.01).unwrap();

        // Output should be state + input = 5.0 + 3.0 = 8.0
        assert_close(output.x, 8.0);
    }

    #[test]
    fn test_multi_state() {
        // 2-state system
        #[rustfmt::skip]
        let a = SMatrix::<f32, 2, 2>::from_row_slice(&[
            1.0, 0.1,
            0.0, 1.0,
        ]);
        let b = SMatrix::<f32, 2, 1>::from_column_slice(&[0.0, 0.1]);
        let c = SMatrix::<f32, 1, 2>::from_row_slice(&[1.0, 0.0]);
        let d = SMatrix::<f32, 1, 1>::zeros();
        let x0 = SVector::<f32, 2>::zeros();

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        let input = SVector::<f32, 1>::from([10.0]);
        let (ss, output) = ss.update(input, 0.01).unwrap();

        assert_close(output.x, 0.0); // Initial output

        // State should be updated
        let state = ss.state();
        assert!(state.iter().all(|v| v.is_finite()));
        // x1 = 0.1 * 10.0 = 1.0 after one step; x0 has not moved yet.
        assert_close(state.x, 0.0);
        assert_close(state.y, 1.0);
    }

    #[test]
    fn test_reset() {
        let a = SMatrix::<f32, 1, 1>::identity();
        let b = SMatrix::<f32, 1, 1>::identity();
        let c = SMatrix::<f32, 1, 1>::identity();
        let d = SMatrix::<f32, 1, 1>::zeros();
        let x0 = SVector::<f32, 1>::from([5.0]);

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        let input = SVector::<f32, 1>::from([10.0]);
        let (ss, _) = ss.update(input, 0.01).unwrap();

        // State should be non-zero: 5.0 + 10.0
        assert_close(ss.state().x, 15.0);

        // Reset
        let ss = ss.reset();
        assert_eq!(ss.state(), SVector::<f32, 1>::zeros());
    }

    #[test]
    fn test_invalid_input() {
        let a = SMatrix::<f32, 1, 1>::identity();
        let b = SMatrix::<f32, 1, 1>::identity();
        let c = SMatrix::<f32, 1, 1>::identity();
        let d = SMatrix::<f32, 1, 1>::zeros();
        let x0 = SVector::<f32, 1>::zeros();

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        let input = SVector::<f32, 1>::from([f32::NAN]);
        assert_eq!(
            ss.update(input, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );

        let input = SVector::<f32, 1>::from([f32::INFINITY]);
        assert_eq!(
            ss.update(input, 0.01).unwrap_err(),
            ControlError::InvalidInput
        );
    }

    #[test]
    fn test_invalid_delta() {
        let a = SMatrix::<f32, 1, 1>::identity();
        let b = SMatrix::<f32, 1, 1>::identity();
        let c = SMatrix::<f32, 1, 1>::identity();
        let d = SMatrix::<f32, 1, 1>::zeros();
        let x0 = SVector::<f32, 1>::zeros();

        let ss = DiscreteStateSpace::new(a, b, c, d, x0);

        let input = SVector::<f32, 1>::from([1.0]);

        assert_eq!(
            ss.update(input, 0.0).unwrap_err(),
            ControlError::InvalidDelta
        );
        assert_eq!(
            ss.update(input, -0.01).unwrap_err(),
            ControlError::InvalidDelta
        );
    }
}
