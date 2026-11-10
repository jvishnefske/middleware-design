# Safety Manual: middleware and middleware-codegen

This manual states the assumptions of use under which the middleware runtime crate and the `middleware_model!` code generator are developed as an ISO 26262 Safety Element out of Context (SEooC, ISO 26262-10 clause 9) at ASIL B. An integrator who uses the elements outside these assumptions must re-evaluate the safety argument for their item.

## 1. Scope

The elements are the runtime library `middleware` (types, ports, components, the table-driven state machine, control blocks and telemetry) and the proc-macro `middleware-codegen`, which turns a YAML model into Rust source. Telemetry (CBOR encoding, framing and buffering) is classified QM: it is a diagnostic output and must not be used as an input to any safety mechanism. All other runtime and generated code is developed to ASIL B.

The elements provide no hazard analysis, safety goals or technical safety concept. These belong to the integrator's item and must be derived by the integrator (ISO 26262-3 and -4). The requirements in `design.md` are the assumed software safety requirements; the integrator must confirm they are consistent with their own technical safety requirements.

## 2. Execution environment assumptions

The reference target is the RP2040 (Cortex-M0+, `thumbv6m-none-eabi`) in a `no_std` environment without a heap. The library allocates nothing at run time; every buffer and table is sized at compile time.

The control blocks currently use `f32` arithmetic. The M0+ has no FPU, so floating point is software emulated. Execution time therefore depends on operand values, and the integrator must measure worst-case execution time on the target with representative and boundary inputs. This is a recorded known deviation (see `design.md`, Known deviations). A conversion to fixed point is planned separately.

The integrator is responsible for freedom from interference: memory protection, stack sizing, watchdog supervision and timing supervision of the scheduler that calls active components. The library contains no `unsafe` code (enforced by `forbid(unsafe_code)`), so it cannot corrupt memory owned by other software through its own code paths, but it relies on the compiler, core library and target hardware being correct.

## 3. Assumptions on the YAML model

The model is the primary design artefact and must be reviewed as such. The generator rejects models that violate `schema/model.schema.json` or its semantic rules (unknown ports, wrong port direction, inverted limits, unknown states or events). Acceptance by the generator does not show that the model is correct for the item. Transition tables, guard names, PID gains and limits must be reviewed against the integrator's requirements.

Transition rows are evaluated in declaration order and the first row whose guard passes wins. Where two rows share a source state and event, their order is significant, and the reviewer must confirm it is intended.

The model file must be under configuration management together with the generated code's build, because the macro reads it at compile time.

## 4. Obligations on user-supplied hooks

The generated `Hooks` trait is the imperative shell through which all side effects enter a generated state machine. The integrator's implementation must meet the following.

Guards must be free of side effects, deterministic for a given system state, and complete in bounded time. A guard is called once per candidate row during a step, so a guard with side effects would make behaviour depend on row order in a way the model does not show.

Actions, entry hooks and exit hooks must complete in bounded time and must report failure through their `Error` type rather than by panicking. A panic on the target is a loss of the function.

Every `Err` returned by `step` must be handled. On an exit, action or entry failure, `step` returns the error and the caller keeps its previous state value. Side effects already performed by an earlier hook in the same step (for example an exit hook that succeeded before an action failed) are not rolled back. The integrator must define the safe reaction (retry, transition to a safe state through a dedicated event, or report to a supervisor) and must ensure that any partially applied side effect is safe.

An `Ignored` or `Remained` outcome is not an error. If an unexpected event must be treated as a fault for the item, the integrator must check for these outcomes explicitly.

## 5. Obligations on control block use

Control blocks reject non-finite inputs and non-positive or non-finite time steps with `ControlError::InvalidInput` or `ControlError::InvalidDelta`, and reject non-finite results with `ControlError::NumericalError`. On error no new block is returned. Because every control block type is `Copy`, the caller still holds the block as it was before the failed update and so keeps a consistent state. The integrator must decide the safe output when an update fails. The library does not substitute a default output.

PID integral clamping and output saturation bound the controller's output only within the configured limits. The integrator must choose limits that keep the actuator in a safe range, and must ensure that sensor plausibility checks happen before values reach a control block. Range checks on finite values are not performed by the library beyond the configured limits.

Control blocks are immutable values. State is advanced only by using the block returned from `update`; discarding it and calling the old block again repeats the previous step. This is intended, and it allows rollback, but the integrator must not mix stale and current instances.

## 6. Obligations on ports and components

Port reads and writes return `Result`. Errors such as a missing message or a full port must be handled by the component, and the component's reaction must be part of its reviewed design. Component lifecycle transitions are checked; an invalid transition returns `LifecycleError` and leaves the lifecycle state unchanged.

## 7. Telemetry

Telemetry encoding returns `BufferError::BufferOverflow` when the fixed-size buffer is too small. Because telemetry is QM, overflow loses diagnostic data only, and must not affect the control path. The integrator must place telemetry so that a telemetry fault cannot delay or block the safety-relevant cycle.

## 8. Tool use

The code generator is classified and qualified according to `docs/tool-qualification.md`. Its tool confidence level of TCL2 depends on a tool error detection measure that the integrator must carry out: code generated from the integrator's own models must be verified by requirements-based tests on that generated code, tracing to the integrator's requirements. Treating the generated code as correct by construction is outside these assumptions; without that verification the generator is TCL3 for that use. The integrator must use the toolchain versions recorded there, or re-run the qualification suite (`cargo test --workspace`) with their own versions and archive the result. Structural coverage is measured with a pinned nightly compiler; that dependency is a tool-confidence item recorded in the same document.

## 9. Verification evidence available to the integrator

Each requirement in `design.md` is linked to its verifying tests through `Verifies:` tags, checked by `cargo xtask trace --check`. The generated matrix is `docs/traceability.md`. Structural (branch) coverage is measured by `cargo xtask coverage`, with justified exclusions listed in `docs/coverage-exclusions.md`. The `no_std` build of generated code for the reference target is checked by building the `nostd-gen` crate for `thumbv6m-none-eabi`.

## 10. Outside the scope of these elements

The following are required for certification of an item but are not provided here: hazard analysis and risk assessment, safety plan, safety case, hardware-software interface specification, confirmation reviews, the functional safety assessment, and target-level integration and timing tests.
