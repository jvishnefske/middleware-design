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

    /// Every state, in discriminant order.
    const ALL_STATES: [LifecycleState; 7] = [
        LifecycleState::Uninitialized,
        LifecycleState::Initializing,
        LifecycleState::Ready,
        LifecycleState::Running,
        LifecycleState::Paused,
        LifecycleState::ShuttingDown,
        LifecycleState::Terminated,
    ];

    /// Expected transition table, written out independently of
    /// `can_transition_to`. Row = from, column = to, both in `ALL_STATES`
    /// order. `T` = allowed.
    const T: bool = true;
    const F: bool = false;
    const EXPECTED: [[bool; 7]; 7] = [
        //  Uninit Init  Ready  Run   Pause  ShutD  Term
        [F, T, F, F, F, F, F], // Uninitialized
        [F, F, T, F, F, F, F], // Initializing
        [F, F, F, T, F, F, F], // Ready
        [F, F, F, F, T, T, F], // Running
        [F, F, F, T, F, T, F], // Paused
        [F, F, F, F, F, F, T], // ShuttingDown
        [F, F, F, F, F, F, F], // Terminated
    ];

    /// Exhaustiveness guard: adding a variant makes this match fail to
    /// compile, forcing `ALL_STATES` and `EXPECTED` to be extended.
    fn index_of(state: LifecycleState) -> usize {
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

    /// Test double that keeps the default hook bodies.
    struct DefaultHooks {
        state: LifecycleState,
    }

    impl Lifecycle for DefaultHooks {
        fn current_state(&self) -> LifecycleState {
            self.state
        }
    }

    /// Hook names, in the order the requirement lists them.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Hook {
        Init,
        Start,
        Pause,
        Resume,
        Shutdown,
    }

    const ALL_HOOKS: [Hook; 5] = [
        Hook::Init,
        Hook::Start,
        Hook::Pause,
        Hook::Resume,
        Hook::Shutdown,
    ];

    fn call<L: Lifecycle>(component: &mut L, hook: Hook) -> Result<(), LifecycleError> {
        match hook {
            Hook::Init => component.on_init(),
            Hook::Start => component.on_start(),
            Hook::Pause => component.on_pause(),
            Hook::Resume => component.on_resume(),
            Hook::Shutdown => component.on_shutdown(),
        }
    }

    /// Test double whose hooks drive the state machine and can be made to
    /// fail one at a time, to inject hook faults.
    struct DrivenComponent {
        state: LifecycleState,
        failing: Option<Hook>,
        calls: [u8; 5],
    }

    impl DrivenComponent {
        fn new(failing: Option<Hook>) -> Self {
            Self {
                state: LifecycleState::Uninitialized,
                failing,
                calls: [0; 5],
            }
        }

        fn step(&mut self, hook: Hook, path: &[LifecycleState]) -> Result<(), LifecycleError> {
            let slot = match hook {
                Hook::Init => 0,
                Hook::Start => 1,
                Hook::Pause => 2,
                Hook::Resume => 3,
                Hook::Shutdown => 4,
            };
            *self.calls.get_mut(slot).unwrap() += 1;
            if self.failing == Some(hook) {
                return Err(LifecycleError::HookFailed);
            }
            let mut next = self.state;
            for target in path {
                next = next.transition_to(*target)?;
            }
            self.state = next;
            Ok(())
        }
    }

    impl Lifecycle for DrivenComponent {
        fn current_state(&self) -> LifecycleState {
            self.state
        }
        fn on_init(&mut self) -> Result<(), LifecycleError> {
            self.step(
                Hook::Init,
                &[LifecycleState::Initializing, LifecycleState::Ready],
            )
        }
        fn on_start(&mut self) -> Result<(), LifecycleError> {
            self.step(Hook::Start, &[LifecycleState::Running])
        }
        fn on_pause(&mut self) -> Result<(), LifecycleError> {
            self.step(Hook::Pause, &[LifecycleState::Paused])
        }
        fn on_resume(&mut self) -> Result<(), LifecycleError> {
            self.step(Hook::Resume, &[LifecycleState::Running])
        }
        fn on_shutdown(&mut self) -> Result<(), LifecycleError> {
            self.step(
                Hook::Shutdown,
                &[LifecycleState::ShuttingDown, LifecycleState::Terminated],
            )
        }
    }

    /// Exhaustive 7x7 table: every (from, to) pair, valid and invalid,
    /// including self-transitions, checked against an independently written
    /// table through both `can_transition_to` and `transition_to`.
    /// Verifies: REQ-CMP-030
    #[test]
    fn transition_table_is_exhaustive() {
        let mut allowed = 0;
        for (from, row) in ALL_STATES.iter().zip(EXPECTED.iter()) {
            assert_eq!(row.len(), ALL_STATES.len());
            for (to, &expected) in ALL_STATES.iter().zip(row.iter()) {
                assert_eq!(from.can_transition_to(*to), expected, "{from:?} -> {to:?}");
                let result = from.transition_to(*to);
                if expected {
                    allowed += 1;
                    assert_eq!(result, Ok(*to), "{from:?} -> {to:?}");
                } else {
                    assert_eq!(
                        result,
                        Err(LifecycleError::InvalidTransition),
                        "{from:?} -> {to:?}"
                    );
                }
            }
        }
        assert_eq!(allowed, 8);
    }

    /// All seven required states exist, are distinct, and their wire values
    /// equal their discriminant order.
    /// Verifies: REQ-CMP-030
    #[test]
    fn all_seven_states_exist_and_are_distinct() {
        for (position, state) in ALL_STATES.iter().enumerate() {
            assert_eq!(index_of(*state), position);
            assert_eq!(usize::from(u8::from(*state)), position);
            for (other_position, other) in ALL_STATES.iter().enumerate() {
                assert_eq!(state == other, position == other_position);
            }
        }
        assert_eq!(core::mem::size_of::<LifecycleState>(), 1);
    }

    /// Terminated is absorbing and only Uninitialized has no predecessor.
    /// Verifies: REQ-CMP-030
    #[test]
    fn terminated_is_absorbing_and_uninitialized_unreachable() {
        for state in ALL_STATES {
            assert!(!LifecycleState::Terminated.can_transition_to(state));
            assert!(!state.can_transition_to(LifecycleState::Uninitialized));
        }
    }

    /// All five hooks exist with the `fn(&mut Self) -> Result<(),
    /// LifecycleError>` signature (pinned by function pointer coercion), and
    /// the default bodies succeed without changing state.
    /// Verifies: REQ-CMP-031
    #[test]
    fn default_hooks_exist_and_succeed() {
        type HookFn = fn(&mut DefaultHooks) -> Result<(), LifecycleError>;
        let hooks: [HookFn; 5] = [
            <DefaultHooks as Lifecycle>::on_init,
            <DefaultHooks as Lifecycle>::on_start,
            <DefaultHooks as Lifecycle>::on_pause,
            <DefaultHooks as Lifecycle>::on_resume,
            <DefaultHooks as Lifecycle>::on_shutdown,
        ];
        for state in ALL_STATES {
            let mut component = DefaultHooks { state };
            for hook in hooks {
                assert_eq!(hook(&mut component), Ok(()));
                assert_eq!(component.current_state(), state);
            }
        }
    }

    /// Overridden hooks drive the full path, including a pause/resume cycle
    /// and shutdown from Paused, each hook called exactly once per use.
    /// Verifies: REQ-CMP-031
    #[test]
    fn overridden_hooks_drive_full_lifecycle() {
        let mut component = DrivenComponent::new(None);
        call(&mut component, Hook::Init).unwrap();
        assert_eq!(component.current_state(), LifecycleState::Ready);
        call(&mut component, Hook::Start).unwrap();
        assert_eq!(component.current_state(), LifecycleState::Running);
        call(&mut component, Hook::Pause).unwrap();
        assert_eq!(component.current_state(), LifecycleState::Paused);
        call(&mut component, Hook::Resume).unwrap();
        assert_eq!(component.current_state(), LifecycleState::Running);
        call(&mut component, Hook::Pause).unwrap();
        call(&mut component, Hook::Shutdown).unwrap();
        assert_eq!(component.current_state(), LifecycleState::Terminated);
        assert_eq!(component.calls, [1, 1, 2, 1, 1]);
    }

    /// Fault injection: each hook in turn returns `HookFailed`; the error
    /// propagates to the caller and the state is left where it was.
    /// Verifies: REQ-CMP-031
    #[test]
    fn hook_failure_propagates_and_preserves_state() {
        // Hooks needed to bring the component to the state where `hook` is legal.
        let prefix = |hook: Hook| -> &'static [Hook] {
            match hook {
                Hook::Init => &[],
                Hook::Start => &[Hook::Init],
                Hook::Pause | Hook::Shutdown => &[Hook::Init, Hook::Start],
                Hook::Resume => &[Hook::Init, Hook::Start, Hook::Pause],
            }
        };
        for failing in ALL_HOOKS {
            let mut component = DrivenComponent::new(Some(failing));
            for setup in prefix(failing) {
                call(&mut component, *setup).unwrap();
            }
            let before = component.current_state();
            assert_eq!(
                call(&mut component, failing),
                Err(LifecycleError::HookFailed)
            );
            assert_eq!(component.current_state(), before, "{failing:?}");
        }
    }

    /// Error guessing: a hook called out of order is refused by the state
    /// machine with `InvalidTransition`, and the state is unchanged.
    /// Verifies: REQ-CMP-030, REQ-CMP-031
    #[test]
    fn hook_called_out_of_order_is_rejected() {
        let mut component = DrivenComponent::new(None);
        for hook in [Hook::Start, Hook::Pause, Hook::Resume, Hook::Shutdown] {
            assert_eq!(
                call(&mut component, hook),
                Err(LifecycleError::InvalidTransition)
            );
            assert_eq!(component.current_state(), LifecycleState::Uninitialized);
        }
        call(&mut component, Hook::Init).unwrap();
        // Re-initialising, pausing and shutting down from Ready are invalid.
        for hook in [Hook::Init, Hook::Pause, Hook::Shutdown] {
            assert_eq!(
                call(&mut component, hook),
                Err(LifecycleError::InvalidTransition)
            );
        }
        assert_eq!(component.current_state(), LifecycleState::Ready);
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
