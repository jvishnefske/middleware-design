//! Finite state machine primitives for embedded control systems.
//!
//! This module provides a type-safe, immutable state machine implementation
//! suitable for DO-178C DAL-C certified embedded systems.
//!
//! # Traceability
//! - REQ-FSM-001: State representation
//! - REQ-FSM-002: State lifecycle actions
//! - REQ-FSM-003: Event representation
//! - REQ-FSM-010: Transition specification
//! - REQ-FSM-011: Guard conditions
//! - REQ-FSM-012: Transition actions
//! - REQ-FSM-013: Compile-time transition tables
//! - REQ-FSM-020: StateMachine container
//! - REQ-FSM-021: Event processing
//! - REQ-FSM-022: Transition results
//! - REQ-FSM-023: Immutable processing
//! - REQ-FSM-024: Row selection (first passing row fires; Remained / Ignored)
//! - REQ-FSM-025: Distinct errors for failing exit, action and entry hooks
//! - REQ-FSM-030: Threshold guards
//! - REQ-FSM-031: Timed guards

pub mod event;
pub mod guards;
pub mod machine;
pub mod state;
pub mod transition;

pub use event::Event;
pub use guards::{ControlGuard, ThresholdGuard, TimedGuard};
pub use machine::{FsmError, StateMachine, TransitionResult};
pub use state::State;
pub use transition::{ActionFn, GuardFn, Transition};
