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
//!
//! The crate is `#![no_std]` and declares no `extern crate alloc`; every
//! container is sized by a const generic. The evidence (a cross build for
//! `thumbv6m-none-eabi`, a heap/`std` source scan and `size_of` checks) is
//! in `checks/nostd-gen/tests/platform.rs`.

pub mod component;
pub mod control;
pub mod error;
pub mod port;
pub mod state_machine;
pub mod telemetry;
pub mod types;

#[cfg(test)]
mod test_support;

// Re-export key types
pub use component::{Component, ComponentConfig, ComponentId, ComponentState};
pub use control::{
    ControlBlock, ControlError as ControlBlockError, DiscreteStateSpace, HighPassFilter,
    LowPassFilter, NotchFilter, PidController, PidControllerBuilder, PidGains, PidLimits, PidState,
    RateLimiter, Saturator,
};
pub use error::{
    ControlError, CycleError, FrameworkError, InitError, InvokeError, LifecycleError, PortError,
    RecvError, SendError, SerializationError, SerializeError, StateMachineError,
};
pub use port::{PortDirection, PortMessage};
pub use state_machine::{
    ControlGuard, Event, FsmError, State, StateMachine, ThresholdGuard, TimedGuard, Transition,
    TransitionResult,
};
pub use telemetry::{
    ControlTelemetry, FrameHeader, MessageType, PidTelemetry, StateMachineTelemetry,
    TelemetryBuffer,
};
pub use types::{
    bounded::{BoundedString, BoundedVec},
    primitive::{Bool, Quantity, F32, F64, I16, I32, I64, I8, U16, U32, U64, U8},
    unit::{
        Degrees, Dimensionless, MetersPerSecond, Milliseconds, Radians, RadiansPerSecond, Seconds,
    },
};
