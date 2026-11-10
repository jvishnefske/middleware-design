# Structural Coverage: Results and Justified Gaps

This document records the structural coverage result required by ISO 26262-6 Table 9 (unit level). At ASIL B, branch coverage is the highly recommended metric. It is measured by `cargo xtask coverage`, which runs `cargo llvm-cov --branch` on the pinned toolchain `nightly-2025-09-08` over the `middleware` and `middleware-codegen` packages. The command writes the HTML report to `target/coverage/html/index.html` and the JSON summary to `target/coverage/summary.json`, and it fails below the thresholds in `xtask/src/coverage.rs`.

## Result

At the time the gate was introduced, coverage was 96.78% of branches (271 of 280) and 98.94% of lines (7035 of 7110, counted after `cargo fmt`, which put assertion-message arguments on lines of their own that only run when a test fails). Before requirements-based testing the figures were 71.81% branches and 93.02% lines. The gate thresholds are 96.50% branches and 98.90% lines.

Merged across all instantiations and test binaries, every branch in non-test (production) code is covered. Each of the 9 uncovered branches in the merged summary is listed below with its justification.

## Uncovered branches in test code (8)

These are not production code. They are the short-circuit side of an `&&` inside an assertion, or a defensive fallback in a test helper. A test reaching them would mean the test had already failed.

- `middleware-codegen/src/generator.rs`, test `enum_conversion_is_a_match_not_a_cast`: the second operand of an `&&` assertion.
- `middleware-codegen/src/generator/state_machine.rs`, multi-machine output test: the second operand of an `&&` assertion.
- `middleware-codegen/src/validator.rs`, test `every_schema_object_is_closed`: the failure branch of the walk, which would report an open object schema.
- `middleware/src/control/filters.rs`, tests `test_lowpass_from_cutoff` and `test_highpass_from_cutoff`: the second operand of the alpha range assertions (2 branches).
- `middleware/src/state_machine/machine.rs`, call-recording test helper: the `None` side of a bounds-checked `get`, used because indexing is denied by the coding guidelines.
- `middleware/src/telemetry/buffer.rs`, two size assertions in tests: the second operand of an `&&` assertion (2 of the 3 uncovered branches in this file).

## Uncovered branches that are tool artefacts (1)

- `middleware/src/telemetry/buffer.rs`, the `e.is_write()` decision in `TelemetryBuffer::encode`. The buffer is generic over its capacity, and llvm-cov counts the decision once per monomorphised instantiation. The overflow tests take the true side, and `test_non_write_error_is_encode_error` takes the false side. They use different capacities, so in the merged summary each instantiation shows one side as untaken. Both outcomes of the source-level decision are exercised.

## Code not visible to the coverage tool

Code that runs only inside rustc while `middleware_model!` expands does not contribute coverage data to the test binaries. That includes the `#[proc_macro]` entry points in `middleware-codegen/src/lib.rs`. The proc-macro dynamic library is itself instrumented during the coverage build, but it only ever expands the valid models used by the integration tests. Its per-binary records therefore show the rejection branches of `parser.rs`, `generator/*.rs` and `naming.rs` as untaken. The same branches are covered by the library's unit tests, which call the pure functions directly. The merged summary takes the union, which is why these files report full branch coverage. The entry points themselves contain no decisions; their logic lives in the unit-tested `expand_model_file` and `expand_model_text`. The compile-fail suite (`middleware-codegen/tests/compile_fail.rs`) verifies the end-to-end behaviour through rustc.

The coverage build prints "function basic block count change detected (counter mismatch)" warnings for functions of the third-party `syn` crate. They come from `syn` being compiled twice with different instrumentation (as a dependency of the proc macro and of the test binaries). They concern third-party code only and do not affect the figures for this workspace's files.

## Defensive code that cannot fail

The following checks are kept as defence in depth. They have no uncovered branch in the merged summary, but their failure arms cannot be reached from valid inputs. They are listed so that a reviewer does not mistake them for missing tests.

- `middleware/src/types/bounded.rs`, `append_bytes`: `checked_add` on the length cannot overflow, because the length is at most the capacity and a slice length is at most `isize::MAX`.
- `middleware-codegen/src/generator/naming.rs`, `to_pascal_case`: the empty-part arm is unreachable because empty parts are filtered out first.
- `middleware-codegen/src/validator.rs`: the `Display` `write!` error arm only fails if the formatter fails. The embedded-schema compile error is checked by `embedded_schema_compiles`.
- `middleware-codegen/src/generator/component.rs`: the component index conversion to `u16` would need more than 65,535 components.
- `middleware-codegen/src/lib.rs`: the parse-failure arm after schema validation cannot be reached, because the schema types every key. `parse_model`'s error is unit tested directly in `parser.rs`.

## Not measured

- **MC/DC:** not measured. It is highly recommended only at ASIL D (see `design.md`, Known deviations).
- **`checks/nostd-gen`:** excluded from the coverage run. It contains no library logic, and its platform tests need the `thumbv6m-none-eabi` target, which the pinned nightly does not have installed.
- **`xtask`:** host-only build automation, not delivered code, so it is excluded.
