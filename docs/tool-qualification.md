# Tool Qualification: middleware-codegen

This document is the tool-classification and qualification evidence for the `middleware-codegen` code generator, prepared per ISO 26262-8:2018 clause 11 (confidence in the use of software tools). The target integrity level is ASIL B. It is written as input to a tool confidence report and is not that report: the sign-off items listed under "Outside this repository" still have to be done.

## 1. Tool identification

| Item | Value |
|---|---|
| Tool | `middleware-codegen`, a Rust procedural-macro crate in this workspace (`middleware-codegen/`) |
| Version | 0.1.0 (workspace version), on git branch `iso26262-asil-b`, based on commit 2cf600d plus the working-tree changes under review |
| Entry points | the `middleware_model!` macro (YAML file) and the `middleware_model_inline!` macro (YAML string literal) |
| Inputs | a YAML model document and the embedded JSON Schema `schema/model.schema.json` (draft-07, built into the tool with `include_str!`) |
| Output | Rust tokens that rustc compiles into the invoking crate |
| Host compiler used for this evidence | rustc 1.91.1 and cargo 1.91.1 (stable). `Span::local_file`, which path resolution uses, needs rustc 1.88 or newer |
| Run-time dependencies of the tool (from `Cargo.lock`) | serde_yaml 0.9.34 (upstream marks it deprecated; it parses through unsafe-libyaml 0.2.11), serde 1.0.228, serde_json 1.0.145, jsonschema 0.30.0 (default features off, so no remote references are resolved), syn 2.0.111, quote 1.0.47, proc-macro2 1.0.103 |
| Verification-only dependencies | trybuild 1.0.122 (compile-fail and pass suite) and proptest 1.12.0 (back-to-back tests). Both are dev-dependencies and run on the host only |

The tool runs inside rustc while the invoking crate compiles. It has no configuration beyond the macro argument, and it reads only the named YAML file.

Path resolution follows the rule for `include_str!`. A relative path is joined onto the directory of the source file that contains the invocation, and an absolute path is used as given. The rule does not depend on the working directory or on `CARGO_MANIFEST_DIR`, so a given literal names the same file under cargo builds, doctests and the trybuild harness. The expansion also includes the canonical model path through `include_str!`, which makes cargo treat the YAML as a build input and rebuild when the model changes. The rustdoc on `middleware_model` in `middleware-codegen/src/lib.rs` states the rule normatively, and unit tests in the same file cover it. If the compiler reports no source file for the invocation, the macro emits a compile error rather than falling back to another base directory.

IDE limitation: rust-analyzer's proc-macro server does not report the invocation file the way rustc does. The editor can therefore show a "Failed to read YAML file" diagnostic for a path that rustc resolves correctly. The error is visible and nothing is generated from a wrong file. Only the rustc build is qualification evidence; the IDE expansion is not.

## 2. Use cases

The tool is used in the following ways in the development of ASIL B software built on the `middleware` runtime crate.

- **UC-1, state machines.** Generate a compile-time `TRANSITIONS` table, `Guard` and `Action` enums, `on_enter` and `on_exit` lookups, a `Hooks` trait and a pure `step` function from a model state machine. This is the most safety-relevant output: it decides which state the system is in and which actions run.
- **UC-2, data types and port messages.** Generate enums with an explicit `repr`, structs and messages, and fixed-width little-endian `PortMessage` serialization for every type carried on a port.
- **UC-3, components.** Generate `Config`, `State`, `Command` and `Ports` types, a component struct implementing `middleware::Component`, and a `handle` method that drives the linked state machine.
- **UC-4, math block constants.** Generate PID gains and limits, and filter frequency constants.
- **UC-5, traceability constants.** Generate requirement ID, description and level arrays.

In every use case the generated code becomes part of the safety-related item. Rustc compiles it, and the integrator then verifies it with requirements-based tests (see section 5.4). The tool also has to build for the `no_std` reference target; `checks/nostd-gen` holds that build check.

## 3. Tool impact and tool error detection

Tool impact (TI) states whether a malfunction of the tool can introduce or fail to detect an error in the safety-related item: TI1 if it cannot, TI2 if it can. Tool error detection (TD) states how confident we are that such an error is prevented or detected: TD1 high, TD2 medium, TD3 low or unknown. The table assesses each error class against the use cases above.

| Error class | TI | Measures that prevent or detect it | TD |
|---|---|---|---|
| E1. Wrong code generated silently: a wrong table row, wrong hook order, wrong guard or action wiring, wrong serialization layout, wrong constants | TI2 | Back-to-back testing of generated state machines against the independent runtime implementation (5.1). A table-against-YAML comparison (5.1). Compile-fail and pass suite (5.2). rustc type and exhaustiveness checks on generated code. Requirements-based tests of the generated code (5.4) | TD2 for UC-1, which the back-to-back tests cover. TD2 for UC-2 to UC-5 only when the integrator verifies the generated code with requirements-based tests; otherwise TD3 |
| E2. Invalid model accepted: an inconsistent or misspelled model produces code that compiles | TI2 | Schema validation (5.3). The schema is closed: every object sets `additionalProperties` to false, so an unknown or misspelled key such as `gaurd` is a compile error naming its JSON pointer. Semantic and generator checks, with one compile-fail case for every reachable `Err` path (5.2). rustc rejects generated code that references undefined types. Duplicate (state, event) rows are accepted, and the generated and runtime machines now give them the same meaning (section 6) | TD2 for every class the schema or a semantic check covers, which now includes misspelled optional keys |
| E3. Valid model rejected | TI1 | No code is generated and the build fails visibly | Not needed for TI1 |
| E4. Nondeterministic output: the same input gives different code across builds or hosts | TI2 | The generator is a pure function of the YAML text. It uses ordered `Vec` collections in model order and no hash maps, clocks, randomness or environment input. The path rule does not depend on the working directory. The output is checked through the same tests as E1 | TD2 (argued from design plus test repetition). No byte-for-byte reproducibility comparison of expansions is automated yet |
| E5. Stale output: generated code does not reflect the current model | TI2 | The expansion includes the model file through `include_str!`, so cargo's dependency tracking rebuilds the crate when the YAML changes | TD2 (this relies on cargo's change tracking) |
| E6. Wrong model file read | TI2 | There is a single resolution rule independent of the working directory, unit tested in `middleware-codegen/src/lib.rs`. The doctests and the trybuild cases, which build in a directory under `target/`, exercise it in the contexts where earlier behaviour broke. A missing file is a compile error that names both the requested path and the resolved path | TD1 |

### Resulting tool confidence level

The worst-case combination is TI2 with TD2 for the state-machine use case. For the data, component and constant use cases it is TI2 with TD2 only if the integrator verifies the generated code at requirements level, and TD3 if not.

- With the integrator's requirements-based verification of the generated code in place, the tool is classified **TCL2**. That verification is an assumption of use. `docs/safety-manual.md` does not state it yet, and it must be added there before this classification is relied on.
- Without that downstream verification, the class rated TD3 (the non-state-machine outputs) would make the tool **TCL3**. The tool must not be used that way at ASIL B unless further qualification is done.

Two earlier drivers of the TD3 ratings have been resolved: the schema now rejects unknown keys, and duplicate (state, event) rows behave the same in generated and runtime machines. Misspelled optional keys therefore move from TD3 to TD2. The remaining TD3 rating, for the non-state-machine outputs without integrator verification, is unchanged, so the classification above still holds. It should be re-evaluated if back-to-back or equivalent independent checks are added for UC-2 to UC-5.

## 4. Qualification method selected

The method used is **validation of the software tool**, ISO 26262-8 clause 11.4.9 (method 1c of the qualification tables). The validation measures are listed in section 5. At ASIL B, the standard's table for TCL2 rates increased confidence from use (1a) and evaluation of the tool development process (1b) more highly than validation. The tool confidence report therefore has to justify choosing validation, or add a 1b evaluation. The development-process evidence in this repository (requirements in `design.md`, traceability in `docs/traceability.md`, regression tests, review) is a starting point for that evaluation. Check the recommendation levels against the edition of ISO 26262-8 you assess against.

The validation is done in the tool's own use environment: the same rustc and cargo invoke the macro exactly as a user crate does. The model inputs are the shipped example (`examples/attitude_controller.yaml`) and targeted fixtures.

## 5. Validation evidence

All evidence below runs under `cargo test --workspace` and is regression tested on every run.

### 5.1 Back-to-back testing against the runtime implementation

File: `middleware/tests/codegen_back_to_back.rs`. Fixtures: `middleware/tests/fixtures/b2b_door.yaml` and `middleware/tests/fixtures/b2b_duplicate_rows.yaml`.

The runtime crate's table-driven `middleware::state_machine::StateMachine` (`middleware/src/state_machine/machine.rs`, `transition.rs`, `state.rs`, `event.rs`) is used as the reference. It was written separately from the generator and shares no code with it. The test reads each model YAML with `serde_yaml`, independently of the tool's own parser. It builds the runtime transition table from the YAML alone and uses the generated enums only to name variants.

Three models are covered: `examples/attitude_controller.yaml` (two guards, transition actions, entry and exit hooks), the door fixture and the duplicate-rows fixture. The door fixture adds a guard shared by two rows, guarded and unguarded self-loops, an action used both as a transition action and as an entry hook, and an enum variant that has no state entry. The duplicate-rows fixture (`middleware/tests/fixtures/b2b_duplicate_rows.yaml`, 7 rows) has several rows for one (state, event) pair: guarded rows followed by an unguarded fallback row, two guarded rows on one pair, and hooks for every failure kind. Each model is checked three ways.

1. **Table equivalence.** The generated enum order and discriminants, every `TRANSITIONS` row (source, event, target, guard, action), every `on_enter` and `on_exit` lookup, and the initial state all equal the YAML.
2. **Lockstep property** (proptest, 1024 cases per model, including the duplicate-rows model). Each case is a random event sequence of 0 to 63 steps. Every step draws its own guard outcomes, and in about 10% of steps one action is chosen to fail. At every step the test asserts that the two machines agree on the state reached, on the outcome (transitioned with from and to, remained, ignored, exit failed, action failed, entry failed), and on the ordered list of actions run, including the partial list before a failure.
3. **Coverage of the oracle.** A deterministic run over 512 seeded sequences must fire every table row and produce all six outcome kinds. This shows the lockstep property is not vacuous.

A fault was injected by hand during development: the exit action and the transition action were swapped in the generator. Both the lockstep property and the coverage check failed for both models. The change was then reverted. That fault-injection run is not automated.

### 5.2 Compile-fail and pass suite

Harness: `middleware-codegen/tests/compile_fail.rs`. Cases are in `middleware-codegen/tests/ui/` (one `.rs` file and a committed `.stderr` file per case) and fixtures in `middleware-codegen/tests/fixtures/`. Each case must fail with exactly its committed diagnostic. The suite has 36 compile-fail cases.

- **Schema violation classes:** a missing top-level required field, a missing nested required field, a wrong type, an unknown enum value, a pattern mismatch, `minItems`, an invalid enum `repr`, and an unknown key (`schema_unknown_key`: a transition misspelling `guard` as `gaurd` is rejected at `/state_machines/0/transitions/0`).
- **Input errors:** a missing file, malformed YAML, YAML that cannot be represented as JSON (a non-string key), a value that does not fit its model type, a non-literal macro argument, and a schema violation through `middleware_model_inline!`. Because the schema now declares the type of every math-block `config` key, the type-mismatch case is reported by the schema check before the parser runs. The parser's own type error remains as defence in depth and is covered by a unit test in `middleware-codegen/src/parser.rs`.
- **Semantic and generator errors:**
  - State machines: an initial state not listed in the machine's states, a state type that is not a model enum, an event type that is not a model enum, an unknown state or event in a transition, a duplicate state, the same name used as both a guard and an action, and guard names or action names that collide after PascalCase conversion.
  - PID blocks: inverted output limits and inverted integral limits.
  - Port messages: a port message type that is not defined, a field type that cannot be encoded, and a message larger than the runtime maximum.
  - Ports and components: a duplicate port, an undefined component port, a port listed under the wrong direction, a port listed twice in one component, an undefined linked state machine, a component state that differs from its machine's state type, a component state that is not an enum, and a duplicate command.

One pass case is in `middleware-codegen/tests/ui/pass/`. It expands the example model and checks the generated table, lookups and constants at run time. The former second pass case, which pinned the acceptance of unknown keys, became the `schema_unknown_key` compile-fail case when the schema was closed.

Some `Err` paths have no compile-fail case because the schema already rejects their inputs before the check runs. These include the version and namespace format checks, PascalCase and snake_case name checks, enum `repr`, component and math-block kinds, the requirement ID prefix, port direction parsing, and command name syntax. They stay in the tool as defence in depth. The overflow of a 16-bit component ID also has no case, because it needs 65,536 components. The schema cases cover the first group indirectly.

### 5.3 Schema validation

The unit tests in `middleware-codegen/src/validator.rs` check the following:
- the embedded schema compiles;
- the example model passes;
- missing, nested-missing, wrong-type and unknown-enum violations are reported with their JSON pointer;
- an unknown key is rejected with its JSON pointer and names the key, both inside a transition (the `gaurd` misspelling) and at the top level and inside a math-block `config`;
- all violations are reported together;
- invalid YAML is reported.

The compile-fail suite (5.2) confirms that these reports reach the user as compile errors.

### 5.4 Requirements-based verification of generated code

The following files test the behaviour of the generated code:
- `middleware/tests/attitude_controller_model.rs`: component structure, port message round trips, guard behaviour, the exit, action and enter order, fault recovery, hook failure and PID limits for the example model;
- `middleware-codegen/tests/codegen_integration.rs`: enum, struct, traceability and state-machine generation from inline models;
- the unit tests in `middleware-codegen/src/generator.rs`, `generator/state_machine.rs`, `generator/naming.rs` and `parser.rs`;
- the runnable doctests on both macros in `middleware-codegen/src/lib.rs`;
- the build check in `checks/nostd-gen` for the `no_std` reference target.

The generated code is production code in the integrator's crate, so it is held to the same coding guidelines as the rest of the workspace (no `unsafe`, no `unwrap`, `expect` or `panic`, no indexing or slicing, no `as` conversions, no float `==`). The templates in `middleware-codegen/src/generator/` meet them by construction. Enum discriminants are read through a generated `const fn to_repr` (an exhaustive `match`) and `From<Enum> for <repr>` instead of `as` casts. The transition lookup walks the table with a slice pattern rather than an index. Port messages use checked `get` and `get_mut` access and return `SerializationError`. `checks/nostd-gen` opts into the workspace lints, and `cargo clippy -p nostd-gen -- -D warnings` passes. Clippy may skip lints inside macro expansions, so this is supported by review of the templates and by a unit test in `middleware-codegen/src/generator.rs` that checks the enum conversion is emitted as a `match`. During this change the pretty-printed expansions of the example model and of the back-to-back fixtures were also linted as ordinary source with the workspace lints, and none of the denied lints fired. That check is not automated.

For the integrator's own models, requirements-based testing of the generated code is an assumption of use that the TCL2 result depends on. It still has to be written into `docs/safety-manual.md`, which currently covers model configuration management, the `Hooks` contract and the `no_std` build check, but not this verification duty.

## 6. Findings and residual risks

- **Duplicate (state, event) rows (resolved).** The generated and runtime machines previously differed when several rows matched one state and event. Their semantics are now aligned: rows are scanned in table order and the first row matching (state, event) whose guard is absent or true fires; if rows match but every guard is false the outcome is `Remained`; if no row matches it is `Ignored`. Aligned; verified by `middleware/tests/codegen_back_to_back.rs`, through the deterministic check `duplicate_rows::fallthrough_agrees` (fall-through and all-guards-false cases) and the 1024-case lockstep property (5.1), which now also runs over `middleware/tests/fixtures/b2b_duplicate_rows.yaml` (7 rows). Duplicate rows are therefore inside the compared domain and no longer contribute to the TD3 ratings.
- **Unknown keys (resolved).** Every object in `schema/model.schema.json`, at the top level and nested, now sets `additionalProperties` to false, and the math-block `config` object declares each key the parser reads. A misspelled optional key, such as `gaurd` instead of `guard`, is therefore a compile error rather than an unguarded transition. The compile-fail case `middleware-codegen/tests/ui/schema_unknown_key.rs` and the validator unit tests (5.3) verify this. The example model and every pass fixture, including the back-to-back fixtures, validate against the closed schema. This item moved from TD3 to TD2.
- **Generated-code style lints.** Generated math-block and state-machine modules use upper-case and PascalCase names, which rustc's `non_snake_case` style lint would flag in hand-written code, and the component `handle` signature is long enough to trigger clippy's `type_complexity`. Neither is a coding-guideline lint, and rustc suppresses both inside macro expansions. Renaming the modules would change the generated API, so it is left for a separate change.
- **Reliance on third-party software.** rustc compiles both the tool and its output. The tool's correctness also depends on serde_yaml (deprecated upstream and built on unsafe-libyaml), serde, serde_json, jsonschema, syn, quote and proc-macro2. None of these is qualified here. The back-to-back and requirements-based tests detect their errors only in the outputs those tests exercise. Replacing serde_yaml with a maintained YAML parser is recommended.
- **Float constants.** PID and filter values are parsed as f32 and emitted as f32 literals. Rounding is that of serde_yaml and rustc. Only the values asserted in 5.4 are tested.
- **Shared assumptions.** The oracle runtime and the generator were written in the same project, so they may share a misreading of the requirements. The back-to-back test detects disagreement, not a misunderstanding common to both. Requirements-based tests (5.4) are the mitigation.
- **Diagnostic text.** The committed `.stderr` files contain rustc's diagnostic layout and operating-system error text (for example "os error 2" on Linux). A different toolchain version or host may require regenerating and reviewing them. A diagnostic that changes is not a tool malfunction.
- **Determinism.** Determinism is argued from the design and not measured by an automated comparison of expansions across builds or hosts.

## 7. Outside this repository

The following items are needed for a complete tool qualification and cannot be produced by the code in this workspace:

- the signed tool confidence report: classification review, a justification for the chosen qualification method, and approval by the responsible safety manager or assessor;
- configuration management of the qualified tool version: a release tag, the toolchain pinned for production builds, and the archived evidence from the qualifying test run;
- a decision on how rustc itself is trusted, for example a qualified compiler distribution or a separate argument, and on how the third-party libraries listed in section 6 are trusted;
- if the tool confidence report requires them, records of increased confidence from use (1a) or an evaluation of the tool development process (1b);
- independent review of this analysis at the level of independence required for ASIL B.
