// Pass case: the example model expands, and its generated table and lookups
// match the YAML. The path is relative to this file.
middleware_codegen::middleware_model!("../../../../examples/attitude_controller.yaml");

use attitude_control::{ControllerStateMachine as Fsm, SystemEvent, SystemState};

fn main() {
    assert_eq!(Fsm::INITIAL_STATE, SystemState::Initializing);
    assert_eq!(Fsm::TRANSITION_COUNT, 8);
    let row = Fsm::find_transition(SystemState::Fault, SystemEvent::FaultCleared);
    assert_eq!(row.map(|r| r.to), Some(SystemState::Ready));
    assert_eq!(row.and_then(|r| r.guard), Some(Fsm::Guard::FaultResolved));
    assert_eq!(Fsm::on_exit(SystemState::Active), Some(Fsm::Action::DisableControl));
    assert_eq!(attitude_control::YAW_PID::KD, 0.5);
}
