//! Component state management with immutable transformations.
//!
//! This module defines traits for component state that enforce
//! immutability through copy-on-write semantics using the with_* pattern.
//!
//! # Traceability
//! - REQ-CMP-001: Components have associated Config, State, and Ports types
//! - REQ-PLT-021: All state transitions are immutable transformations

#![forbid(unsafe_code)]

/// Trait for component state types.
///
/// Component state must be cloneable to support immutable updates.
/// State transitions are performed via methods that return new state
/// instances rather than mutating in place.
///
/// # Design Pattern
///
/// State updates follow the "with_*" pattern, where each state field
/// has a corresponding `with_field_name` method that returns a new
/// state instance with that field updated.
///
/// # Examples
///
/// ```
/// use middleware::component::ComponentState;
///
/// #[derive(Debug, Clone, PartialEq)]
/// struct ControllerState {
///     setpoint: f32,
///     output: f32,
/// }
///
/// impl ControllerState {
///     fn with_setpoint(self, setpoint: f32) -> Self {
///         Self { setpoint, ..self }
///     }
///
///     fn with_output(self, output: f32) -> Self {
///         Self { output, ..self }
///     }
/// }
///
/// impl ComponentState for ControllerState {}
///
/// let state = ControllerState { setpoint: 0.0, output: 0.0 }
///     .with_setpoint(10.0)
///     .with_output(2.5);
/// assert_eq!(state, ControllerState { setpoint: 10.0, output: 2.5 });
/// ```
///
/// # Immutability
///
/// State transitions create new instances:
///
/// ```
/// # #[derive(Debug, Clone, PartialEq)]
/// # struct ControllerState { setpoint: f32, output: f32 }
/// # impl ControllerState {
/// #     fn with_setpoint(self, setpoint: f32) -> Self { Self { setpoint, ..self } }
/// # }
///
/// let state1 = ControllerState { setpoint: 0.0, output: 0.0 };
/// // `with_*` consumes its receiver, so clone to keep the original.
/// let state2 = state1.clone().with_setpoint(10.0);
/// // state1 is unchanged, state2 has setpoint = 10.0
/// assert_eq!(state1.setpoint, 0.0);
/// assert_eq!(state2.setpoint, 10.0);
/// ```
pub trait ComponentState: Clone {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestState {
        counter: u32,
        active: bool,
    }

    impl TestState {
        fn new() -> Self {
            Self {
                counter: 0,
                active: false,
            }
        }

        fn with_counter(self, counter: u32) -> Self {
            Self { counter, ..self }
        }

        fn with_active(self, active: bool) -> Self {
            Self { active, ..self }
        }
    }

    impl ComponentState for TestState {}

    #[test]
    fn state_is_cloneable() {
        let state1 = TestState::new();
        let state2 = state1.clone();
        assert_eq!(state1, state2);
    }

    #[test]
    fn with_pattern_creates_new_instance() {
        let state1 = TestState::new();
        let state2 = state1.clone().with_counter(42);

        assert_eq!(state1.counter, 0);
        assert_eq!(state2.counter, 42);
        assert_eq!(state1.active, state2.active);
    }

    #[test]
    fn with_pattern_preserves_other_fields() {
        let state1 = TestState {
            counter: 10,
            active: true,
        };
        let state2 = state1.clone().with_counter(20);

        assert_eq!(state2.counter, 20);
        assert!(state2.active);
    }

    #[test]
    fn chained_with_methods() {
        let state = TestState::new()
            .with_counter(100)
            .with_active(true);

        assert_eq!(state.counter, 100);
        assert!(state.active);
    }
}
