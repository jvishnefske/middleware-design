//! State machine container and event processing.
//!
//! This module implements the core state machine logic with immutable state transitions.
//!
//! # Traceability
//! - REQ-FSM-020: StateMachine parameterized by State and Event types
//! - REQ-FSM-021: StateMachine.process(event) returns Result<TransitionResult, FsmError>
//! - REQ-FSM-022: TransitionResult indicates Transitioned, Remained, or Ignored
//! - REQ-FSM-023: State machine processing is immutable (returns new instance)
//! - REQ-FSM-024: First matching row whose guard passes fires; Remained if all
//!   matching guards fail; Ignored if no row matches
//! - REQ-FSM-025: Exit, action and entry failures map to distinct `FsmError`s

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
    ///
    /// The first failing hook decides the error and the later hooks do not
    /// run. Because `process` consumes `self` and the error carries no
    /// machine, the machine (state and context, including any changes made
    /// by hooks that ran before the failure) is dropped on error. A caller
    /// that needs its previous machine after a failure must keep a copy
    /// taken before the call (`StateMachine` is `Clone` when its state,
    /// event and context types are).
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

    /// Verifies: REQ-FSM-002, REQ-FSM-011, REQ-FSM-022
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

    /// Verifies: REQ-FSM-011, REQ-FSM-022, REQ-FSM-024
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

    /// Verifies: REQ-FSM-022, REQ-FSM-024
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

    /// Verifies: REQ-FSM-012
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

    fn dup_process(
        low: bool,
        high: bool,
        event: DupEvent,
    ) -> (DupState, TransitionResult<DupState>) {
        let context = DupContext {
            low_ok: low,
            high_ok: high,
        };
        let fsm = StateMachine::new(DupState::Idle, context, &DUP_TRANSITIONS);
        let (fsm, result) = fsm.process(event).unwrap();
        (fsm.current_state(), result)
    }

    /// Verifies: REQ-FSM-024
    #[test]
    fn test_first_guard_false_second_row_fires() {
        assert_eq!(
            dup_process(false, true, DupEvent::Go),
            (
                DupState::High,
                TransitionResult::Transitioned {
                    from: DupState::Idle,
                    to: DupState::High
                }
            )
        );
    }

    /// Verifies: REQ-FSM-024
    #[test]
    fn test_first_passing_row_wins() {
        assert_eq!(
            dup_process(true, true, DupEvent::Go),
            (
                DupState::Low,
                TransitionResult::Transitioned {
                    from: DupState::Idle,
                    to: DupState::Low
                }
            )
        );
    }

    /// Verifies: REQ-FSM-024
    #[test]
    fn test_all_matching_guards_false_remains() {
        assert_eq!(
            dup_process(false, false, DupEvent::Go),
            (DupState::Idle, TransitionResult::Remained)
        );
    }

    /// Verifies: REQ-FSM-024
    #[test]
    fn test_no_matching_row_ignored() {
        assert_eq!(
            dup_process(true, true, DupEvent::Other),
            (DupState::Idle, TransitionResult::Ignored)
        );
    }

    // ---------------------------------------------------------------------
    // Hook ordering and hook failure (REQ-FSM-002, REQ-FSM-012, REQ-FSM-025)
    // ---------------------------------------------------------------------

    /// `repr(u16)` state with non-contiguous discriminants, so the machine is
    /// exercised with a state type other than `repr(u8)`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u16)]
    enum HookState {
        A = 0x0100,
        B = 0x0200,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HookEvent {
        Go,
        Other,
    }

    impl Event for HookEvent {
        const NAME: &'static str = "HookEvent";
    }

    /// Context that injects a failure into any of the three hooks and
    /// records the sequence number and receiver of every hook that ran.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct HookCtx {
        fail_exit: bool,
        fail_action: bool,
        fail_enter: bool,
        seq: u8,
        exit_at: Option<(u8, HookState)>,
        action_at: Option<u8>,
        enter_at: Option<(u8, HookState)>,
    }

    impl HookCtx {
        const fn new(fail_exit: bool, fail_action: bool, fail_enter: bool) -> Self {
            Self {
                fail_exit,
                fail_action,
                fail_enter,
                seq: 0,
                exit_at: None,
                action_at: None,
                enter_at: None,
            }
        }

        fn tick(&mut self) -> u8 {
            self.seq += 1;
            self.seq
        }
    }

    impl State for HookState {
        type Context = HookCtx;

        fn on_enter(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            let at = context.tick();
            context.enter_at = Some((at, *self));
            if context.fail_enter {
                Err(LifecycleError::EntryActionFailed)
            } else {
                Ok(())
            }
        }

        fn on_exit(&self, context: &mut Self::Context) -> Result<(), LifecycleError> {
            let at = context.tick();
            context.exit_at = Some((at, *self));
            if context.fail_exit {
                Err(LifecycleError::ExitActionFailed)
            } else {
                Ok(())
            }
        }
    }

    fn hook_action(context: &mut HookCtx) -> Result<(), StateMachineError> {
        let at = context.tick();
        context.action_at = Some(at);
        if context.fail_action {
            Err(StateMachineError::ActionFailed)
        } else {
            Ok(())
        }
    }

    const HOOK_TRANSITIONS: [Transition<HookState, HookEvent>; 1] = [Transition {
        source: HookState::A,
        event: HookEvent::Go,
        target: HookState::B,
        guard: None,
        action: Some(hook_action),
    }];

    type HookMachine = StateMachine<HookState, HookEvent, HookCtx, 1>;

    fn hook_machine(fail_exit: bool, fail_action: bool, fail_enter: bool) -> HookMachine {
        StateMachine::new(
            HookState::A,
            HookCtx::new(fail_exit, fail_action, fail_enter),
            &HOOK_TRANSITIONS,
        )
    }

    /// The source state's `on_exit` runs first (on the source), then the
    /// row's action, then the target state's `on_enter` (on the target).
    /// Verifies: REQ-FSM-002, REQ-FSM-012
    #[test]
    fn test_hooks_run_exit_action_enter_in_order() {
        let (fsm, result) = hook_machine(false, false, false)
            .process(HookEvent::Go)
            .unwrap();
        assert_eq!(
            result,
            TransitionResult::Transitioned {
                from: HookState::A,
                to: HookState::B
            }
        );
        assert_eq!(fsm.current_state(), HookState::B);
        let ctx = fsm.context();
        assert_eq!(ctx.exit_at, Some((1, HookState::A)));
        assert_eq!(ctx.action_at, Some(2));
        assert_eq!(ctx.enter_at, Some((3, HookState::B)));
        assert_eq!(ctx.seq, 3);
    }

    /// Fault injection over all 2^3 combinations of failing exit / action /
    /// entry hooks. Each failure maps to its own `FsmError` variant, and the
    /// first failing hook in execution order decides the error, so a failure
    /// short-circuits the later hooks (e.g. a failing exit is never masked
    /// by a failing action or entry).
    /// Verifies: REQ-FSM-025, REQ-FSM-002, REQ-FSM-012
    #[test]
    fn test_hook_failure_reports_distinct_error_per_phase() {
        let cases = [
            (false, false, false, Ok(HookState::B)),
            (false, false, true, Err(FsmError::EntryFailed)),
            (false, true, false, Err(FsmError::ActionFailed)),
            (false, true, true, Err(FsmError::ActionFailed)),
            (true, false, false, Err(FsmError::ExitFailed)),
            (true, false, true, Err(FsmError::ExitFailed)),
            (true, true, false, Err(FsmError::ExitFailed)),
            (true, true, true, Err(FsmError::ExitFailed)),
        ];
        for (fail_exit, fail_action, fail_enter, expected) in cases {
            let outcome = hook_machine(fail_exit, fail_action, fail_enter)
                .process(HookEvent::Go)
                .map(|(fsm, _)| fsm.current_state());
            assert_eq!(
                outcome, expected,
                "exit={fail_exit} action={fail_action} entry={fail_enter}"
            );
        }
        // The three error variants are pairwise distinct.
        assert_ne!(FsmError::ExitFailed, FsmError::ActionFailed);
        assert_ne!(FsmError::ActionFailed, FsmError::EntryFailed);
        assert_ne!(FsmError::ExitFailed, FsmError::EntryFailed);
    }

    /// `process` consumes the machine and the error carries no machine, so
    /// on a hook failure the caller keeps its previous state only through a
    /// copy taken before the call. This shows that such a copy (via
    /// `Clone`, available when the context is `Clone`) is unaffected by the
    /// partial hook execution and remains usable.
    /// Verifies: REQ-FSM-025
    #[test]
    fn test_hook_failure_previous_state_retained_via_clone() {
        for (fail_exit, fail_action, fail_enter) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let fsm = hook_machine(fail_exit, fail_action, fail_enter);
            let before = fsm.clone();
            assert!(fsm.process(HookEvent::Go).is_err());

            assert_eq!(before.current_state(), HookState::A);
            assert_eq!(
                *before.context(),
                HookCtx::new(fail_exit, fail_action, fail_enter),
                "hooks that ran before the failure must not touch the retained copy"
            );
            let (before, result) = before.process(HookEvent::Other).unwrap();
            assert_eq!(result, TransitionResult::Ignored);
            assert_eq!(before.current_state(), HookState::A);
        }
    }

    /// Evidence for the REQ-FSM-025 wording problem: the error value is a
    /// single-byte fieldless enum, so it cannot hand the machine (state or
    /// context) back to the caller.
    #[test]
    fn test_hook_failure_error_carries_no_machine() {
        assert_eq!(core::mem::size_of::<FsmError>(), 1);
        let err = hook_machine(true, false, false)
            .process(HookEvent::Go)
            .err();
        assert_eq!(err, Some(FsmError::ExitFailed));
    }

    // ---------------------------------------------------------------------
    // Container properties (REQ-FSM-013, REQ-FSM-020, REQ-FSM-021, REQ-FSM-023)
    // ---------------------------------------------------------------------

    /// `process` takes the machine by value and returns a new machine. The
    /// coercion to a `fn` pointer whose first parameter is the machine type
    /// (not a reference) only compiles for a by-value receiver; the old
    /// instance's clone still holds the old state while the returned one
    /// holds the new state.
    /// Verifies: REQ-FSM-023
    #[test]
    fn test_process_consumes_self_and_returns_new_instance() {
        type M = StateMachine<TestState, TestEvent, TestContext, 3>;
        type Out = Result<(M, TransitionResult<TestState>), FsmError>;
        let process: fn(M, TestEvent) -> Out = M::process;

        let old = StateMachine::new(
            TestState::Idle,
            TestContext {
                ready: true,
                fault_detected: false,
                entry_count: 0,
                exit_count: 0,
            },
            &TRANSITIONS,
        );
        let snapshot = old.clone();
        let (new, _) = process(old, TestEvent::Start).unwrap();
        assert_eq!(new.current_state(), TestState::Running);
        assert_eq!(new.context().entry_count, 1);
        assert_eq!(snapshot.current_state(), TestState::Idle);
        assert_eq!(snapshot.context().entry_count, 0);
    }

    /// `process` returns a `Result` whose error type is `FsmError`; the
    /// success value pairs the new machine with a `TransitionResult`.
    /// Verifies: REQ-FSM-021
    #[test]
    fn test_process_returns_result_of_transition_result_or_fsm_error() {
        type Out = Result<(HookMachine, TransitionResult<HookState>), FsmError>;
        let _process: fn(HookMachine, HookEvent) -> Out = HookMachine::process;

        let ok = hook_machine(false, false, false).process(HookEvent::Go);
        assert!(matches!(ok, Ok((_, TransitionResult::Transitioned { .. }))));
        let err = hook_machine(false, true, false).process(HookEvent::Go);
        assert!(matches!(err, Err(FsmError::ActionFailed)));
    }

    /// The three `TransitionResult` variants, matched exhaustively (no
    /// wildcard, so a new variant would fail to compile), each produced by
    /// the machine for the condition it names.
    /// Verifies: REQ-FSM-022
    #[test]
    fn test_transition_result_is_closed_set_of_three() {
        fn kind(r: TransitionResult<DupState>) -> u8 {
            match r {
                TransitionResult::Transitioned { .. } => 0,
                TransitionResult::Remained => 1,
                TransitionResult::Ignored => 2,
            }
        }
        assert_eq!(kind(dup_process(true, false, DupEvent::Go).1), 0);
        assert_eq!(kind(dup_process(false, false, DupEvent::Go).1), 1);
        assert_eq!(kind(dup_process(true, true, DupEvent::Other).1), 2);
    }

    /// One generic driver works over three unrelated (state, event, context,
    /// N) instantiations, including a `repr(u16)` state.
    /// Verifies: REQ-FSM-020
    #[test]
    fn test_state_machine_generic_over_state_and_event() {
        fn first_step<S, E, C, const N: usize>(
            initial: S,
            context: C,
            table: &'static [Transition<S, E>; N],
            event: E,
        ) -> Result<(S, TransitionResult<S>), FsmError>
        where
            S: State<Context = C> + 'static,
            E: Event + 'static,
        {
            let (fsm, result) = StateMachine::new(initial, context, table).process(event)?;
            Ok((fsm.current_state(), result))
        }

        let ctx = TestContext {
            ready: true,
            fault_detected: false,
            entry_count: 0,
            exit_count: 0,
        };
        assert_eq!(
            first_step(TestState::Idle, ctx, &TRANSITIONS, TestEvent::Start)
                .unwrap()
                .0,
            TestState::Running
        );
        let dup = DupContext {
            low_ok: false,
            high_ok: true,
        };
        assert_eq!(
            first_step(DupState::Idle, dup, &DUP_TRANSITIONS, DupEvent::Go)
                .unwrap()
                .0,
            DupState::High
        );
        assert_eq!(
            first_step(
                HookState::A,
                HookCtx::new(false, false, false),
                &HOOK_TRANSITIONS,
                HookEvent::Go
            )
            .unwrap()
            .0,
            HookState::B
        );
    }

    // ---------------------------------------------------------------------
    // Row selection details (REQ-FSM-024)
    // ---------------------------------------------------------------------

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SelState {
        Idle,
        One,
        Two,
        Three,
        Elsewhere,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum SelEvent {
        Go,
        Other,
    }

    impl Event for SelEvent {
        const NAME: &'static str = "SelEvent";
    }

    /// Guard results per row plus a count of how often each guard ran.
    struct SelCtx {
        pass: [bool; 4],
        calls: [core::cell::Cell<u8>; 4],
    }

    impl SelCtx {
        fn new(pass: [bool; 4]) -> Self {
            Self {
                pass,
                calls: Default::default(),
            }
        }

        fn guard(&self, i: usize) -> bool {
            if let Some(c) = self.calls.get(i) {
                c.set(c.get() + 1);
            }
            self.pass.get(i).copied().unwrap_or(false)
        }

        fn calls(&self) -> [u8; 4] {
            self.calls.each_ref().map(core::cell::Cell::get)
        }
    }

    impl State for SelState {
        type Context = SelCtx;

        fn on_enter(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
            Ok(())
        }

        fn on_exit(&self, _context: &mut Self::Context) -> Result<(), LifecycleError> {
            Ok(())
        }
    }

    fn sel_g0(c: &SelCtx) -> bool {
        c.guard(0)
    }
    fn sel_g1(c: &SelCtx) -> bool {
        c.guard(1)
    }
    fn sel_g2(c: &SelCtx) -> bool {
        c.guard(2)
    }
    fn sel_g3(c: &SelCtx) -> bool {
        c.guard(3)
    }

    /// Row 0 matches on a different source state, row 1 on a different
    /// event; rows 2..=4 all match (Idle, Go): guarded, guarded, unguarded.
    const SEL_TRANSITIONS: [Transition<SelState, SelEvent>; 5] = [
        Transition {
            source: SelState::Elsewhere,
            event: SelEvent::Go,
            target: SelState::Elsewhere,
            guard: Some(sel_g0),
            action: None,
        },
        Transition {
            source: SelState::Idle,
            event: SelEvent::Other,
            target: SelState::Elsewhere,
            guard: Some(sel_g1),
            action: None,
        },
        Transition {
            source: SelState::Idle,
            event: SelEvent::Go,
            target: SelState::One,
            guard: Some(sel_g2),
            action: None,
        },
        Transition {
            source: SelState::Idle,
            event: SelEvent::Go,
            target: SelState::Two,
            guard: Some(sel_g3),
            action: None,
        },
        Transition {
            source: SelState::Idle,
            event: SelEvent::Go,
            target: SelState::Three,
            guard: None,
            action: None,
        },
    ];

    fn sel_run(pass: [bool; 4]) -> (SelState, TransitionResult<SelState>, [u8; 4]) {
        let fsm = StateMachine::new(SelState::Idle, SelCtx::new(pass), &SEL_TRANSITIONS);
        let (fsm, result) = fsm.process(SelEvent::Go).unwrap();
        (fsm.current_state(), result, fsm.context().calls())
    }

    /// Guards of non-matching rows are never evaluated; matching guards are
    /// evaluated in table order and evaluation stops at the first one that
    /// passes; an absent guard counts as passing.
    /// Verifies: REQ-FSM-024
    #[test]
    fn test_row_selection_guard_evaluation_order() {
        // Non-matching rows' guards would pass, but must not be consulted.
        let (state, _, calls) = sel_run([true, true, true, true]);
        assert_eq!(state, SelState::One);
        assert_eq!(calls, [0, 0, 1, 0]);

        let (state, _, calls) = sel_run([true, true, false, true]);
        assert_eq!(state, SelState::Two);
        assert_eq!(calls, [0, 0, 1, 1]);

        // Both guarded rows fail: the unguarded third matching row fires.
        let (state, result, calls) = sel_run([true, true, false, false]);
        assert_eq!(state, SelState::Three);
        assert_eq!(
            result,
            TransitionResult::Transitioned {
                from: SelState::Idle,
                to: SelState::Three
            }
        );
        assert_eq!(calls, [0, 0, 1, 1]);
    }

    /// Boundary: an empty table ignores every event and runs no hook.
    /// Verifies: REQ-FSM-024
    #[test]
    fn test_empty_table_ignores_event() {
        static EMPTY: [Transition<HookState, HookEvent>; 0] = [];
        let fsm = StateMachine::new(HookState::A, HookCtx::new(true, true, true), &EMPTY);
        let (fsm, result) = fsm.process(HookEvent::Go).unwrap();
        assert_eq!(result, TransitionResult::Ignored);
        assert_eq!(fsm.current_state(), HookState::A);
        assert_eq!(*fsm.context(), HookCtx::new(true, true, true));
    }

    /// Remained and Ignored run no hook, even hooks that would fail: the
    /// machine with every hook failing still returns `Ok`.
    /// Verifies: REQ-FSM-024
    #[test]
    fn test_remained_and_ignored_run_no_hooks() {
        fn never(_: &HookCtx) -> bool {
            false
        }
        static BLOCKED: [Transition<HookState, HookEvent>; 1] = [Transition {
            source: HookState::A,
            event: HookEvent::Go,
            target: HookState::B,
            guard: Some(never),
            action: Some(hook_action),
        }];
        let fsm = StateMachine::new(HookState::A, HookCtx::new(true, true, true), &BLOCKED);
        let (fsm, result) = fsm.process(HookEvent::Go).unwrap();
        assert_eq!(result, TransitionResult::Remained);
        let (fsm, result) = fsm.process(HookEvent::Other).unwrap();
        assert_eq!(result, TransitionResult::Ignored);
        assert_eq!(fsm.current_state(), HookState::A);
        assert_eq!(*fsm.context(), HookCtx::new(true, true, true));
    }

    // ---------------------------------------------------------------------
    // Events with associated data (REQ-FSM-003)
    // ---------------------------------------------------------------------

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum DataEvent {
        Timeout(u32),
        Level(f32),
    }

    impl Event for DataEvent {
        const NAME: &'static str = "DataEvent";
    }

    const DATA_TRANSITIONS: [Transition<HookState, DataEvent>; 2] = [
        Transition {
            source: HookState::A,
            event: DataEvent::Timeout(100),
            target: HookState::B,
            guard: None,
            action: None,
        },
        Transition {
            source: HookState::A,
            event: DataEvent::Level(f32::NAN),
            target: HookState::B,
            guard: None,
            action: None,
        },
    ];

    fn data_step(event: DataEvent) -> TransitionResult<HookState> {
        let fsm = StateMachine::new(
            HookState::A,
            HookCtx::new(false, false, false),
            &DATA_TRANSITIONS,
        );
        fsm.process(event).unwrap().1
    }

    /// A data-carrying event matches a row only when the payload is equal
    /// as well (`PartialEq` on the whole event). Error guessing: a NaN
    /// payload is never equal to itself, so a row keyed on a NaN payload can
    /// never fire.
    /// Verifies: REQ-FSM-003
    #[test]
    fn test_data_event_matches_on_payload() {
        assert_eq!(
            data_step(DataEvent::Timeout(100)),
            TransitionResult::Transitioned {
                from: HookState::A,
                to: HookState::B
            }
        );
        assert_eq!(data_step(DataEvent::Timeout(99)), TransitionResult::Ignored);
        assert_eq!(
            data_step(DataEvent::Timeout(101)),
            TransitionResult::Ignored
        );
        assert_eq!(
            data_step(DataEvent::Level(f32::NAN)),
            TransitionResult::Ignored
        );
    }
}
