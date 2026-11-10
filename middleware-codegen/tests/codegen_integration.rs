//! Integration tests for the code generation proc-macro.
//!
//! These tests verify that the proc-macro correctly generates Rust code
//! from YAML model definitions.
//!
//! # Traceability
//! - REQ-GEN-010: middleware_model! macro reads YAML at compile time
//! - REQ-GEN-011: Macro generates Rust structs, traits, and implementations
//! - REQ-GEN-013: Generated code includes traceability comments
//! - REQ-FSM-002: Generated machines run on_enter/on_exit actions
//! - REQ-FSM-011: Generated machines evaluate optional guards
//! - REQ-FSM-012: Generated machines run optional transition actions
//!
//! Models here have no ports or components, so the generated code needs no
//! runtime crate. Component generation is exercised by
//! `middleware/tests/attitude_controller_model.rs`.

use middleware_codegen::middleware_model_inline;

// Test basic enum generation
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_enums"
types:
  enums:
    - name: TestState
      repr: u8
      traces: [REQ-TEST-001]
      variants:
        - Idle
        - Running
        - Stopped
"#
}

#[test]
fn test_enum_generation() {
    use test_enums::TestState;

    // Verify enum exists and has correct variants
    let idle = TestState::Idle;
    let running = TestState::Running;
    let stopped = TestState::Stopped;

    // Verify discriminant values
    assert_eq!(idle as u8, 0);
    assert_eq!(running as u8, 1);
    assert_eq!(stopped as u8, 2);

    // Verify COUNT constant
    assert_eq!(TestState::COUNT, 3);

    // Verify ALL array
    assert_eq!(TestState::ALL.len(), 3);
    assert_eq!(TestState::ALL[0], TestState::Idle);
    assert_eq!(TestState::ALL[1], TestState::Running);
    assert_eq!(TestState::ALL[2], TestState::Stopped);
}

// Test struct generation
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_structs"
types:
  structs:
    - name: SensorData
      traces: [REQ-TEST-002]
      fields:
        - name: temperature
          type: f32
        - name: pressure
          type: f32
        - name: timestamp
          type: u64
"#
}

#[test]
fn test_struct_generation() {
    use test_structs::SensorData;

    let data = SensorData {
        temperature: 25.5,
        pressure: 101.3,
        timestamp: 1000,
    };

    assert_eq!(data.temperature, 25.5);
    assert_eq!(data.pressure, 101.3);
    assert_eq!(data.timestamp, 1000);
}

// Test message generation
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_messages"
messages:
  - name: Command
    traces: [REQ-TEST-003]
    fields:
      - name: id
        type: u16
      - name: value
        type: f32
"#
}

#[test]
fn test_message_generation() {
    use test_messages::Command;

    let cmd = Command {
        id: 42,
        value: 3.14,
    };

    assert_eq!(cmd.id, 42);
    assert!((cmd.value - 3.14).abs() < 0.001);
}

// Test math block generation
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_math"
math_blocks:
  - name: pitch_controller
    traces: [REQ-TEST-004]
    kind: pid
    config:
      kp: 1.5
      ki: 0.1
      kd: 0.05
      output_min: -1.0
      output_max: 1.0
      integral_max: 0.5
  - name: asymmetric
    kind: pid
    config:
      integral_min: -0.2
      integral_max: 0.4
  - name: sensor_filter
    traces: [REQ-TEST-005]
    kind: low_pass
    config:
      cutoff_hz: 10.0
      sample_rate_hz: 100.0
"#
}

#[test]
fn test_math_block_generation() {
    // Verify PID constants
    assert_eq!(test_math::PITCH_CONTROLLER::KP, 1.5);
    assert_eq!(test_math::PITCH_CONTROLLER::KI, 0.1);
    assert_eq!(test_math::PITCH_CONTROLLER::KD, 0.05);
    assert_eq!(test_math::PITCH_CONTROLLER::OUTPUT_MIN, -1.0);
    assert_eq!(test_math::PITCH_CONTROLLER::OUTPUT_MAX, 1.0);
    assert_eq!(test_math::PITCH_CONTROLLER::INTEGRAL_MAX, 0.5);
    // integral_min unset: symmetric with integral_max
    assert_eq!(test_math::PITCH_CONTROLLER::INTEGRAL_MIN, -0.5);
    assert_eq!(test_math::ASYMMETRIC::INTEGRAL_MIN, -0.2);
    assert_eq!(test_math::ASYMMETRIC::INTEGRAL_MAX, 0.4);

    // Verify filter constants
    assert_eq!(test_math::SENSOR_FILTER::CUTOFF_HZ, 10.0);
    assert_eq!(test_math::SENSOR_FILTER::SAMPLE_RATE_HZ, 100.0);
}

// Test state machine generation
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_fsm"
types:
  enums:
    - name: State
      repr: u8
      variants:
        - Idle
        - Running
        - Error
    - name: Event
      repr: u8
      variants:
        - Start
        - Stop
        - Fault
state_machines:
  - name: Controller
    traces: [REQ-TEST-006]
    state_type: State
    initial_state: Idle
    events: Event
    states:
      - name: Idle
        on_enter: reset_controller
      - name: Running
        on_enter: start_control
        on_exit: stop_control
      - name: Error
        on_enter: log_error
    transitions:
      - from: Idle
        event: Start
        to: Running
      - from: Running
        event: Stop
        to: Idle
      - from: Running
        event: Fault
        to: Error
"#
}

#[test]
fn test_state_machine_generation() {
    use test_fsm::{Event, State};
    use test_fsm::Controller;

    // Verify initial state
    assert_eq!(Controller::INITIAL_STATE as u8, State::Idle as u8);

    // Verify transition count
    assert_eq!(Controller::TRANSITION_COUNT, 3);

    // Verify transitions array
    assert_eq!(Controller::TRANSITIONS.len(), 3);

    // Verify first transition (Idle --Start--> Running)
    let row = Controller::TRANSITIONS[0];
    assert_eq!(row.from, State::Idle);
    assert_eq!(row.event, Event::Start);
    assert_eq!(row.to, State::Running);
    assert_eq!(row.guard, None);
    assert_eq!(row.action, None);

    // Test find_transition function
    let next = Controller::find_transition(State::Idle, Event::Start).map(|t| t.to);
    assert_eq!(next, Some(State::Running));

    let next = Controller::find_transition(State::Running, Event::Stop).map(|t| t.to);
    assert_eq!(next, Some(State::Idle));

    // Enter/exit lookups follow the model
    assert_eq!(Controller::on_enter(State::Idle), Some(Controller::Action::ResetController));
    assert_eq!(Controller::on_exit(State::Running), Some(Controller::Action::StopControl));
    assert_eq!(Controller::on_exit(State::Idle), None);

    // Test non-existent transition
    let next = Controller::find_transition(State::Idle, Event::Stop);
    assert!(next.is_none());
}

// Test traceability generation
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_trace"
requirements:
  - id: REQ-TRACE-001
    description: "First requirement"
    level: C
  - id: REQ-TRACE-002
    description: "Second requirement"
    level: B
"#
}

#[test]
fn test_traceability_generation() {
    use test_trace::traceability;

    assert_eq!(traceability::REQUIREMENT_COUNT, 2);

    assert_eq!(traceability::REQUIREMENT_IDS[0], "REQ-TRACE-001");
    assert_eq!(traceability::REQUIREMENT_IDS[1], "REQ-TRACE-002");

    assert_eq!(traceability::REQUIREMENT_DESCRIPTIONS[0], "First requirement");
    assert_eq!(traceability::REQUIREMENT_DESCRIPTIONS[1], "Second requirement");

    assert_eq!(traceability::REQUIREMENT_LEVELS[0], "C");
    assert_eq!(traceability::REQUIREMENT_LEVELS[1], "B");
}

// Test complete model with all features
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "complete_model"
requirements:
  - id: REQ-COMP-001
    description: "Complete model test"
    level: C
types:
  enums:
    - name: Mode
      repr: u8
      traces: [REQ-COMP-001]
      variants:
        - Off
        - On
  structs:
    - name: Config
      traces: [REQ-COMP-001]
      fields:
        - name: enabled
          type: bool
        - name: threshold
          type: f32
messages:
  - name: Status
    traces: [REQ-COMP-001]
    fields:
      - name: mode
        type: u8
      - name: value
        type: f32
math_blocks:
  - name: controller
    traces: [REQ-COMP-001]
    kind: pid
    config:
      kp: 1.0
      ki: 0.1
      kd: 0.01
"#
}

#[test]
fn test_complete_model() {
    use complete_model::{Config, Mode, Status};

    // Test enum
    assert_eq!(Mode::COUNT, 2);
    assert_eq!(Mode::Off as u8, 0);
    assert_eq!(Mode::On as u8, 1);

    // Test struct
    let config = Config {
        enabled: true,
        threshold: 0.5,
    };
    assert!(config.enabled);
    assert_eq!(config.threshold, 0.5);

    // Test message
    let status = Status {
        mode: Mode::On as u8,
        value: 42.0,
    };
    assert_eq!(status.mode, 1);
    assert_eq!(status.value, 42.0);

    // Test math block
    assert_eq!(complete_model::CONTROLLER::KP, 1.0);
    assert_eq!(complete_model::CONTROLLER::KI, 0.1);
    assert_eq!(complete_model::CONTROLLER::KD, 0.01);

    // Test traceability
    assert_eq!(complete_model::traceability::REQUIREMENT_COUNT, 1);
    assert_eq!(complete_model::traceability::REQUIREMENT_IDS[0], "REQ-COMP-001");
}

// Test guards, transition actions and enter/exit hooks
middleware_model_inline! {
    r#"
version: "1.0"
namespace: "test_hooks"
types:
  enums:
    - name: Mode
      repr: u8
      variants:
        - Idle
        - Armed
        - Flying
        - Locked
    - name: Cmd
      repr: u8
      variants:
        - Arm
        - Launch
        - Land
        - Ping
state_machines:
  - name: Flight
    state_type: Mode
    initial_state: Idle
    events: Cmd
    states:
      - name: Idle
        on_exit: leave_idle
      - name: Armed
        on_enter: enter_armed
        on_exit: leave_armed
      - name: Flying
        on_enter: enter_flying
      - name: Locked
        on_enter: enter_locked
    transitions:
      - from: Idle
        event: Arm
        to: Armed
        guard: preflight_ok
        action: spin_up
      - from: Idle
        event: Arm
        to: Locked
        guard: lockout
      - from: Armed
        event: Launch
        to: Flying
        action: throttle_up
      - from: Flying
        event: Land
        to: Idle
"#
}

/// Hook calls recorded by [`Recorder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    LeaveIdle,
    EnterArmed,
    LeaveArmed,
    EnterFlying,
    EnterLocked,
    SpinUp,
    ThrottleUp,
}

/// Test double for the generated `Flight::Hooks` trait.
///
/// Guards are configurable; actions append to a fixed-size log and fail on
/// the configured call.
struct Recorder {
    preflight_ok: bool,
    lockout: bool,
    fail_on: Option<Call>,
    log: [Option<Call>; 8],
    len: usize,
}

impl Recorder {
    fn new(preflight_ok: bool, lockout: bool) -> Self {
        Self { preflight_ok, lockout, fail_on: None, log: [None; 8], len: 0 }
    }

    fn record(&mut self, call: Call) -> Result<(), &'static str> {
        if self.fail_on == Some(call) {
            return Err("hook failed");
        }
        self.log[self.len] = Some(call);
        self.len += 1;
        Ok(())
    }

    fn calls(&self) -> Vec<Call> {
        self.log[..self.len].iter().flatten().copied().collect()
    }
}

impl test_hooks::Flight::Hooks for Recorder {
    type Error = &'static str;

    fn preflight_ok(&self) -> bool {
        self.preflight_ok
    }
    fn lockout(&self) -> bool {
        self.lockout
    }
    fn leave_idle(&mut self) -> Result<(), Self::Error> {
        self.record(Call::LeaveIdle)
    }
    fn enter_armed(&mut self) -> Result<(), Self::Error> {
        self.record(Call::EnterArmed)
    }
    fn leave_armed(&mut self) -> Result<(), Self::Error> {
        self.record(Call::LeaveArmed)
    }
    fn enter_flying(&mut self) -> Result<(), Self::Error> {
        self.record(Call::EnterFlying)
    }
    fn enter_locked(&mut self) -> Result<(), Self::Error> {
        self.record(Call::EnterLocked)
    }
    fn spin_up(&mut self) -> Result<(), Self::Error> {
        self.record(Call::SpinUp)
    }
    fn throttle_up(&mut self) -> Result<(), Self::Error> {
        self.record(Call::ThrottleUp)
    }
}

/// REQ-FSM-011: a false guard blocks the transition and runs no actions.
#[test]
fn test_fsm_guard_false_blocks_transition() {
    use test_hooks::{Cmd, Flight, Mode};

    let mut hooks = Recorder::new(false, false);
    let (state, outcome) = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap();
    assert_eq!(state, Mode::Idle);
    assert_eq!(outcome, Flight::Outcome::Remained);
    assert!(hooks.calls().is_empty());
}

/// REQ-FSM-011: rows are tried in order; the first passing guard wins.
#[test]
fn test_fsm_guard_selects_first_passing_row() {
    use test_hooks::{Cmd, Flight, Mode};

    let mut hooks = Recorder::new(false, true);
    let (state, outcome) = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap();
    assert_eq!(state, Mode::Locked);
    assert_eq!(outcome, Flight::Outcome::Transitioned { from: Mode::Idle, to: Mode::Locked });
    assert_eq!(hooks.calls(), vec![Call::LeaveIdle, Call::EnterLocked]);

    let mut hooks = Recorder::new(true, true);
    let (state, _) = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap();
    assert_eq!(state, Mode::Armed);
}

/// REQ-FSM-002, REQ-FSM-012: exit, transition action, then enter, in order.
#[test]
fn test_fsm_actions_run_exit_action_enter_in_order() {
    use test_hooks::{Cmd, Flight, Mode};

    let mut hooks = Recorder::new(true, false);
    let (state, outcome) = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap();
    assert_eq!(state, Mode::Armed);
    assert_eq!(outcome, Flight::Outcome::Transitioned { from: Mode::Idle, to: Mode::Armed });
    assert_eq!(hooks.calls(), vec![Call::LeaveIdle, Call::SpinUp, Call::EnterArmed]);

    let (state, _) = Flight::step(state, Cmd::Launch, &mut hooks).unwrap();
    assert_eq!(state, Mode::Flying);
    assert_eq!(
        hooks.calls(),
        vec![
            Call::LeaveIdle,
            Call::SpinUp,
            Call::EnterArmed,
            Call::LeaveArmed,
            Call::ThrottleUp,
            Call::EnterFlying,
        ]
    );

    // Flying has no exit hook, the row no action and Idle no enter hook
    let (state, _) = Flight::step(state, Cmd::Land, &mut hooks).unwrap();
    assert_eq!(state, Mode::Idle);
    assert_eq!(hooks.len, 6);
}

/// Events with no matching row are ignored and run no hooks.
#[test]
fn test_fsm_unmatched_event_is_ignored() {
    use test_hooks::{Cmd, Flight, Mode};

    let mut hooks = Recorder::new(true, true);
    let (state, outcome) = Flight::step(Mode::Idle, Cmd::Ping, &mut hooks).unwrap();
    assert_eq!(state, Mode::Idle);
    assert_eq!(outcome, Flight::Outcome::Ignored);
    assert!(hooks.calls().is_empty());
}

/// A failing hook stops the transition and reports which phase failed.
#[test]
fn test_fsm_hook_failure_reports_phase() {
    use test_hooks::{Cmd, Flight, Mode};

    let mut hooks = Recorder::new(true, false);
    hooks.fail_on = Some(Call::SpinUp);
    let err = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap_err();
    assert_eq!(err, Flight::StepError::ActionFailed("hook failed"));
    assert_eq!(hooks.calls(), vec![Call::LeaveIdle]);

    let mut hooks = Recorder::new(true, false);
    hooks.fail_on = Some(Call::LeaveIdle);
    let err = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap_err();
    assert_eq!(err, Flight::StepError::ExitFailed("hook failed"));

    let mut hooks = Recorder::new(true, false);
    hooks.fail_on = Some(Call::EnterArmed);
    let err = Flight::step(Mode::Idle, Cmd::Arm, &mut hooks).unwrap_err();
    assert_eq!(err, Flight::StepError::EntryFailed("hook failed"));
}

/// REQ-FSM-013: guards and actions are recorded in the const table.
#[test]
fn test_fsm_table_carries_guards_and_actions() {
    use test_hooks::Flight;

    const FIRST: Flight::Transition = Flight::TRANSITIONS[0];
    assert_eq!(FIRST.guard, Some(Flight::Guard::PreflightOk));
    assert_eq!(FIRST.action, Some(Flight::Action::SpinUp));
    assert_eq!(Flight::TRANSITIONS[1].action, None);
    assert_eq!(Flight::TRANSITIONS[3].guard, None);
}
