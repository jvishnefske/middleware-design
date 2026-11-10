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
//! # Traceability
//! - REQ-GEN-010: the macro reads the YAML file at compile time
//! - REQ-GEN-012: invalid models become compile errors

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    t.pass("tests/ui/pass/*.rs");
}
