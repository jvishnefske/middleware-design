# Model-Based Development Framework - Functional Requirements

This document defines the functional requirements for the middleware framework.
Requirements are tracked with REQ-xxx identifiers for DO-178C DAL-C traceability.

## 1. Type System Requirements

### 1.1 Physical Units
- [x] **REQ-TYP-001**: System shall provide type-safe physical unit wrappers (Seconds, Radians, MetersPerSecond, etc.)
- [x] **REQ-TYP-002**: Unit types shall be zero-cost abstractions using `#[repr(transparent)]`
- [x] **REQ-TYP-003**: Arithmetic operations shall enforce dimensional correctness at compile time
- [x] **REQ-TYP-004**: All unit types shall implement Copy, Clone, Debug, PartialEq, PartialOrd

### 1.2 Primitive Wrappers
- [x] **REQ-TYP-010**: System shall provide newtype wrappers for primitives with unit parameters
- [x] **REQ-TYP-011**: Supported primitive types: i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool
- [x] **REQ-TYP-012**: Primitive wrappers shall support From/Into conversions

### 1.3 Bounded Collections
- [x] **REQ-TYP-020**: System shall provide BoundedVec<T, N> for fixed-capacity vectors without alloc
- [x] **REQ-TYP-021**: System shall provide BoundedString<N> for fixed-capacity strings without alloc
- [x] **REQ-TYP-022**: Bounded collections shall return Result on capacity overflow

## 2. Component Model Requirements

### 2.1 Base Component
- [x] **REQ-CMP-001**: Components shall have associated Config, State, and Ports types
- [x] **REQ-CMP-002**: Components shall have static NAME and ID identifiers
- [x] **REQ-CMP-003**: Component construction shall return Result<Self, InitError>

### 2.2 Passive Components
- [x] **REQ-CMP-010**: Passive components shall be invoked synchronously
- [x] **REQ-CMP-011**: Passive component invoke() shall return Result<Response, InvokeError>

### 2.3 Active Components
- [x] **REQ-CMP-020**: Active components shall have an associated Schedule type
- [x] **REQ-CMP-021**: Active components shall implement run_cycle() returning Result<CycleResult, CycleError>
- [x] **REQ-CMP-022**: Schedule shall specify rate (Duration) and priority

### 2.4 Lifecycle
- [x] **REQ-CMP-030**: Components shall support lifecycle states: Uninitialized, Initializing, Ready, Running, Paused, ShuttingDown, Terminated
- [x] **REQ-CMP-031**: Lifecycle trait shall provide on_init, on_start, on_pause, on_resume, on_shutdown hooks

## 3. Port Requirements

### 3.1 Port Messages
- [x] **REQ-PRT-001**: Port messages shall implement serialize/deserialize to/from byte buffers
- [x] **REQ-PRT-002**: Serialization errors shall be returned as Result

### 3.2 Input Ports
- [x] **REQ-PRT-010**: Input ports shall be parameterized by message type
- [x] **REQ-PRT-011**: Sync input ports shall support blocking recv with timeout
- [x] **REQ-PRT-012**: Async input ports shall support non-blocking poll

### 3.3 Output Ports
- [x] **REQ-PRT-020**: Output ports shall be parameterized by message type
- [x] **REQ-PRT-021**: Output ports shall return Result<(), SendError> on send

## 4. State Machine Requirements

### 4.1 States and Events
- [x] **REQ-FSM-001**: States shall be represented as enums with #[repr(u8)] or #[repr(u16)]
- [x] **REQ-FSM-002**: States shall support on_enter and on_exit actions (generated FSMs: test_fsm_actions_run_exit_action_enter_in_order, test_fsm_hook_failure_reports_phase in middleware-codegen/tests/codegen_integration.rs; test_example_actions_enter_exit_order in middleware/tests/attitude_controller_model.rs)
- [x] **REQ-FSM-003**: Events shall be represented as enums with optional associated data

### 4.2 Transitions
- [x] **REQ-FSM-010**: Transitions shall specify source state, event, target state
- [x] **REQ-FSM-011**: Transitions shall support optional guard conditions (generated FSMs: test_fsm_guard_false_blocks_transition, test_fsm_guard_selects_first_passing_row in middleware-codegen/tests/codegen_integration.rs; test_example_guard_false_blocks_start, test_example_fault_recovery_guard in middleware/tests/attitude_controller_model.rs)
- [x] **REQ-FSM-012**: Transitions shall support optional actions (generated FSMs: test_fsm_actions_run_exit_action_enter_in_order, test_fsm_table_carries_guards_and_actions in middleware-codegen/tests/codegen_integration.rs; test_example_actions_enter_exit_order in middleware/tests/attitude_controller_model.rs)
- [x] **REQ-FSM-013**: Transition tables shall be compile-time constant arrays

### 4.3 State Machine Container
- [x] **REQ-FSM-020**: StateMachine shall be parameterized by State and Event types
- [x] **REQ-FSM-021**: StateMachine.process(event) shall return Result<TransitionResult, FsmError>
- [x] **REQ-FSM-022**: TransitionResult shall indicate Transitioned, Remained, or Ignored
- [x] **REQ-FSM-023**: State machine processing shall be immutable (return new instance)

### 4.4 Guards
- [x] **REQ-FSM-030**: System shall provide ThresholdGuard for control-based transitions
- [x] **REQ-FSM-031**: System shall provide TimedGuard for duration-based transitions

## 5. Control Block Requirements

### 5.1 Control Block Trait
- [x] **REQ-CTL-001**: Control blocks shall implement update(self, input, dt) -> Result<(Self, Output), Error>
- [x] **REQ-CTL-002**: Control blocks shall implement reset(self) -> Self
- [x] **REQ-CTL-003**: Control block state shall be immutable (updates return new instances)

### 5.2 PID Controller
- [x] **REQ-CTL-010**: PID shall support configurable Kp, Ki, Kd gains
- [x] **REQ-CTL-011**: PID shall implement anti-windup via integral clamping
- [x] **REQ-CTL-012**: PID shall support output saturation limits
- [x] **REQ-CTL-013**: PID shall use derivative-on-measurement to avoid setpoint kick (verified by middleware/src/control/pid.rs tests: test_pid_derivative_on_measurement, test_pid_first_update_has_no_derivative_kick, test_pid_reset)
- [x] **REQ-CTL-014**: PID shall be constructable via Builder pattern

### 5.3 Digital Filters
- [x] **REQ-CTL-020**: System shall provide first-order low-pass filter
- [x] **REQ-CTL-021**: System shall provide first-order high-pass filter
- [x] **REQ-CTL-022**: System shall provide second-order notch filter
- [x] **REQ-CTL-023**: Filter coefficients shall be computed from cutoff frequency

### 5.4 State-Space Models
- [x] **REQ-CTL-030**: System shall provide discrete-time state-space model
- [x] **REQ-CTL-031**: State-space shall support const generic dimensions (N states, M inputs, P outputs)
- [x] **REQ-CTL-032**: State-space shall use nalgebra for matrix operations

### 5.5 Limiters
- [x] **REQ-CTL-040**: System shall provide RateLimiter with configurable rate limits
- [x] **REQ-CTL-041**: System shall provide Saturator with min/max bounds

## 6. Serialization Requirements

### 6.1 CBOR Encoding
- [x] **REQ-SER-001**: System shall use CBOR for binary serialization
- [x] **REQ-SER-002**: CBOR implementation shall be no_std compatible (minicbor)
- [x] **REQ-SER-003**: All message types shall derive Encode and Decode

### 6.2 Telemetry
- [x] **REQ-SER-010**: System shall provide ControlTelemetry message type
- [x] **REQ-SER-011**: System shall provide PidTelemetry message type
- [x] **REQ-SER-012**: System shall provide StateMachineTelemetry message type

### 6.3 Framing
- [x] **REQ-SER-020**: Telemetry frames shall include message type, sequence number, payload length
- [x] **REQ-SER-021**: System shall provide fixed-size TelemetryBuffer<N> for encoding

## 7. Code Generation Requirements

### 7.1 YAML Schema
- [x] **REQ-GEN-001**: YAML schema shall define component structure
- [x] **REQ-GEN-002**: YAML schema shall define state machine structure
- [x] **REQ-GEN-003**: YAML schema shall define math block configuration
- [x] **REQ-GEN-004**: All YAML elements shall support traces field for requirement IDs

### 7.2 Proc-Macro
- [x] **REQ-GEN-010**: middleware_model! macro shall read YAML at compile time
- [x] **REQ-GEN-011**: Macro shall generate Rust structs, traits, and implementations (test_example_component_struct_with_ports, test_example_port_message_roundtrip, test_example_hook_failure_keeps_component_state in middleware/tests/attitude_controller_model.rs; test_fsm_guard_false_blocks_transition in middleware-codegen/tests/codegen_integration.rs)
- [x] **REQ-GEN-012**: Macro shall validate YAML against schema and emit compile errors (verified by middleware-codegen/src/validator.rs tests: embedded_schema_compiles, attitude_controller_example_passes, missing_required_field_fails_with_path, missing_nested_required_field_fails_with_path, wrong_type_fails_with_path, unknown_enum_value_fails_with_path, all_violations_are_reported, invalid_yaml_is_reported)
- [x] **REQ-GEN-013**: Generated code shall include traceability comments and const arrays

## 8. Platform Requirements

### 8.1 no_std Compatibility
- [x] **REQ-PLT-001**: All library code shall compile with #![no_std]
- [x] **REQ-PLT-002**: No heap allocation (no alloc feature required)
- [x] **REQ-PLT-003**: All data structures shall use fixed-size stack allocation

### 8.2 Error Handling
- [x] **REQ-PLT-010**: All fallible operations shall return Result<T, E>
- [x] **REQ-PLT-011**: Error types shall be exhaustive enums (no catch-all variants)

### 8.3 Safety
- [x] **REQ-PLT-020**: No unsafe code shall be used in the library
- [x] **REQ-PLT-021**: All state transitions shall be immutable transformations

## Traceability Matrix

| Artifact | Requirements |
|----------|--------------|
| src/types/unit.rs | REQ-TYP-001, REQ-TYP-002, REQ-TYP-003, REQ-TYP-004 |
| src/types/primitive.rs | REQ-TYP-010, REQ-TYP-011, REQ-TYP-012 |
| src/types/bounded.rs | REQ-TYP-020, REQ-TYP-021, REQ-TYP-022 |
| src/component.rs | REQ-CMP-001, REQ-CMP-002, REQ-CMP-003 |
| src/component/passive.rs | REQ-CMP-010, REQ-CMP-011 |
| src/component/active.rs | REQ-CMP-020, REQ-CMP-021, REQ-CMP-022 |
| src/component/lifecycle.rs | REQ-CMP-030, REQ-CMP-031 |
| src/port/message.rs | REQ-PRT-001, REQ-PRT-002 |
| src/port/input.rs | REQ-PRT-010, REQ-PRT-011, REQ-PRT-012 |
| src/port/output.rs | REQ-PRT-020, REQ-PRT-021 |
| src/state_machine/state.rs | REQ-FSM-001, REQ-FSM-002 |
| src/state_machine/event.rs | REQ-FSM-003 |
| src/state_machine/transition.rs | REQ-FSM-010, REQ-FSM-011, REQ-FSM-012, REQ-FSM-013 |
| src/state_machine/machine.rs | REQ-FSM-020, REQ-FSM-021, REQ-FSM-022, REQ-FSM-023 |
| src/state_machine/guards.rs | REQ-FSM-030, REQ-FSM-031 |
| src/control/block.rs | REQ-CTL-001, REQ-CTL-002, REQ-CTL-003 |
| src/control/pid.rs | REQ-CTL-010, REQ-CTL-011, REQ-CTL-012, REQ-CTL-013, REQ-CTL-014 |
| src/control/filters.rs | REQ-CTL-020, REQ-CTL-021, REQ-CTL-022, REQ-CTL-023 |
| src/control/state_space.rs | REQ-CTL-030, REQ-CTL-031, REQ-CTL-032 |
| src/control/limiters.rs | REQ-CTL-040, REQ-CTL-041 |
| src/telemetry/messages.rs | REQ-SER-001, REQ-SER-002, REQ-SER-003, REQ-SER-010, REQ-SER-011, REQ-SER-012 |
| src/telemetry/framing.rs | REQ-SER-020 |
| src/telemetry/buffer.rs | REQ-SER-021 |
| middleware-codegen/src/lib.rs | REQ-GEN-010, REQ-GEN-011, REQ-GEN-012, REQ-GEN-013 |
| middleware-codegen/src/parser.rs | REQ-GEN-001, REQ-GEN-002, REQ-GEN-003, REQ-GEN-004 |
| middleware-codegen/src/validator.rs | REQ-GEN-012 |
| middleware-codegen/src/generator.rs | REQ-GEN-011, REQ-GEN-013 |
| schema/model.schema.json | REQ-GEN-001, REQ-GEN-002, REQ-GEN-003, REQ-GEN-004 |
| examples/attitude_controller.yaml | Example implementation |
| src/lib.rs | REQ-PLT-001, REQ-PLT-002, REQ-PLT-003 |
| src/error.rs | REQ-PLT-010, REQ-PLT-011 |
| (all modules) | REQ-PLT-020, REQ-PLT-021 |
