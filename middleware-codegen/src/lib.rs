//! # Middleware Code Generation
//!
//! Proc-macro for generating Rust code from YAML model definitions.
//!
//! # Traceability
//! - REQ-GEN-010: middleware_model! macro reads YAML at compile time
//! - REQ-GEN-011: Macro generates Rust structs, traits, and implementations
//! - REQ-GEN-012: Macro validates YAML against schema and emits compile errors
//! - REQ-GEN-013: Generated code includes traceability comments and const arrays
//! - REQ-GEN-015: Model paths resolve relative to the invoking file; the model
//!   is a build input of the invoking crate
//!
//! The proc-macro entry points only read the invocation from rustc and
//! convert tokens; the expansion itself is in plain functions
//! (`expand_model_file`, `expand_model_text`) that are unit tested.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::path::{Path, PathBuf};

use syn::{parse_macro_input, LitStr};

mod generator;
mod model;
mod parser;
mod validator;

use generator::generate_code;
use parser::parse_model;

/// Generates Rust code from a YAML model file.
///
/// # Usage
///
/// ```
/// // This doctest lives in `middleware-codegen/src/lib.rs`, so the path is
/// // relative to `middleware-codegen/src/`.
/// middleware_codegen::middleware_model!("../../examples/attitude_controller.yaml");
///
/// use attitude_control::{ControllerStateMachine as Fsm, SystemEvent, SystemState};
///
/// assert_eq!(Fsm::INITIAL_STATE, SystemState::Initializing);
/// assert_eq!(Fsm::TRANSITION_COUNT, 8);
/// let row = Fsm::find_transition(SystemState::Ready, SystemEvent::Start);
/// assert_eq!(row.map(|r| r.to), Some(SystemState::Active));
/// assert_eq!(row.and_then(|r| r.guard), Some(Fsm::Guard::SensorsCalibrated));
/// ```
///
/// # Path resolution
///
/// The argument must be a string literal. It is resolved exactly like the
/// argument of `include_str!`: a relative path is joined onto the directory
/// of the source file that contains the macro invocation, and an absolute
/// path is used unchanged. The source file is the one rustc reports for the
/// invocation span (`proc_macro::Span::local_file`). The rule does not
/// depend on the working directory or on `CARGO_MANIFEST_DIR`, so the same
/// literal works under `cargo build`, `cargo test`, doctests and trybuild.
/// If rustc reports no file on disk for the invocation (for example when the
/// invocation is itself produced by a macro whose span has no real file), the
/// macro emits a `compile_error!` instead of guessing a base directory;
/// there is no fallback to the working directory or to `CARGO_MANIFEST_DIR`.
///
/// IDE limitation: rust-analyzer's proc-macro server does not report the
/// invocation file the way rustc does, so in the editor the expansion can
/// show "Failed to read YAML file" (or the no-source-file error) for a path
/// that rustc resolves correctly. The macro never falls back to a different
/// base directory in that case; rustc's result is authoritative.
///
/// The expansion also contains `const _: &str = include_str!(<canonical
/// path>);`, so cargo records the YAML file as a build input and recompiles
/// the invoking crate whenever the model changes. If the canonical path
/// cannot be determined or is not valid UTF-8, the macro emits a
/// `compile_error!` rather than expanding a model that would not be rebuilt
/// when it changes.
///
/// # Generated Code
///
/// The YAML is first checked against `schema/model.schema.json`; any
/// violation becomes a compile error naming its JSON pointer path.
///
/// The macro generates:
/// - Enums with an explicit `#[repr]` for states and events, each with a
///   `const fn to_repr` and `From<Enum> for <repr>` so callers never need an
///   `as` cast
/// - Structs for messages and configuration
/// - `PortMessage` impls (fixed-width little-endian) for every port message type
/// - Per state machine: a const `TRANSITIONS` table whose rows carry optional
///   guards and actions, `Guard`/`Action` enums, `on_enter`/`on_exit`, a
///   `Hooks` trait the user implements, and a pure `step` function
/// - Per component: `Config`, `State`, `Command` and `Ports` types, a struct
///   implementing `middleware::Component`, and a `handle` method that drives
///   its linked state machine
/// - Math block configurations, including PID `INTEGRAL_MIN`
/// - Traceability comments linking to requirements
///
/// # Errors
///
/// Every failure is reported as a `compile_error!`: the model file cannot be
/// located, read or tracked as a build input, the YAML violates the schema,
/// it cannot be parsed into the model, or a semantic check in code
/// generation rejects it.
///
/// # Traceability
/// - REQ-GEN-010: Reads YAML at compile time
/// - REQ-GEN-011: Generates structs, traits, implementations
/// - REQ-GEN-012: Validates YAML against `schema/model.schema.json` (see
///   `validator`); any violation becomes a `compile_error!` listing each
///   instance path and message. Semantic checks run afterwards.
/// - REQ-GEN-013: Includes traceability in generated code
/// - REQ-GEN-015: Resolves the path against the invoking file and tracks the
///   model as a build input
#[proc_macro]
pub fn middleware_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    expand_model_file(
        &input.value(),
        proc_macro::Span::call_site().local_file(),
        input.span(),
    )
    .into()
}

/// Expands `middleware_model!` for the model path `requested`, written in
/// the source file `invoking_file`.
///
/// This is the whole macro except for reading the invocation span from
/// rustc, so it can be unit tested: the path is resolved with
/// [`resolve_model_path`], the file is read, its canonical path is turned
/// into a build-input `include_str!` by [`track_model_file`], and the text is
/// expanded by [`expand_model_text`]. Any failure becomes a
/// `compile_error!`; schema errors are reported at `span` (the literal).
fn expand_model_file(
    requested: &str,
    invoking_file: Option<PathBuf>,
    span: proc_macro2::Span,
) -> TokenStream2 {
    let path = match resolve_model_path(requested, invoking_file) {
        Ok(path) => path,
        Err(msg) => return quote! { compile_error!(#msg); },
    };

    // Read YAML file at compile time
    let yaml_content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) => {
            let msg = format!(
                "Failed to read YAML file '{}' (resolved to '{}'): {}",
                requested,
                path.display(),
                e
            );
            return quote! { compile_error!(#msg); };
        }
    };

    // REQ-GEN-015: register the model file as a build input of the invoking crate.
    let tracked = match track_model_file(&path) {
        Ok(tokens) => tokens,
        Err(msg) => return quote! { compile_error!(#msg); },
    };

    match expand_model_text(&yaml_content, span) {
        Ok(tokens) => quote! { #tracked #tokens },
        Err(error) => error,
    }
}

/// Returns `const _: &str = include_str!(<canonical path>);` for the model
/// file at `path`, so cargo recompiles the invoking crate whenever the file
/// changes.
///
/// # Errors
/// Returns a message if `path` cannot be canonicalised, or if the canonical
/// path is not valid UTF-8 and so cannot be written as an `include_str!`
/// literal. Both are errors rather than a silently untracked model, which
/// would let a stale expansion survive a model change.
fn track_model_file(path: &Path) -> Result<TokenStream2, String> {
    let canonical = std::fs::canonicalize(path).map_err(|e| {
        format!(
            "Cannot track YAML file '{}' as a build input: {}",
            path.display(),
            e
        )
    })?;
    include_str_tokens(&canonical)
}

/// Builds the `include_str!` item for an already canonical path.
///
/// # Errors
/// Returns a message if `canonical` is not valid UTF-8.
fn include_str_tokens(canonical: &Path) -> Result<TokenStream2, String> {
    canonical
        .to_str()
        .map(|abs| quote! { const _: &str = include_str!(#abs); })
        .ok_or_else(|| {
            format!(
                "Cannot track YAML file '{}' as a build input: the path is not valid UTF-8",
                canonical.display()
            )
        })
}

/// Validates, parses and generates code for model text.
///
/// Shared by both macros: the YAML is checked against the embedded schema
/// (violations reported at `span`), parsed into the model, and passed to
/// the generator.
///
/// # Errors
/// Returns the `compile_error!` tokens for the first stage that fails.
fn expand_model_text(
    yaml_content: &str,
    span: proc_macro2::Span,
) -> Result<TokenStream2, TokenStream2> {
    // REQ-GEN-012: validate against the embedded JSON schema before parsing
    validator::validate_yaml(yaml_content)
        .map_err(|e| syn::Error::new(span, e).to_compile_error())?;

    // Parse YAML into model
    let model = parse_model(yaml_content).map_err(|e| {
        let msg = format!("Failed to parse YAML: {}", e);
        quote! { compile_error!(#msg); }
    })?;

    // Generate code
    generate_code(&model).map_err(|e| {
        let msg = format!("Code generation failed: {}", e);
        quote! { compile_error!(#msg); }
    })
}

/// Resolves the path given to [`middleware_model!`] against the source file
/// that contains the invocation.
///
/// Pure: `invoking_file` is the path rustc reports for the invocation span
/// (possibly relative to rustc's working directory, in which case the result
/// is relative to it as well, just as for `include_str!`). A relative
/// `requested` path is joined onto the parent directory of `invoking_file`;
/// an absolute `requested` path replaces it, per [`Path::join`].
///
/// # Errors
/// Returns a message if `invoking_file` is `None`, because there is then no
/// directory to resolve a relative path against.
fn resolve_model_path(requested: &str, invoking_file: Option<PathBuf>) -> Result<PathBuf, String> {
    let file = invoking_file.ok_or_else(|| {
        format!(
            "Cannot resolve YAML file '{}': rustc reports no source file on disk for this \
             middleware_model! invocation",
            requested
        )
    })?;
    let base = file.parent().unwrap_or_else(|| Path::new(""));
    Ok(base.join(requested))
}

/// Generates code from inline YAML content.
///
/// # Usage
///
/// ```
/// middleware_codegen::middleware_model_inline! {
///     r#"
///     version: "1.0"
///     namespace: "my_system"
///     types:
///       enums:
///         - name: MyState
///           repr: u8
///           variants: [Idle, Running]
///     "#
/// }
///
/// assert_eq!(my_system::MyState::COUNT, 2);
/// assert_eq!(u8::from(my_system::MyState::Running), 1);
/// assert_eq!(my_system::MyState::Idle.to_repr(), 0);
/// ```
///
/// # Traceability
/// - REQ-GEN-010: Processes YAML at compile time
/// - REQ-GEN-012: Validates YAML against the embedded schema; violations
///   become a `compile_error!` naming each instance path and message
#[proc_macro]
pub fn middleware_model_inline(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    match expand_model_text(&input.value(), input.span()) {
        Ok(tokens) | Err(tokens) => tokens.into(),
    }
}

#[cfg(test)]
// test code: a panic is a test failure, the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
mod tests {
    use super::*;

    /// This source file, as rustc would report it for an invocation here.
    fn this_file() -> Option<PathBuf> {
        Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
    }

    fn span() -> proc_macro2::Span {
        proc_macro2::Span::call_site()
    }

    /// Verifies: REQ-GEN-015
    #[test]
    fn relative_path_joins_invoking_file_directory() {
        let resolved = resolve_model_path(
            "../fixtures/m.yaml",
            Some(PathBuf::from("/ws/crate/tests/t.rs")),
        );
        assert_eq!(
            resolved,
            Ok(PathBuf::from("/ws/crate/tests/../fixtures/m.yaml"))
        );
    }

    /// Verifies: REQ-GEN-015
    #[test]
    fn relative_invoking_file_stays_relative_to_rustc_cwd() {
        let resolved = resolve_model_path("m.yaml", Some(PathBuf::from("crate/src/lib.rs")));
        assert_eq!(resolved, Ok(PathBuf::from("crate/src/m.yaml")));
    }

    /// Verifies: REQ-GEN-015
    #[test]
    fn absolute_path_is_used_unchanged() {
        let resolved = resolve_model_path("/abs/m.yaml", Some(PathBuf::from("/ws/src/lib.rs")));
        assert_eq!(resolved, Ok(PathBuf::from("/abs/m.yaml")));
    }

    /// Boundary: an invoking file with no parent directory resolves against
    /// the empty path, i.e. rustc's working directory, as `include_str!` does.
    /// Verifies: REQ-GEN-015
    #[test]
    fn invoking_file_without_parent_resolves_against_empty_base() {
        let resolved = resolve_model_path("m.yaml", Some(PathBuf::from("/")));
        assert_eq!(resolved, Ok(PathBuf::from("m.yaml")));
    }

    /// Verifies: REQ-GEN-015
    #[test]
    fn missing_invoking_file_is_an_error() {
        let err = resolve_model_path("m.yaml", None).unwrap_err();
        assert!(err.contains("no source file on disk"), "{}", err);
    }

    /// The expansion of a model file contains `include_str!` of the
    /// canonical path of the file resolved next to the invoking source file,
    /// which makes cargo rebuild the invoking crate when the model changes,
    /// followed by the generated module.
    /// Verifies: REQ-GEN-010, REQ-GEN-015
    #[test]
    fn file_expansion_includes_canonical_model_path() {
        let requested = "../../examples/attitude_controller.yaml";
        let canonical = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join(requested),
        )
        .unwrap();
        let expected = format!(
            "const _ : & str = include_str ! ({:?}) ;",
            canonical.to_str().unwrap()
        );
        let code = expand_model_file(requested, this_file(), span()).to_string();
        assert!(code.starts_with(&expected), "{}", code);
        assert!(code.contains("pub mod attitude_control"), "{}", code);
        assert!(!code.contains("compile_error"), "{}", code);
    }

    /// The same request from a file in another directory resolves to a
    /// different model: resolution follows the invoking file, not the
    /// working directory.
    /// Verifies: REQ-GEN-015
    #[test]
    fn file_expansion_resolves_against_invoking_file_not_cwd() {
        let requested = "../examples/attitude_controller.yaml";
        // From src/lib.rs this names middleware-codegen/examples/, which does not exist.
        let code = expand_model_file(requested, this_file(), span()).to_string();
        assert!(code.contains("Failed to read YAML file"), "{}", code);
        // From the crate root (one level up) it names the workspace example.
        let at_root = Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"));
        let code = expand_model_file(requested, at_root, span()).to_string();
        assert!(code.contains("include_str !"), "{}", code);
        assert!(code.contains("pub mod attitude_control"), "{}", code);
    }

    /// Fault: a missing file is a compile error naming the request and the
    /// resolved path, and nothing else is emitted.
    /// Verifies: REQ-GEN-010, REQ-GEN-015
    #[test]
    fn missing_model_file_is_a_compile_error() {
        let code = expand_model_file("no_such_model.yaml", this_file(), span()).to_string();
        assert!(code.starts_with("compile_error !"), "{}", code);
        assert!(code.contains("'no_such_model.yaml'"), "{}", code);
        let resolved = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/no_such_model.yaml");
        assert!(code.contains(resolved.to_str().unwrap()), "{}", code);
        assert!(!code.contains("include_str"), "{}", code);
    }

    /// Fault: no invoking file on disk is a compile error, not a guess.
    /// Verifies: REQ-GEN-015
    #[test]
    fn file_expansion_without_invoking_file_is_a_compile_error() {
        let code = expand_model_file("m.yaml", None, span()).to_string();
        assert!(code.starts_with("compile_error !"), "{}", code);
        assert!(code.contains("no source file on disk"), "{}", code);
    }

    /// Fault: a path that cannot be canonicalised cannot be tracked.
    /// Verifies: REQ-GEN-015
    #[test]
    fn untrackable_path_is_an_error() {
        let err = track_model_file(Path::new("/definitely/not/here.yaml")).unwrap_err();
        assert!(err.contains("Cannot track YAML file"), "{}", err);
    }

    /// Fault: a non-UTF-8 path cannot be written as an `include_str!`
    /// literal, so it is rejected instead of silently dropping the rebuild
    /// dependency.
    /// Verifies: REQ-GEN-015
    #[cfg(unix)]
    #[test]
    fn non_utf8_path_is_rejected() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(OsStr::from_bytes(b"/tmp/m\xff.yaml"));
        let err = include_str_tokens(path).unwrap_err();
        assert!(err.contains("not valid UTF-8"), "{}", err);
        let ok = include_str_tokens(Path::new("/m.yaml"))
            .unwrap()
            .to_string();
        assert_eq!(ok, "const _ : & str = include_str ! (\"/m.yaml\") ;");
    }

    /// Fault: a model that is read successfully from a directory whose
    /// name is not UTF-8 cannot be tracked, so the expansion is a compile
    /// error and no generated code is emitted without its rebuild dependency.
    /// Verifies: REQ-GEN-015
    #[cfg(unix)]
    #[test]
    fn readable_model_in_non_utf8_directory_is_a_compile_error() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let mut name = b"middleware_codegen_\xff_".to_vec();
        name.extend_from_slice(std::process::id().to_string().as_bytes());
        let dir = std::env::temp_dir().join(OsStr::from_bytes(&name));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.yaml"), "version: \"1.0\"\nnamespace: demo\n").unwrap();
        let code = expand_model_file("m.yaml", Some(dir.join("x.rs")), span()).to_string();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(code.starts_with("compile_error !"), "{}", code);
        assert!(code.contains("not valid UTF-8"), "{}", code);
        assert!(!code.contains("pub mod demo"), "{}", code);
    }

    /// Verifies: REQ-GEN-010, REQ-GEN-012
    #[test]
    fn schema_violation_becomes_compile_error() {
        let err = expand_model_text("version: \"1.0\"\n", span())
            .unwrap_err()
            .to_string();
        assert!(err.contains("compile_error"), "{}", err);
        assert!(err.contains("model violates schema"), "{}", err);
        assert!(err.contains("namespace"), "{}", err);
    }

    /// A schema-valid model that fails a semantic check is a compile error.
    #[test]
    fn generation_failure_becomes_compile_error() {
        let yaml = "version: \"1.0\"\nnamespace: t\nmath_blocks:\n  - name: p\n    kind: pid\n    config:\n      output_min: 1.0\n      output_max: 0.0\n";
        let err = expand_model_text(yaml, span()).unwrap_err().to_string();
        assert!(err.contains("Code generation failed"), "{}", err);
        assert!(err.contains("output_min"), "{}", err);
    }

    /// Verifies: REQ-GEN-010
    #[test]
    fn valid_text_expands_to_namespace_module() {
        let code = expand_model_text("version: \"1.0\"\nnamespace: demo\n", span())
            .unwrap()
            .to_string();
        assert!(code.contains("pub mod demo"), "{}", code);
    }
}
