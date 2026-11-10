//! Control block primitives for embedded control systems.
//!
//! This module provides immutable control blocks including PID controllers,
//! digital filters, state-space models, and limiters.
//!
//! # Traceability
//! - REQ-CTL-001: ControlBlock trait with update method
//! - REQ-CTL-002: ControlBlock reset method
//! - REQ-CTL-003: Immutable control block state
//! - REQ-CTL-004: Non-finite inputs, non-positive or non-finite time steps
//!   and non-finite results are reported as errors (see [`ControlError`];
//!   invalid limits and filter parameters are reported as
//!   [`ControlError::InvalidConfig`])
//! - REQ-CTL-010 through REQ-CTL-014: PID controller
//! - REQ-CTL-020 through REQ-CTL-023: Digital filters
//! - REQ-CTL-030 through REQ-CTL-032: State-space models
//! - REQ-CTL-040, REQ-CTL-041: Limiters

pub mod block;
pub mod filters;
pub mod limiters;
pub mod pid;
pub mod state_space;

pub use block::{ControlBlock, ControlError};
pub use filters::{HighPassFilter, LowPassFilter, NotchFilter};
pub use limiters::{RateLimiter, Saturator};
pub use pid::{PidController, PidControllerBuilder, PidGains, PidLimits, PidState};
pub use state_space::DiscreteStateSpace;
