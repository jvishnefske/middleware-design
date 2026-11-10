#![no_std]
//! # Middleware Framework
//!
//! Model-based development framework for embedded control systems.
//!
//! This library provides type-safe abstractions for building DO-178C DAL-C
//! compliant control systems on ARM Cortex-M targets.
//!
//! # Traceability
//!
//! - REQ-PLT-001: no_std compatibility
//! - REQ-PLT-002: No heap allocation
//! - REQ-PLT-003: Fixed-size stack allocation

pub mod control;
pub mod error;
pub mod port;
pub mod state_machine;
pub mod telemetry;
pub mod types;
pub mod component;

// Re-export key types
pub use control::{
    ControlBlock, ControlError as ControlBlockError, DiscreteStateSpace, HighPassFilter,
    LowPassFilter, NotchFilter, PidController, PidControllerBuilder, PidGains, PidLimits,
    PidState, RateLimiter, Saturator,
};
pub use error::{
    FrameworkError, InitError, LifecycleError, PortError, StateMachineError, SerializeError,
    ControlError, InvokeError, CycleError, RecvError, SendError, SerializationError
};
pub use state_machine::{
    ControlGuard, Event, FsmError, State, StateMachine, ThresholdGuard,
    TimedGuard, Transition, TransitionResult,
};
pub use telemetry::{ControlTelemetry, PidTelemetry, StateMachineTelemetry, TelemetryBuffer, FrameHeader, MessageType};
pub use types::{
    unit::{Seconds, Milliseconds, Radians, Degrees, MetersPerSecond, RadiansPerSecond, Dimensionless},
    primitive::{Quantity, F32, F64, I32, U32, I16, U16, I8, U8, I64, U64, Bool},
    bounded::{BoundedVec, BoundedString},
};
pub use component::{Component, ComponentId, ComponentConfig, ComponentState};
pub use port::{PortMessage, PortDirection};
