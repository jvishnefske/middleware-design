//! State machine container and event processing.
//!
//! This module implements the core state machine logic with immutable state transitions.
//!
//! # Traceability
//! - REQ-FSM-020: StateMachine parameterized by State and Event types
//! - REQ-FSM-021: StateMachine.process(event) returns Result<TransitionResult, FsmError>
//! - REQ-FSM-022: TransitionResult indicates Transitioned, Remained, or Ignored
//! - REQ-FSM-023: State machine processing is immutable (returns new instance)

use super::{Event, State, Transition};

/// Errors that can occur during state machine operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsmError {
    /// State entry action failed.
    EntryFailed,
    /// State exit action failed.
    ExitFailed,
    /// Transition action failed.
    ActionFailed,
}

/// Result of processing an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionResult<S>
where
    S: State,
{
    /// Transition occurred from one state to another.
    Transitioned {
        /// The state we transitioned from.
        from: S,
        /// The state we transitioned to.
        to: S,
    },
    /// No transition occurred: at least one row matched the current state
    /// and event, but every matching row's guard evaluated to false.
    Remained,
    /// Event was ignored: no row matched the current state and event.
    Ignored,
}

/// A finite state machine with compile-time transition table.
///
/// The state machine is immutable - processing an event returns a new instance
/// with updated state.
///
/// # Type Parameters
/// - `S`: State type
/// - `E`: Event type
/// - `C`: Context type
/// - `N`: Number of transitions in the transition table
///
/// # Examples
///
/// Basic state machine:
/// ```
/// use middleware::state_machine::{Event, State, StateMachine, Transition, TransitionResult};
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
/// const TRANSITIONS: [Transition<MyState, MyEvent>; 2] = [
///     Transition {
///         source: MyState::Idle,
///         event: MyEvent::Start,
///         target: MyState::Running,
///         guard: None,
///         action: None,
///     },
///     Transition {
///         source: MyState::Running,
///         event: MyEvent::Stop,
///         target: MyState::Idle,
///         guard: None,
///         action: None,
///     },
/// ];
///
/// let context = MyContext::new();
/// let fsm = StateMachine::new(MyState::Idle, context, &TRANSITIONS);
///
/// let (fsm, result) = fsm.process(MyEvent::Start).unwrap();
/// assert_eq!(
///     result,
///     TransitionResult::Transitioned { from: MyState::Idle, to: MyState::Running }
/// );
/// assert_eq!(fsm.current_state(), MyState::Running);
///
/// // No transition matches `Start` while `Running`, so the event is ignored.
/// let (fsm, result) = fsm.process(MyEvent::Start).unwrap();
/// assert_eq!(result, TransitionResult::Ignored);
/// assert_eq!(fsm.current_state(), MyState::Running);
/// ```
#[derive(Clone)]
pub struct StateMachine<S, E, C, const N: usize>
where
    S: State<Context = C> + 'static,
    E: Event + 'static,
{
    /// Current state of the machine.
    current_state: S,
    /// Context data available to states and transitions.
    context: C,
    /// Compile-time constant transition table.
    transitions: &'static [Transition<S, E>; N],
}

impl<S, E, C, const N: usize> StateMachine<S, E, C, N>
where
    S: State<Context = C> + 'static,
    E: Event + 'static,
{
    /// Creates a new state machine.
    ///
    /// # Parameters
    /// - `initial_state`: The initial state of the machine
    /// - `context`: The initial context
    /// - `transitions`: Static reference to the transition table
    ///
    /// # Returns
    /// A new state machine instance
    pub fn new(initial_state: S, context: C, transitions: &'static [Transition<S, E>; N]) -> Self {
        Self {
            current_state: initial_state,
            context,
            transitions,
        }
    }

    /// Returns the current state.
    pub fn current_state(&self) -> S {
        self.current_state
    }

    /// Returns a reference to the context.
    pub fn context(&self) -> &C {
        &self.context
    }

    /// Processes an event, potentially transitioning to a new state.
    ///
    /// This method is immutable - it consumes self and returns a new instance.
    ///
    /// # Row selection
    /// Rows are scanned in table order. The first row whose source state and
    /// event match, and whose guard is absent or evaluates to true, fires.
    /// Guards of matching rows are evaluated in table order until one passes;
    /// later rows' guards are not evaluated. If at least one row matched but
    /// every matching guard was false the result is
    /// [`TransitionResult::Remained`]; if no row matched it is
    /// [`TransitionResult::Ignored`]. In both cases no hook runs and the
    /// state is unchanged. This is the same rule as the `step` function
    /// generated by `middleware_model!`. The scan is bounded by `N`.
    ///
    /// When a row fires, the hooks run in this order: the current state's
    /// `on_exit`, the row's action, then the target state's `on_enter`.
    ///
    /// # Parameters
    /// - `event`: The event to process
    ///
    /// # Returns
    /// `Ok((new_machine, result))` on success, where `new_machine` is the updated
    /// state machine and `result` indicates what happened.
    /// `Err(error)` if a state entry, exit, or transition action failed.
    ///
    /// # Errors
    /// - `FsmError::ExitFailed`: The exit action of the current state failed
    /// - `FsmError::ActionFailed`: The transition action failed
    /// - `FsmError::EntryFailed`: The entry action of the target state failed
    pub fn process(mut self, event: E) -> Result<(Self, TransitionResult<S>), FsmError> {
        // First matching row whose guard passes, remembering whether any
        // row matched at all.
        let current = self.current_state;
        let context = &self.context;
        let mut any_matched = false;
        let fired = self
            .transitions
            .iter()
            .filter(|t| t.matches(current, event))
            .inspect(|_| any_matched = true)
            .find(|t| t.is_allowed(context));

        let transition = match (fired, any_matched) {
            (Some(t), _) => t,
            (None, true) => return Ok((self, TransitionResult::Remained)),
            (None, false) => return Ok((self, TransitionResult::Ignored)),
        };

        let from_state = self.current_state;
        let to_state = transition.target;

        // Execute state exit
        self.current_state
            .on_exit(&mut self.context)
            .map_err(|_| FsmError::ExitFailed)?;

        // Execute transition action
        transition
            .execute_action(&mut self.context)
            .map_err(|_| FsmError::ActionFailed)?;

        // Update state
        self.current_state = to_state;

        // Execute state entry
        self.current_state
            .on_enter(&mut self.context)
            .map_err(|_| FsmError::EntryFailed)?;

        Ok((
            self,
            TransitionResult::Transitioned {
                from: from_state,
                to: to_state,
            },
        ))
    }
}

// test code: a panic is a test failure, which is the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{LifecycleError, StateMachineError};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    enum TestState {
        Idle = 0,
        Running = 1,
        Fault = 2,
    }

    #[derive(Clone)]
    struct TestContext {
        ready: bool,
        fault_detected: bool,
        entry_count: u32,
        exit_count: u32,
    }

    impl State for TestState {
        type Context = TestContext;

        fn on_enter(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            context.entry_count += 1;
            Ok(())
        }

        fn on_exit(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            context.exit_count += 1;
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestEvent {
        Start,
        Stop,
        Error,
    }

    impl Event for TestEvent {
        const NAME: &'static str = "TestEvent";
    }

    fn is_ready(context: &TestContext) -> bool {
        context.ready
    }

    fn record_fault(context: &mut TestContext) -> Result<(), StateMachineError> {
        context.fault_detected = true;
        Ok(())
    }

    const TRANSITIONS: [Transition<TestState, TestEvent>; 3] = [
        Transition {
            source: TestState::Idle,
            event: TestEvent::Start,
            target: TestState::Running,
            guard: Some(is_ready),
            action: None,
        },
        Transition {
            source: TestState::Running,
            event: TestEvent::Stop,
            target: TestState::Idle,
            guard: None,
            action: None,
        },
        Transition {
            source: TestState::Running,
            event: TestEvent::Error,
            target: TestState::Fault,
            guard: None,
            action: Some(record_fault),
        },
    ];

    #[test]
    fn test_successful_transition() {
        let context = TestContext {
            ready: true,
            fault_detected: false,
            entry_count: 0,
            exit_count: 0,
        };

        let fsm = StateMachine::new(TestState::Idle, context, &TRANSITIONS);
        let (fsm, result) = fsm.process(TestEvent::Start).unwrap();

        assert_eq!(
            result,
            TransitionResult::Transitioned {
                from: TestState::Idle,
                to: TestState::Running
            }
        );
        assert_eq!(fsm.current_state(), TestState::Running);
        assert_eq!(fsm.context().entry_count, 1);
        assert_eq!(fsm.context().exit_count, 1);
    }

    #[test]
    fn test_guard_blocks_transition() {
        let context = TestContext {
            ready: false,
            fault_detected: false,
            entry_count: 0,
            exit_count: 0,
        };

        let fsm = StateMachine::new(TestState::Idle, context, &TRANSITIONS);
        let (fsm, result) = fsm.process(TestEvent::Start).unwrap();

        assert_eq!(result, TransitionResult::Remained);
        assert_eq!(fsm.current_state(), TestState::Idle);
        assert_eq!(fsm.context().entry_count, 0);
        assert_eq!(fsm.context().exit_count, 0);
    }

    #[test]
    fn test_ignored_event() {
        let context = TestContext {
            ready: true,
            fault_detected: false,
            entry_count: 0,
            exit_count: 0,
        };

        let fsm = StateMachine::new(TestState::Idle, context, &TRANSITIONS);
        let (fsm, result) = fsm.process(TestEvent::Stop).unwrap();

        assert_eq!(result, TransitionResult::Ignored);
        assert_eq!(fsm.current_state(), TestState::Idle);
        assert_eq!(fsm.context().entry_count, 0);
        assert_eq!(fsm.context().exit_count, 0);
    }

    #[test]
    fn test_multiple_transitions() {
        let context = TestContext {
            ready: true,
            fault_detected: false,
            entry_count: 0,
            exit_count: 0,
        };

        let fsm = StateMachine::new(TestState::Idle, context, &TRANSITIONS);

        // Idle -> Running
        let (fsm, result) = fsm.process(TestEvent::Start).unwrap();
        assert_eq!(
            result,
            TransitionResult::Transitioned {
                from: TestState::Idle,
                to: TestState::Running
            }
        );
        assert_eq!(fsm.current_state(), TestState::Running);

        // Running -> Fault
        let (fsm, result) = fsm.process(TestEvent::Error).unwrap();
        assert_eq!(
            result,
            TransitionResult::Transitioned {
                from: TestState::Running,
                to: TestState::Fault
            }
        );
        assert_eq!(fsm.current_state(), TestState::Fault);
        assert!(fsm.context().fault_detected);

        // Verify lifecycle hooks called
        assert_eq!(fsm.context().entry_count, 2);
        assert_eq!(fsm.context().exit_count, 2);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DupState {
        Idle,
        Low,
        High,
    }

    #[derive(Clone)]
    struct DupContext {
        low_ok: bool,
        high_ok: bool,
    }

    impl State for DupState {
        type Context = DupContext;

        fn on_enter(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
            Ok(())
        }

        fn on_exit(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
            Ok(())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DupEvent {
        Go,
        Other,
    }

    impl Event for DupEvent {
        const NAME: &'static str = "DupEvent";
    }

    fn low_ok(context: &DupContext) -> bool {
        context.low_ok
    }

    fn high_ok(context: &DupContext) -> bool {
        context.high_ok
    }

    /// Two rows share (Idle, Go); both are guarded.
    const DUP_TRANSITIONS: [Transition<DupState, DupEvent>; 2] = [
        Transition {
            source: DupState::Idle,
            event: DupEvent::Go,
            target: DupState::Low,
            guard: Some(low_ok),
            action: None,
        },
        Transition {
            source: DupState::Idle,
            event: DupEvent::Go,
            target: DupState::High,
            guard: Some(high_ok),
            action: None,
        },
    ];

    fn dup_process(low: bool, high: bool, event: DupEvent) -> (DupState, TransitionResult<DupState>) {
        let context = DupContext { low_ok: low, high_ok: high };
        let fsm = StateMachine::new(DupState::Idle, context, &DUP_TRANSITIONS);
        let (fsm, result) = fsm.process(event).unwrap();
        (fsm.current_state(), result)
    }

    #[test]
    fn test_first_guard_false_second_row_fires() {
        assert_eq!(
            dup_process(false, true, DupEvent::Go),
            (
                DupState::High,
                TransitionResult::Transitioned { from: DupState::Idle, to: DupState::High }
            )
        );
    }

    #[test]
    fn test_first_passing_row_wins() {
        assert_eq!(
            dup_process(true, true, DupEvent::Go),
            (
                DupState::Low,
                TransitionResult::Transitioned { from: DupState::Idle, to: DupState::Low }
            )
        );
    }

    #[test]
    fn test_all_matching_guards_false_remains() {
        assert_eq!(
            dup_process(false, false, DupEvent::Go),
            (DupState::Idle, TransitionResult::Remained)
        );
    }

    #[test]
    fn test_no_matching_row_ignored() {
        assert_eq!(
            dup_process(true, true, DupEvent::Other),
            (DupState::Idle, TransitionResult::Ignored)
        );
    }
}
