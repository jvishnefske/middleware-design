//! trybuild suite for the `middleware_model!` and `middleware_model_inline!`
//! macros.
//!
//! Every file in `tests/ui/*.rs` must fail to compile with exactly the
//! diagnostic committed next to it in `tests/ui/<case>.stderr`. There is one
//! case per schema violation class (including an unknown key, which the
//! closed schema rejects), per input error (missing file, malformed YAML,
//! YAML that cannot be checked as JSON, a value that does not fit its model
//! type, a non-literal argument) and per reachable semantic or
//! code-generation `Err` path. Because the schema declares the type of every
//! key, a value of the wrong model type is now reported by the schema check;
//! the parser's own type error stays as defence in depth and is unit tested
//! in `src/parser.rs`. The YAML for each case
//! is `tests/fixtures/<case>.yaml`. Every file in `tests/ui/pass/*.rs` must
//! compile and run successfully.
//!
//! # Why the suite lives in this crate
//! The cases only need the macro, plus the `middleware` runtime crate for the
//! pass case that expands a model with components. `middleware` is a
//! dev-dependency of this crate (a dev-dependency cycle that cargo allows),
//! and trybuild makes the dev-dependencies of the crate under test available
//! to every case.
//!
//! # Paths
//! trybuild builds the cases in a generated project under
//! `target/tests/trybuild/`, so both the working directory and
//! `CARGO_MANIFEST_DIR` point there during expansion. The case files keep
//! their original location, and `middleware_model!` resolves its argument
//! against the directory of the invoking source file, so `../fixtures/x.yaml`
//! in `tests/ui/x.rs` names `tests/fixtures/x.yaml` here.
//!
//! Regenerate the `.stderr` files with `TRYBUILD=overwrite cargo test -p
//! middleware-codegen --test compile_fail` and review the diff.
//!
//! # Test functions
//! The cases are split by error class, one `#[test]` per class, so each
//! carries the requirement tags it verifies. Every `.rs` file in `tests/ui/`
//! is matched by exactly one of the globs below.
//!
//! trybuild prepares its shared project directory before taking its own
//! file lock, so two `TestCases` running at once in this binary race on the
//! build files. Each test therefore holds [`TRYBUILD`] while its cases run;
//! the `TestCases` value is dropped (and runs) before the guard.
//!
//! # Traceability
//! - REQ-GEN-010: the macro reads the YAML file at compile time
//! - REQ-GEN-012: invalid models become compile errors
//! - REQ-GEN-014: an unknown key is a compile error
//! - REQ-GEN-015: paths resolve relative to the invoking source file

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serialises the trybuild runs of this binary (see the module docs). The
/// only shared state is this unit lock; a test that panics while holding it
/// poisons it, which is ignored because the guarded data is `()`.
static TRYBUILD: Mutex<()> = Mutex::new(());

fn serialised() -> MutexGuard<'static, ()> {
    TRYBUILD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Schema violations, from a file or inline, are compile errors naming the
/// JSON pointer of each violation; `schema_unknown_key` shows an unknown
/// key is rejected rather than ignored.
/// Verifies: REQ-GEN-012, REQ-GEN-014
#[test]
fn schema_violations_are_compile_errors() {
    let _guard = serialised();
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/schema_*.rs");
    t.compile_fail("tests/ui/inline_*.rs");
}

/// Input errors: a missing file (the diagnostic shows the path resolved
/// against the invoking file's directory), malformed YAML, YAML that cannot
/// be checked as JSON, a value of the wrong model type and a non-literal
/// argument.
/// Verifies: REQ-GEN-010, REQ-GEN-012, REQ-GEN-015
#[test]
fn input_errors_are_compile_errors() {
    let _guard = serialised();
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/input_*.rs");
}

/// Semantic and code-generation errors that the schema cannot express:
/// unknown references, duplicates, name collisions, port encoding limits
/// and inverted PID limits.
/// Verifies: REQ-GEN-017
#[test]
fn semantic_errors_are_compile_errors() {
    let _guard = serialised();
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/component_*.rs");
    t.compile_fail("tests/ui/fsm_*.rs");
    t.compile_fail("tests/ui/port_*.rs");
    t.compile_fail("tests/ui/pid_*.rs");
    t.compile_fail("tests/ui/semantic_*.rs");
}

/// A valid model named by a path relative to the case file expands and
/// its generated table and lookups match the YAML.
/// Verifies: REQ-GEN-010, REQ-GEN-011, REQ-GEN-015
#[test]
fn valid_models_compile_and_run() {
    let _guard = serialised();
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
}
