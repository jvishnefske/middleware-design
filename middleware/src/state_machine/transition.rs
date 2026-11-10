//! Transition definitions and function types.
//!
//! This module defines the structure of state machine transitions, including
//! guard conditions and actions.
//!
//! # Traceability
//! - REQ-FSM-010: Transitions specify source state, event, target state
//! - REQ-FSM-011: Transitions support optional guard conditions
//! - REQ-FSM-012: Transitions support optional actions
//! - REQ-FSM-013: Transition tables are compile-time constant arrays

use super::{Event, State};
use crate::error::StateMachineError;

/// Function type for guard conditions.
///
/// Guards determine whether a transition is allowed to occur.
///
/// # Parameters
/// - `context`: Reference to the state machine context
///
/// # Returns
/// `true` if the transition is allowed, `false` otherwise
pub type GuardFn<C> = fn(context: &C) -> bool;

/// Function type for transition actions.
///
/// Actions are executed during a transition, after the source state's `on_exit`
/// and before the target state's `on_enter`.
///
/// # Parameters
/// - `context`: Mutable reference to the state machine context
///
/// # Returns
/// `Ok(())` on success, or a [`StateMachineError`] (conventionally
/// [`StateMachineError::ActionFailed`]) if the action failed. The state
/// machine reports any such error as `FsmError::ActionFailed`.
pub type ActionFn<C> = fn(context: &mut C) -> Result<(), StateMachineError>;

/// A state machine transition.
///
/// Transitions are typically stored in compile-time constant arrays.
///
/// # Type Parameters
/// - `S`: State type
/// - `E`: Event type
///
/// # Examples
///
/// Basic transition without guard or action:
/// ```
/// use middleware::state_machine::{Event, State, Transition};
///
/// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// # enum MyState { Idle, Running }
/// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// # enum MyEvent { Start, Stop }
/// # impl Event for MyEvent { const NAME: &'static str = "MyEvent"; }
/// # struct MyContext { initialized: bool }
/// # impl MyContext { fn new() -> Self { Self { initialized: true } } }
/// # impl State for MyState {
/// #     type Context = MyContext;
/// #     fn on_enter(&self, _context: &mut MyContext) -> Result<(), middleware::LifecycleError> { Ok(()) }
/// #     fn on_exit(&self, _context: &mut MyContext) -> Result<(), middleware::LifecycleError> { Ok(()) }
/// # }
///
/// const TRANSITIONS: [Transition<MyState, MyEvent>; 1] = [
///     Transition {
///         source: MyState::Idle,
///         event: MyEvent::Start,
///         target: MyState::Running,
///         guard: None,
///         action: None,
///     },
/// ];
///
/// let t = &TRANSITIONS[0];
/// assert!(t.matches(MyState::Idle, MyEvent::Start));
/// assert!(!t.matches(MyState::Idle, MyEvent::Stop));
/// assert!(!t.matches(MyState::Running, MyEvent::Start));
/// // With no guard the transition is always allowed.
/// assert!(t.is_allowed(&MyContext::new()));
/// ```
///
/// Transition with guard:
/// ```
/// use middleware::state_machine::{Event, State, Transition};
///
/// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// # enum MyState { Idle, Running }
/// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// # enum MyEvent { Start, Stop }
/// # impl Event for MyEvent { const NAME: &'static str = "MyEvent"; }
/// # struct MyContext { initialized: bool }
/// # impl MyContext { fn new() -> Self { Self { initialized: true } } }
/// # impl State for MyState {
/// #     type Context = MyContext;
/// #     fn on_enter(&self, _context: &mut MyContext) -> Result<(), middleware::LifecycleError> { Ok(()) }
/// #     fn on_exit(&self, _context: &mut MyContext) -> Result<(), middleware::LifecycleError> { Ok(()) }
/// # }
///
/// fn is_ready(context: &MyContext) -> bool {
///     context.initialized
/// }
///
/// const TRANSITIONS: [Transition<MyState, MyEvent>; 1] = [
///     Transition {
///         source: MyState::Idle,
///         event: MyEvent::Start,
///         target: MyState::Running,
///         guard: Some(is_ready),
///         action: None,
///     },
/// ];
///
/// let t = &TRANSITIONS[0];
/// assert!(t.is_allowed(&MyContext { initialized: true }));
/// assert!(!t.is_allowed(&MyContext { initialized: false }));
/// ```
#[derive(Clone, Copy)]
pub struct Transition<S, E>
where
    S: State,
    E: Event,
{
    /// The source state from which this transition originates.
    pub source: S,

    /// The event that triggers this transition.
    pub event: E,

    /// The target state to which this transition leads.
    pub target: S,

    /// Optional guard condition that must be satisfied for the transition to occur.
    pub guard: Option<GuardFn<S::Context>>,

    /// Optional action to execute during the transition.
    pub action: Option<ActionFn<S::Context>>,
}

impl<S, E> Transition<S, E>
where
    S: State,
    E: Event,
{
    /// Checks if this transition matches the given source state and event.
    ///
    /// # Parameters
    /// - `current_state`: The current state to match against
    /// - `event`: The event to match against
    ///
    /// # Returns
    /// `true` if both source state and event match, `false` otherwise
    pub fn matches(&self, current_state: S, event: E) -> bool {
        self.source == current_state && self.event == event
    }

    /// Evaluates the guard condition.
    ///
    /// # Parameters
    /// - `context`: Reference to the state machine context
    ///
    /// # Returns
    /// `true` if there is no guard or the guard evaluates to true, `false` otherwise
    pub fn is_allowed(&self, context: &S::Context) -> bool {
        match self.guard {
            Some(guard_fn) => guard_fn(context),
            None => true,
        }
    }

    /// Executes the transition action.
    ///
    /// # Parameters
    /// - `context`: Mutable reference to the state machine context
    ///
    /// # Returns
    /// `Ok(())` if there is no action or the action succeeds.
    ///
    /// # Errors
    /// Returns the [`StateMachineError`] produced by the action, unchanged, if
    /// the action fails.
    pub fn execute_action(&self, context: &mut S::Context) -> Result<(), StateMachineError> {
        match self.action {
            Some(action_fn) => action_fn(context),
            None => Ok(()),
        }
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LifecycleError;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    enum TestState {
        Idle = 0,
        Running = 1,
    }

    struct TestContext {
        ready: bool,
        action_executed: bool,
    }

    impl State for TestState {
        type Context = TestContext;

        fn on_enter(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
            Ok(())
        }

        fn on_exit(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestEvent {
        Start,
        Stop,
    }

    impl Event for TestEvent {
        const NAME: &'static str = "TestEvent";
    }

    fn is_ready(context: &TestContext) -> bool {
        context.ready
    }

    fn mark_action_executed(context: &mut TestContext) -> Result<(), StateMachineError> {
        context.action_executed = true;
        Ok(())
    }

    /// Verifies: REQ-FSM-010
    #[test]
    fn test_transition_matches() {
        let transition = Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: None,
            action: None,
        };

        assert!(transition.matches(TestState::Idle, TestEvent::Start));
        assert!(!transition.matches(TestState::Running, TestEvent::Start));
        assert!(!transition.matches(TestState::Idle, TestEvent::Stop));
    }

    /// Verifies: REQ-FSM-011
    #[test]
    fn test_transition_guard_allowed() {
        let transition = Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: Some(is_ready),
            action: None,
        };

        let mut context = TestContext {
            ready: true,
            action_executed: false,
        };
        assert!(transition.is_allowed(&context));

        context.ready = false;
        assert!(!transition.is_allowed(&context));
    }

    /// Verifies: REQ-FSM-011
    #[test]
    fn test_transition_guard_none() {
        let transition = Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: None,
            action: None,
        };

        let context = TestContext {
            ready: false,
            action_executed: false,
        };
        assert!(transition.is_allowed(&context));
    }

    /// Verifies: REQ-FSM-012
    #[test]
    fn test_transition_action() {
        let transition = Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: None,
            action: Some(mark_action_executed),
        };

        let mut context = TestContext {
            ready: false,
            action_executed: false,
        };

        assert!(transition.execute_action(&mut context).is_ok());
        assert!(context.action_executed);
    }

    /// Verifies: REQ-FSM-012
    #[test]
    fn test_transition_no_action() {
        let transition = Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: None,
            action: None,
        };

        let mut context = TestContext {
            ready: false,
            action_executed: false,
        };

        assert!(transition.execute_action(&mut context).is_ok());
        assert!(!context.action_executed);
    }

    fn failing_action(_context: &mut TestContext) -> Result<(), StateMachineError> {
        Err(StateMachineError::ActionFailed)
    }

    /// Verifies: REQ-FSM-012
    #[test]
    fn test_transition_action_error_propagated() {
        let transition = Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: None,
            action: Some(failing_action),
        };

        let mut context = TestContext {
            ready: false,
            action_executed: false,
        };

        assert_eq!(
            transition.execute_action(&mut context),
            Err(StateMachineError::ActionFailed)
        );
    }

    /// Table built in a `const` item. The `const` assertions below are
    /// evaluated by the compiler, so the test fails to build unless the
    /// table (length and row contents) is a compile-time constant; the
    /// table is then usable as the `&'static` table of a state machine
    /// without any runtime construction.
    /// Verifies: REQ-FSM-013, REQ-FSM-010
    #[test]
    fn test_transition_table_is_compile_time_constant() {
        const TABLE: [Transition<TestState, TestEvent>; 2] = [
            Transition {
                source: TestState::Idle,
                event: TestEvent::Start,
                target: TestState::Running,
                guard: Some(is_ready),
                action: Some(mark_action_executed),
            },
            Transition {
                source: TestState::Running,
                event: TestEvent::Stop,
                target: TestState::Idle,
                guard: None,
                action: None,
            },
        ];
        const LEN: usize = TABLE.len();
        const FIRST_TARGET: TestState = TABLE[0].target;
        const SECOND_HAS_GUARD: bool = TABLE[1].guard.is_some();
        const _: () = assert!(LEN == 2);
        const _: () = assert!(matches!(FIRST_TARGET, TestState::Running));
        const _: () = assert!(!SECOND_HAS_GUARD);

        let context = TestContext {
            ready: true,
            action_executed: false,
        };
        let fsm = crate::state_machine::StateMachine::new(TestState::Idle, context, &TABLE);
        let (fsm, _) = fsm.process(TestEvent::Start).unwrap();
        assert_eq!(fsm.current_state(), TestState::Running);
        assert!(fsm.context().action_executed);
        let (fsm, _) = fsm.process(TestEvent::Stop).unwrap();
        assert_eq!(fsm.current_state(), TestState::Idle);
    }
}
