//! Behavioural tests for code generated from `examples/attitude_controller.yaml`.
//!
//! The example model is expanded with `middleware_model!` and the generated
//! component, ports, port messages, state machine and PID constants are
//! exercised against the runtime traits of this crate.
//!
//! # Traceability
//! - REQ-GEN-011: Generated structs, traits and implementations for components
//! - REQ-FSM-002: Generated on_enter/on_exit actions
//! - REQ-FSM-011: Generated guard conditions
//! - REQ-FSM-012: Generated transition actions

// float_cmp: only generated constants are compared with the YAML literals they
// were emitted from, which are exact by construction.
// test code: a panic is a test failure, which is the intended reporting mechanism
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::float_cmp))]

use core::cell::{Cell, RefCell};

use middleware::port::{InputPort, OutputPort};
use middleware::{Component, PortDirection, PortMessage, SendError};

// The path is relative to this file, like `include_str!`.
middleware_codegen::middleware_model!("../../examples/attitude_controller.yaml");

use attitude_control::{
    AttitudeController, AttitudeControllerCommand, AttitudeControllerConfig,
    AttitudeControllerPorts, AttitudeMeasurement, AttitudeSetpoint, ControlOutput,
    ControllerStateMachine as Fsm, SystemEvent, SystemState, TelemetryData,
};

/// Hook calls recorded by [`Hooks`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    InitializeSensors,
    PrepareControlLoop,
    EnableControl,
    DisableControl,
    LogFault,
    CleanupResources,
    ArmActuators,
    DisarmActuators,
}

/// Test implementation of the generated `ControllerStateMachine::Hooks`.
#[derive(Default)]
struct Hooks {
    calibrated: bool,
    fault_resolved: bool,
    fail_on: Option<Call>,
    calls: Vec<Call>,
}

impl Hooks {
    fn record(&mut self, call: Call) -> Result<(), Call> {
        if self.fail_on == Some(call) {
            return Err(call);
        }
        self.calls.push(call);
        Ok(())
    }
}

impl Fsm::Hooks for Hooks {
    type Error = Call;

    fn sensors_calibrated(&self) -> bool {
        self.calibrated
    }
    fn fault_resolved(&self) -> bool {
        self.fault_resolved
    }
    fn initialize_sensors(&mut self) -> Result<(), Call> {
        self.record(Call::InitializeSensors)
    }
    fn prepare_control_loop(&mut self) -> Result<(), Call> {
        self.record(Call::PrepareControlLoop)
    }
    fn enable_control(&mut self) -> Result<(), Call> {
        self.record(Call::EnableControl)
    }
    fn disable_control(&mut self) -> Result<(), Call> {
        self.record(Call::DisableControl)
    }
    fn log_fault(&mut self) -> Result<(), Call> {
        self.record(Call::LogFault)
    }
    fn cleanup_resources(&mut self) -> Result<(), Call> {
        self.record(Call::CleanupResources)
    }
    fn arm_actuators(&mut self) -> Result<(), Call> {
        self.record(Call::ArmActuators)
    }
    fn disarm_actuators(&mut self) -> Result<(), Call> {
        self.record(Call::DisarmActuators)
    }
}

/// Input port test double holding at most one pending message.
///
/// `Default` is a disconnected-at-start handle, as `Component::new` requires.
struct TestInput<M> {
    pending: Cell<Option<M>>,
}

impl<M> core::fmt::Debug for TestInput<M> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("TestInput")
    }
}

impl<M> Default for TestInput<M> {
    fn default() -> Self {
        Self { pending: Cell::new(None) }
    }
}

impl<M: PortMessage + Copy> TestInput<M> {
    fn take(&self) -> Option<M> {
        self.pending.take()
    }
}

impl<M: PortMessage> InputPort<M> for TestInput<M> {
    fn name(&self) -> &'static str {
        "test_input"
    }
    fn is_connected(&self) -> bool {
        true
    }
}

/// Output port test double that serializes every sent message.
#[derive(Debug, Default)]
struct TestOutput {
    last: RefCell<Vec<u8>>,
}

impl<M: PortMessage> OutputPort<M> for TestOutput {
    fn name(&self) -> &'static str {
        "test_output"
    }
    fn is_connected(&self) -> bool {
        true
    }
    fn send(&self, message: M) -> Result<(), SendError> {
        let mut buffer = [0u8; 64];
        let len = message.serialize(&mut buffer).map_err(|_| SendError::SerializationFailed)?;
        *self.last.borrow_mut() = buffer[..len].to_vec();
        Ok(())
    }
}

type Controller = AttitudeController<
    TestInput<AttitudeMeasurement>,
    TestInput<AttitudeSetpoint>,
    TestOutput,
    TestOutput,
>;

fn ready_controller(hooks: &mut Hooks) -> Controller {
    let controller = Controller::new(AttitudeControllerConfig::default()).unwrap();
    let (controller, _) = controller.handle(SystemEvent::Initialize, hooks).unwrap();
    assert_eq!(controller.state().current, SystemState::Ready);
    controller
}

/// REQ-GEN-011: a component struct with typed ports, state and commands exists
/// and implements the runtime `Component` trait.
#[test]
fn test_example_component_struct_with_ports() {
    assert_eq!(Controller::NAME, "AttitudeController");
    assert_eq!(Controller::ID, 1);
    let is_active = Controller::IS_ACTIVE;
    assert!(is_active, "model declares kind: active");
    assert_eq!(Controller::PORT_COUNT, 4);
    assert_eq!(
        Controller::PORT_NAMES,
        ["attitude_input", "setpoint_input", "control_output", "telemetry_output"]
    );
    assert_eq!(
        Controller::PORT_DIRECTIONS,
        [PortDirection::Input, PortDirection::Input, PortDirection::Output, PortDirection::Output]
    );
    assert_eq!(Controller::PORT_MESSAGE_TYPES[3], "TelemetryData");
    assert_eq!(Controller::TRACES, ["REQ-ATT-001", "REQ-ATT-002"]);

    assert_eq!(AttitudeControllerCommand::COUNT, 3);
    assert_eq!(
        AttitudeControllerCommand::ALL,
        [
            AttitudeControllerCommand::Arm,
            AttitudeControllerCommand::Disarm,
            AttitudeControllerCommand::ResetFault
        ]
    );

    let ports = AttitudeControllerPorts {
        attitude_input: TestInput::default(),
        setpoint_input: TestInput::default(),
        control_output: TestOutput::default(),
        telemetry_output: TestOutput::default(),
    };
    let measurement = AttitudeMeasurement { roll: 0.1, pitch: -0.2, yaw: 0.3, timestamp_ms: 42 };
    ports.attitude_input.pending.set(Some(measurement));

    let controller = Controller::with_ports(AttitudeControllerConfig::default(), ports);
    assert_eq!(controller.state().current, Fsm::INITIAL_STATE);
    assert_eq!(controller.ports().attitude_input.take(), Some(measurement));

    let output = ControlOutput { roll_output: 0.5, pitch_output: -0.5, yaw_output: 0.0 };
    controller.ports().control_output.send(output).unwrap();
    let bytes = controller.ports().control_output.last.borrow().clone();
    assert_eq!(bytes.len(), ControlOutput::WIRE_SIZE);
    assert_eq!(ControlOutput::deserialize(&bytes).unwrap(), output);
}

/// REQ-GEN-011: generated `PortMessage` impls round-trip and reject bad input.
#[test]
fn test_example_port_message_roundtrip() {
    let telemetry = TelemetryData { mode: 2, state: 3, pitch_error: 0.125, timestamp_ms: 9_000 };
    let mut buffer = [0u8; 32];
    let len = telemetry.serialize(&mut buffer).unwrap();
    assert_eq!(len, TelemetryData::WIRE_SIZE);
    assert_eq!(len, 1 + 1 + 4 + 8);
    assert_eq!(TelemetryData::max_size(), len);
    assert_eq!(TelemetryData::deserialize(&buffer[..len]).unwrap(), telemetry);

    let mut small = [0u8; 4];
    assert_eq!(
        telemetry.serialize(&mut small),
        Err(middleware::SerializationError::BufferTooSmall)
    );
    assert_eq!(
        TelemetryData::deserialize(&buffer[..len - 1]),
        Err(middleware::SerializationError::InvalidFormat)
    );
}

/// REQ-FSM-011: with `sensors_calibrated` false, Start leaves the component in Ready.
#[test]
fn test_example_guard_false_blocks_start() {
    let mut hooks = Hooks::default();
    let controller = ready_controller(&mut hooks);
    hooks.calls.clear();

    let (controller, outcome) = controller.handle(SystemEvent::Start, &mut hooks).unwrap();
    assert_eq!(outcome, Fsm::Outcome::Remained);
    assert_eq!(controller.state().current, SystemState::Ready);
    assert!(hooks.calls.is_empty());

    hooks.calibrated = true;
    let (controller, outcome) = controller.handle(SystemEvent::Start, &mut hooks).unwrap();
    assert_eq!(
        outcome,
        Fsm::Outcome::Transitioned { from: SystemState::Ready, to: SystemState::Active }
    );
    assert_eq!(controller.state().current, SystemState::Active);
}

/// REQ-FSM-002, REQ-FSM-012: exit, transition action and enter run in order.
#[test]
fn test_example_actions_enter_exit_order() {
    let mut hooks = Hooks { calibrated: true, ..Hooks::default() };
    let controller = ready_controller(&mut hooks);
    assert_eq!(hooks.calls, vec![Call::PrepareControlLoop]);
    hooks.calls.clear();

    let (controller, _) = controller.handle(SystemEvent::Start, &mut hooks).unwrap();
    assert_eq!(hooks.calls, vec![Call::ArmActuators, Call::EnableControl]);
    hooks.calls.clear();

    let (controller, _) = controller.handle(SystemEvent::Stop, &mut hooks).unwrap();
    assert_eq!(
        hooks.calls,
        vec![Call::DisableControl, Call::DisarmActuators, Call::PrepareControlLoop]
    );
    assert_eq!(controller.state().current, SystemState::Ready);
    assert!(!hooks.calls.contains(&Call::InitializeSensors));
}

/// REQ-FSM-011: Fault is left only once `fault_resolved` holds.
#[test]
fn test_example_fault_recovery_guard() {
    let mut hooks = Hooks { calibrated: true, ..Hooks::default() };
    let controller = ready_controller(&mut hooks);
    let (controller, _) = controller.handle(SystemEvent::Start, &mut hooks).unwrap();
    let (controller, _) = controller.handle(SystemEvent::FaultDetected, &mut hooks).unwrap();
    assert_eq!(controller.state().current, SystemState::Fault);
    assert_eq!(hooks.calls.last(), Some(&Call::LogFault));

    let (controller, outcome) = controller.handle(SystemEvent::FaultCleared, &mut hooks).unwrap();
    assert_eq!(outcome, Fsm::Outcome::Remained);
    assert_eq!(controller.state().current, SystemState::Fault);

    hooks.fault_resolved = true;
    let (controller, _) = controller.handle(SystemEvent::FaultCleared, &mut hooks).unwrap();
    assert_eq!(controller.state().current, SystemState::Ready);
}

/// A failing hook returns the component unchanged together with the error.
#[test]
fn test_example_hook_failure_keeps_component_state() {
    let mut hooks = Hooks { calibrated: true, fail_on: Some(Call::ArmActuators), ..Hooks::default() };
    let controller = ready_controller(&mut hooks);

    let (controller, error) = controller.handle(SystemEvent::Start, &mut hooks).unwrap_err();
    assert_eq!(error, Fsm::StepError::ActionFailed(Call::ArmActuators));
    assert_eq!(controller.state().current, SystemState::Ready);
}

/// Events without a matching transition are ignored.
#[test]
fn test_example_unmatched_event_ignored() {
    let mut hooks = Hooks::default();
    let controller = ready_controller(&mut hooks);
    let (controller, outcome) = controller.handle(SystemEvent::Stop, &mut hooks).unwrap();
    assert_eq!(outcome, Fsm::Outcome::Ignored);
    assert_eq!(controller.state().current, SystemState::Ready);
}

/// PID blocks carry both integral limits from the model.
#[test]
fn test_example_pid_integral_limits() {
    assert_eq!(attitude_control::PITCH_PID::INTEGRAL_MIN, -0.5);
    assert_eq!(attitude_control::PITCH_PID::INTEGRAL_MAX, 0.5);
    assert_eq!(attitude_control::YAW_PID::KP, 2.0);
}
