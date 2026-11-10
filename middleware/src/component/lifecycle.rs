//! Component lifecycle management.
//!
//! Defines lifecycle states and hooks for component initialization,
//! startup, shutdown, and state transitions.
//!
//! # Traceability
//! - REQ-CMP-030: Components support lifecycle states
//! - REQ-CMP-031: Lifecycle trait provides state transition hooks

#![forbid(unsafe_code)]

use crate::error::LifecycleError;

/// Lifecycle states for components.
///
/// Components progress through these states during their lifetime.
/// State transitions are managed by the framework and validated to
/// ensure correctness.
///
/// # State Machine
///
/// ```text
/// Uninitialized -> Initializing -> Ready -> Running <-> Paused
///                                              |
///                                              v
///                                        ShuttingDown -> Terminated
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LifecycleState {
    /// Component not yet initialized
    Uninitialized = 0,

    /// Component is initializing resources
    Initializing = 1,

    /// Component is ready but not running
    Ready = 2,

    /// Component is actively running
    Running = 3,

    /// Component execution is paused
    Paused = 4,

    /// Component is shutting down
    ShuttingDown = 5,

    /// Component has terminated
    Terminated = 6,
}

impl From<LifecycleState> for u8 {
    /// Returns the state's wire value, equal to its `#[repr(u8)]`
    /// discriminant.
    fn from(state: LifecycleState) -> Self {
        match state {
            LifecycleState::Uninitialized => 0,
            LifecycleState::Initializing => 1,
            LifecycleState::Ready => 2,
            LifecycleState::Running => 3,
            LifecycleState::Paused => 4,
            LifecycleState::ShuttingDown => 5,
            LifecycleState::Terminated => 6,
        }
    }
}

impl LifecycleState {
    /// Check if transition to target state is valid.
    ///
    /// # Examples
    ///
    /// ```
    /// use middleware::component::lifecycle::LifecycleState;
    ///
    /// assert!(LifecycleState::Uninitialized.can_transition_to(LifecycleState::Initializing));
    /// assert!(!LifecycleState::Terminated.can_transition_to(LifecycleState::Running));
    /// ```
    pub fn can_transition_to(self, target: LifecycleState) -> bool {
        use LifecycleState::*;

        matches!(
            (self, target),
            (Uninitialized, Initializing)
                | (Initializing, Ready)
                | (Ready, Running)
                | (Running, Paused)
                | (Paused, Running)
                | (Running, ShuttingDown)
                | (Paused, ShuttingDown)
                | (ShuttingDown, Terminated)
        )
    }

    /// Validate and perform state transition.
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::InvalidTransition` if the transition
    /// is not allowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use middleware::component::lifecycle::LifecycleState;
    ///
    /// let state = LifecycleState::Uninitialized;
    /// let new_state = state.transition_to(LifecycleState::Initializing).unwrap();
    /// assert_eq!(new_state, LifecycleState::Initializing);
    /// ```
    pub fn transition_to(self, target: LifecycleState) -> Result<LifecycleState, LifecycleError> {
        if self.can_transition_to(target) {
            Ok(target)
        } else {
            Err(LifecycleError::InvalidTransition)
        }
    }
}

/// Trait for components with lifecycle management.
///
/// Provides hooks for state transitions allowing components to
/// perform initialization, cleanup, and state-specific actions.
///
/// All hooks return `Result` to allow error reporting during
/// transitions.
///
/// # Examples
///
/// ```
/// use middleware::LifecycleError;
/// use middleware::component::{Lifecycle, LifecycleState};
///
/// struct MyComponent {
///     state: LifecycleState,
/// }
///
/// impl Lifecycle for MyComponent {
///     fn current_state(&self) -> LifecycleState {
///         self.state
///     }
///
///     fn on_init(&mut self) -> Result<(), LifecycleError> {
///         // Initialize resources
///         self.state = self.state.transition_to(LifecycleState::Initializing)?;
///         self.state = self.state.transition_to(LifecycleState::Ready)?;
///         Ok(())
///     }
///
///     fn on_start(&mut self) -> Result<(), LifecycleError> {
///         // Begin execution
///         self.state = self.state.transition_to(LifecycleState::Running)?;
///         Ok(())
///     }
/// }
///
/// let mut component = MyComponent { state: LifecycleState::Uninitialized };
/// // Starting before initialising is rejected.
/// assert_eq!(component.on_start(), Err(LifecycleError::InvalidTransition));
///
/// assert_eq!(component.on_init(), Ok(()));
/// assert_eq!(component.on_start(), Ok(()));
/// assert_eq!(component.current_state(), LifecycleState::Running);
/// // Hooks that are not overridden default to `Ok(())`.
/// assert_eq!(component.on_pause(), Ok(()));
/// ```
pub trait Lifecycle {
    /// Get the current lifecycle state.
    fn current_state(&self) -> LifecycleState;

    /// Called when transitioning from Uninitialized to Initializing.
    ///
    /// Perform resource initialization, configuration loading, and
    /// hardware setup. Component should be ready to run after this
    /// completes successfully.
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::HookFailed` if initialization fails.
    fn on_init(&mut self) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Called when transitioning from Ready to Running.
    ///
    /// Begin active execution. For active components, this starts
    /// the execution cycle.
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::HookFailed` if startup fails.
    fn on_start(&mut self) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Called when transitioning from Running to Paused.
    ///
    /// Temporarily suspend execution while maintaining state.
    /// The component should be resumable.
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::HookFailed` if pause fails.
    fn on_pause(&mut self) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Called when transitioning from Paused to Running.
    ///
    /// Resume execution from paused state.
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::HookFailed` if resume fails.
    fn on_resume(&mut self) -> Result<(), LifecycleError> {
        Ok(())
    }

    /// Called when transitioning to ShuttingDown.
    ///
    /// Perform cleanup, release resources, and prepare for
    /// termination. This is the last chance to save state or
    /// perform orderly shutdown.
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::HookFailed` if shutdown fails.
    fn on_shutdown(&mut self) -> Result<(), LifecycleError> {
        Ok(())
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;

    struct TestComponent {
        state: LifecycleState,
        init_called: bool,
        start_called: bool,
        shutdown_called: bool,
    }

    impl TestComponent {
        fn new() -> Self {
            Self {
                state: LifecycleState::Uninitialized,
                init_called: false,
                start_called: false,
                shutdown_called: false,
            }
        }
    }

    impl Lifecycle for TestComponent {
        fn current_state(&self) -> LifecycleState {
            self.state
        }

        fn on_init(&mut self) -> Result<(), LifecycleError> {
            self.init_called = true;
            self.state = LifecycleState::Ready;
            Ok(())
        }

        fn on_start(&mut self) -> Result<(), LifecycleError> {
            self.start_called = true;
            self.state = LifecycleState::Running;
            Ok(())
        }

        fn on_shutdown(&mut self) -> Result<(), LifecycleError> {
            self.shutdown_called = true;
            self.state = LifecycleState::Terminated;
            Ok(())
        }
    }

    #[test]
    fn lifecycle_state_values() {
        assert_eq!(u8::from(LifecycleState::Uninitialized), 0);
        assert_eq!(u8::from(LifecycleState::Initializing), 1);
        assert_eq!(u8::from(LifecycleState::Ready), 2);
        assert_eq!(u8::from(LifecycleState::Running), 3);
        assert_eq!(u8::from(LifecycleState::Paused), 4);
        assert_eq!(u8::from(LifecycleState::ShuttingDown), 5);
        assert_eq!(u8::from(LifecycleState::Terminated), 6);
    }

    #[test]
    fn valid_transitions() {
        assert!(LifecycleState::Uninitialized.can_transition_to(LifecycleState::Initializing));
        assert!(LifecycleState::Initializing.can_transition_to(LifecycleState::Ready));
        assert!(LifecycleState::Ready.can_transition_to(LifecycleState::Running));
        assert!(LifecycleState::Running.can_transition_to(LifecycleState::Paused));
        assert!(LifecycleState::Paused.can_transition_to(LifecycleState::Running));
        assert!(LifecycleState::Running.can_transition_to(LifecycleState::ShuttingDown));
        assert!(LifecycleState::ShuttingDown.can_transition_to(LifecycleState::Terminated));
    }

    #[test]
    fn invalid_transitions() {
        assert!(!LifecycleState::Uninitialized.can_transition_to(LifecycleState::Running));
        assert!(!LifecycleState::Terminated.can_transition_to(LifecycleState::Running));
        assert!(!LifecycleState::Ready.can_transition_to(LifecycleState::Terminated));
    }

    #[test]
    fn transition_to_valid() {
        let state = LifecycleState::Ready;
        let new_state = state.transition_to(LifecycleState::Running);
        assert_eq!(new_state, Ok(LifecycleState::Running));
    }

    #[test]
    fn transition_to_invalid() {
        let state = LifecycleState::Uninitialized;
        let result = state.transition_to(LifecycleState::Running);
        assert_eq!(result, Err(LifecycleError::InvalidTransition));
    }

    #[test]
    fn lifecycle_hooks_called() {
        let mut component = TestComponent::new();

        assert_eq!(component.current_state(), LifecycleState::Uninitialized);

        component.on_init().unwrap();
        assert!(component.init_called);
        assert_eq!(component.current_state(), LifecycleState::Ready);

        component.on_start().unwrap();
        assert!(component.start_called);
        assert_eq!(component.current_state(), LifecycleState::Running);

        component.on_shutdown().unwrap();
        assert!(component.shutdown_called);
        assert_eq!(component.current_state(), LifecycleState::Terminated);
    }

    #[test]
    fn lifecycle_state_is_copy() {
        let s1 = LifecycleState::Running;
        let s2 = s1;
        assert_eq!(s1, s2);
    }
}
