# Model-Based Development Framework - Functional Requirements

This document defines the software safety requirements for the middleware framework and the `middleware_model!` code generator, developed as an ISO 26262 Safety Element out of Context at ASIL B. Assumptions of use for integrators are in `docs/safety-manual.md`.

Each requirement has an identifier, an ASIL attribute (`[ASIL B]` or `[QM]`) and a status. A requirement is ticked only when `cargo xtask trace --check` confirms at least one test cites it with a `/// Verifies:` tag. The generated requirement-to-test matrix is `docs/traceability.md`; it is the citation source and must not be edited by hand.

Telemetry (section 6) is QM: it is diagnostic output and must not feed a safety mechanism. All other requirements are ASIL B.

## 1. Type System Requirements

### 1.1 Physical Units
- [x] **REQ-TYP-001** [ASIL B]: System shall provide type-safe physical unit wrappers (Seconds, Radians, MetersPerSecond, etc.)
- [x] **REQ-TYP-002** [ASIL B]: Unit-tagged quantities (Quantity<T, U>) shall be zero-cost: `#[repr(transparent)]` over T, with zero-sized unit markers
- [x] **REQ-TYP-003** [ASIL B]: Arithmetic and comparison shall compile only between quantities of the same unit; scaling shall be only by a scalar of the underlying type
- [x] **REQ-TYP-004** [ASIL B]: All unit types shall implement Copy, Clone, Debug, PartialEq, PartialOrd

### 1.2 Primitive Wrappers
- [x] **REQ-TYP-010** [ASIL B]: System shall provide newtype wrappers for primitives with unit parameters
- [x] **REQ-TYP-011** [ASIL B]: Supported primitive types: i8, i16, i32, i64, u8, u16, u32, u64, f32, f64, bool
- [x] **REQ-TYP-012** [ASIL B]: Primitive wrappers shall support From/Into from the primitive and lossless extraction via into_inner()

### 1.3 Bounded Collections
- [x] **REQ-TYP-020** [ASIL B]: System shall provide BoundedVec<T, N> for fixed-capacity vectors without alloc
- [x] **REQ-TYP-021** [ASIL B]: System shall provide BoundedString<N> for fixed-capacity strings without alloc
- [x] **REQ-TYP-022** [ASIL B]: Bounded collections shall return Result on capacity overflow

## 2. Component Model Requirements

### 2.1 Base Component
- [x] **REQ-CMP-001** [ASIL B]: Components shall have associated Config, State, and Ports types
- [x] **REQ-CMP-002** [ASIL B]: Components shall have static NAME and ID identifiers
- [x] **REQ-CMP-003** [ASIL B]: Component construction shall return Result<Self, InitError>

### 2.2 Passive Components
- [x] **REQ-CMP-010** [ASIL B]: Passive components shall be invoked synchronously
- [x] **REQ-CMP-011** [ASIL B]: Passive component invoke() shall return Result<Response, InvokeError>

### 2.3 Active Components
- [x] **REQ-CMP-020** [ASIL B]: Active components shall provide their Schedule via schedule()
- [x] **REQ-CMP-021** [ASIL B]: Active components shall implement run_cycle() returning Result<CycleResult, CycleError>
- [x] **REQ-CMP-022** [ASIL B]: Schedule shall specify rate (Duration) and priority
- [x] **REQ-CMP-023** [ASIL B]: Schedule construction from a frequency shall reject NaN, infinite, zero and negative frequencies, and frequencies above 1000 Hz whose period would truncate to 0 ms, with an error

### 2.4 Lifecycle
- [x] **REQ-CMP-030** [ASIL B]: Components shall support lifecycle states: Uninitialized, Initializing, Ready, Running, Paused, ShuttingDown, Terminated. Transitions shall be limited to Uninitialized→Initializing→Ready→Running, Running↔Paused, Running/Paused→ShuttingDown→Terminated; all other transitions shall return InvalidTransition
- [x] **REQ-CMP-031** [ASIL B]: Lifecycle trait shall provide on_init, on_start, on_pause, on_resume, on_shutdown hooks

## 3. Port Requirements

### 3.1 Port Messages
- [x] **REQ-PRT-001** [ASIL B]: Port messages shall implement serialize/deserialize to/from byte buffers
- [x] **REQ-PRT-002** [ASIL B]: Serialization errors shall be returned as Result

### 3.2 Input Ports
- [x] **REQ-PRT-010** [ASIL B]: Input ports shall be parameterized by message type
- [x] **REQ-PRT-011** [ASIL B]: Synchronous input ports shall provide recv(timeout) returning Result<T, RecvError> (interface contract; blocking and timeout timing are verified per port implementation)
- [x] **REQ-PRT-012** [ASIL B]: Async input ports shall support non-blocking poll

### 3.3 Output Ports
- [x] **REQ-PRT-020** [ASIL B]: Output ports shall be parameterized by message type
- [x] **REQ-PRT-021** [ASIL B]: Output ports shall return Result<(), SendError> on send

## 4. State Machine Requirements

### 4.1 States and Events
- [x] **REQ-FSM-001** [ASIL B]: States shall be Copy enums; generated states shall use #[repr(u8)] or #[repr(u16)]
- [x] **REQ-FSM-002** [ASIL B]: States shall support on_enter and on_exit actions
- [x] **REQ-FSM-003** [ASIL B]: Events shall be represented as enums with optional associated data

### 4.2 Transitions
- [x] **REQ-FSM-010** [ASIL B]: Transitions shall specify source state, event, target state
- [x] **REQ-FSM-011** [ASIL B]: Transitions shall support optional guard conditions
- [x] **REQ-FSM-012** [ASIL B]: Transitions shall support optional actions
- [x] **REQ-FSM-013** [ASIL B]: Transition tables shall be compile-time constant arrays

### 4.3 State Machine Container
- [x] **REQ-FSM-020** [ASIL B]: StateMachine shall be parameterized by State and Event types
- [x] **REQ-FSM-021** [ASIL B]: StateMachine.process(event) shall return Result<(StateMachine, TransitionResult), FsmError>
- [x] **REQ-FSM-022** [ASIL B]: TransitionResult shall indicate Transitioned, Remained, or Ignored
- [x] **REQ-FSM-023** [ASIL B]: State machine processing shall be immutable (return new instance)
- [x] **REQ-FSM-024** [ASIL B]: Row selection: the first row matching (state, event) whose guard is absent or true shall fire; if rows match but every guard is false the result shall be Remained; if no row matches it shall be Ignored. Generated and runtime machines shall agree
- [x] **REQ-FSM-025** [ASIL B]: A failing exit, transition or entry action shall be reported as a distinct FsmError variant (ExitFailed, ActionFailed, EntryFailed); the first failing hook determines the error and later hooks shall not run. On error the runtime StateMachine is consumed; a caller requiring its previous state shall retain a clone taken before process, which is unaffected by the failed step

### 4.4 Guards
- [x] **REQ-FSM-030** [ASIL B]: System shall provide ThresholdGuard for control-based transitions
- [x] **REQ-FSM-031** [ASIL B]: System shall provide TimedGuard for duration-based transitions
- [x] **REQ-FSM-032** [ASIL B]: TimedGuard shall reject a negative or non-finite time step with an error and leave its elapsed time unchanged

## 5. Control Block Requirements

### 5.1 Control Block Trait
- [x] **REQ-CTL-001** [ASIL B]: Control blocks shall implement update(self, input, dt) -> Result<(Self, Output), Error>
- [x] **REQ-CTL-002** [ASIL B]: Control blocks shall implement reset(self) -> Self, returning the block with its dynamic state set to zero and its configuration preserved
- [x] **REQ-CTL-003** [ASIL B]: Control block state shall be immutable (updates return new instances)
- [x] **REQ-CTL-004** [ASIL B]: Control blocks shall reject non-finite inputs and non-positive or non-finite time steps, and shall report non-finite results, with an error
- [x] **REQ-CTL-005** [ASIL B]: Control blocks shall reject invalid configuration (NaN or misordered limits, negative integral bound, filter frequencies outside (0, fs/2]) with InvalidConfig and shall not panic

### 5.2 PID Controller
- [x] **REQ-CTL-010** [ASIL B]: PID shall support configurable Kp, Ki, Kd gains
- [x] **REQ-CTL-011** [ASIL B]: PID shall implement anti-windup via integral clamping
- [x] **REQ-CTL-012** [ASIL B]: PID shall support output saturation limits
- [x] **REQ-CTL-013** [ASIL B]: PID shall use derivative-on-measurement to avoid setpoint kick
- [x] **REQ-CTL-014** [ASIL B]: PID shall be constructable via Builder pattern

### 5.3 Digital Filters
- [x] **REQ-CTL-020** [ASIL B]: System shall provide first-order low-pass filter
- [x] **REQ-CTL-021** [ASIL B]: System shall provide first-order high-pass filter
- [x] **REQ-CTL-022** [ASIL B]: System shall provide second-order notch filter
- [x] **REQ-CTL-023** [ASIL B]: Filter coefficients shall be computed from cutoff frequency for 0 < f ≤ fs/2; other values shall be rejected

### 5.4 State-Space Models
- [x] **REQ-CTL-030** [ASIL B]: System shall provide discrete-time state-space model
- [x] **REQ-CTL-031** [ASIL B]: State-space shall support const generic dimensions (N states, M inputs, P outputs)
- [x] **REQ-CTL-032** [ASIL B]: State-space shall use nalgebra for matrix operations

### 5.5 Limiters
- [x] **REQ-CTL-040** [ASIL B]: System shall provide RateLimiter with configurable rate limits
- [x] **REQ-CTL-041** [ASIL B]: System shall provide Saturator with min/max bounds

## 6. Serialization Requirements

### 6.1 CBOR Encoding
- [x] **REQ-SER-001** [QM]: System shall use CBOR for binary serialization
- [x] **REQ-SER-002** [QM]: CBOR implementation shall be no_std compatible (minicbor)
- [x] **REQ-SER-003** [QM]: All message types shall derive Encode and Decode

### 6.2 Telemetry
- [x] **REQ-SER-010** [QM]: System shall provide ControlTelemetry message type
- [x] **REQ-SER-011** [QM]: System shall provide PidTelemetry message type
- [x] **REQ-SER-012** [QM]: System shall provide StateMachineTelemetry message type

### 6.3 Framing
- [x] **REQ-SER-020** [QM]: System shall provide a FrameHeader containing message type, sequence number (u16) and payload length (u16), CBOR-encoded ahead of the payload
- [x] **REQ-SER-021** [QM]: System shall provide fixed-size TelemetryBuffer<N> for encoding
- [x] **REQ-SER-022** [QM]: Encoding into a TelemetryBuffer that is too small shall return BufferOverflow, after which len() shall be 0 and as_bytes() empty

## 7. Code Generation Requirements

### 7.1 YAML Schema
- [x] **REQ-GEN-001** [ASIL B]: YAML schema shall define component structure
- [x] **REQ-GEN-002** [ASIL B]: YAML schema shall define state machine structure
- [x] **REQ-GEN-003** [ASIL B]: YAML schema shall define math block configuration
- [x] **REQ-GEN-004** [ASIL B]: Enum, struct, message, component, math block and state machine definitions shall support a traces field of requirement IDs, emitted as a Traces: doc comment on the generated item and, for components, as a TRACES const

### 7.2 Proc-Macro
- [x] **REQ-GEN-010** [ASIL B]: middleware_model! macro shall read YAML at compile time
- [x] **REQ-GEN-011** [ASIL B]: Macro shall generate Rust structs, traits, and implementations
- [x] **REQ-GEN-012** [ASIL B]: Macro shall validate YAML against schema and emit compile errors
- [x] **REQ-GEN-013** [ASIL B]: Generated code shall include traceability comments and const arrays
- [x] **REQ-GEN-014** [ASIL B]: The schema shall reject unknown keys in every object
- [x] **REQ-GEN-015** [ASIL B]: Model paths shall resolve relative to the invoking source file, and the generated code shall rebuild when the model file changes
- [x] **REQ-GEN-016** [ASIL B]: Generated state machines shall behave identically to the runtime StateMachine on the same transition table (back-to-back)
- [x] **REQ-GEN-017** [ASIL B]: Semantic inconsistencies the schema cannot express (unknown references, duplicates, name collisions, port size and encoding limits, inverted PID limits) shall be compile errors

## 8. Platform Requirements

### 8.1 no_std Compatibility
- [x] **REQ-PLT-001** [ASIL B]: All library code shall compile with #![no_std]
- [x] **REQ-PLT-002** [ASIL B]: No heap allocation (no alloc feature required)
- [x] **REQ-PLT-003** [ASIL B]: All data structures shall use fixed-size, compile-time-bounded storage (stack or static)

### 8.2 Error Handling
- [x] **REQ-PLT-010** [ASIL B]: All fallible operations shall return Result<T, E>
- [x] **REQ-PLT-011** [ASIL B]: Error types shall be exhaustive enums (no catch-all variants)

### 8.3 Safety
- [x] **REQ-PLT-020** [ASIL B]: No unsafe code shall be used in the library
- [x] **REQ-PLT-021** [ASIL B]: Control-block and state-machine state transitions shall be by-value transformations returning the new instance
- [x] **REQ-PLT-022** [ASIL B]: Every workspace crate shall enforce the coding-guideline lint set (unsafe_code forbidden; unwrap, expect, panic, indexing, float equality and lossy casts denied in non-test code)
- [x] **REQ-PLT-023** [ASIL B]: Generated code for the example model shall build for thumbv6m-none-eabi without std or alloc

## Traceability

The requirement-to-test matrix is generated by `cargo xtask trace` into `docs/traceability.md`, from this file and from `/// Verifies:` tags on tests. `cargo xtask trace --check` fails if a ticked requirement has no verifying test, a test cites an unknown requirement, a requirement lacks an ASIL tag, or the generated matrix is stale.

## Known deviations

- **Floating point on an FPU-less target.** Control blocks use `f32` although the reference target (Cortex-M0+) has no FPU, which conflicts with the workspace rule against floating point on the M0+. Arithmetic is software-emulated, so execution time depends on operand values and must be measured on target by the integrator (safety manual, section 2). Non-finite values are detected and reported (REQ-CTL-004). Conversion to fixed point is planned as separate work.
- **MC/DC not measured.** ISO 26262-6 Table 12 highly recommends MC/DC only at ASIL D; at ASIL B branch coverage is the highly recommended metric, and that is what `cargo xtask coverage` measures.
- **Branch coverage needs a nightly compiler.** The coverage gate runs on the pinned `nightly-2025-09-08`; the shipped code is built with stable. The pin is a tool-confidence item in `docs/tool-qualification.md`.
- **Proc-macro expansion is not instrumented.** Code in `middleware-codegen` that only runs inside rustc during macro expansion does not appear in coverage data. Its behaviour is verified by unit tests of the pure functions it calls, the compile-fail suite and back-to-back tests; see `docs/coverage-exclusions.md`.
- **Integer quantity overflow.** `Quantity` arithmetic on integer primitives follows the primitive: it panics on overflow in debug builds and wraps in release builds. Integrators using integer quantities on safety paths must bound their ranges (safety manual).
- **Asynchronous input ports cannot report faults.** `AsyncInputPort::poll` returns `Option`, so disconnection or a decode failure is indistinguishable from "no message". Integrators must supervise message freshness (safety manual).
- **Style lints in generated code.** Generated module names and the component `handle` signature trigger the style lints `non_snake_case` and `type_complexity`; none of the denied coding-guideline lints fire (`docs/tool-qualification.md`).

## ISO 26262-6 evidence map

| Clause / table | Topic | Evidence |
|---|---|---|
| 6-5, Table 1 | Coding guidelines | `[workspace.lints]` in `Cargo.toml` (`unsafe_code` forbidden; unwrap, expect, panic, indexing, float equality, `as` casts denied), enforced for every member (REQ-PLT-020, REQ-PLT-022) |
| 6-6 | Software safety requirements | This file: identified, ASIL-tagged, verifiable requirements |
| 6-7, Table 3 | Architectural design principles | Functional core and imperative shell, by-value state transitions, no heap, bounded loops (REQ-PLT-001..003, REQ-PLT-021); hierarchical module structure with rustdoc per module |
| 6-8, Table 6 | Unit design principles | No dynamic objects, no recursion, no implicit conversions, typed errors (REQ-PLT-010, REQ-PLT-011), unit types (REQ-TYP-*) |
| 6-9, Table 7 | Unit verification methods | Requirements-based tests, fault injection (hook failures, port faults, non-finite inputs, buffer overflow), back-to-back tests of generated against runtime state machines (REQ-GEN-016) |
| 6-9, Table 8 | Test case derivation | Equivalence classes and boundary values in each module's tests (see `docs/traceability.md`) |
| 6-9, Table 9 | Structural coverage at unit level | `cargo xtask coverage` (branch coverage gate); justified gaps in `docs/coverage-exclusions.md` |
| 6-10, 6-11 | Integration and embedded-software tests | Integration tests in `middleware/tests/`; `no_std` build for `thumbv6m-none-eabi` (REQ-PLT-023). Target hardware tests are the integrator's |
| 8-11 | Confidence in software tools | `docs/tool-qualification.md` (code generator TCL2 under the safety-manual assumption, compile-fail suite, back-to-back tests) |
| 10-9 | Safety element out of context | `docs/safety-manual.md` (assumptions of use) |

Outside this repository: hazard analysis and risk assessment, safety plan, safety case, confirmation reviews and the functional safety assessment.
