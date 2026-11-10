//! # Middleware Code Generation
//!
//! Proc-macro for generating Rust code from YAML model definitions.
//!
//! # Traceability
//! - REQ-GEN-010: middleware_model! macro reads YAML at compile time
//! - REQ-GEN-011: Macro generates Rust structs, traits, and implementations
//! - REQ-GEN-012: Macro validates YAML against schema and emits compile errors
//! - REQ-GEN-013: Generated code includes traceability comments and const arrays

use proc_macro::TokenStream;
use quote::quote;
use std::path::{Path, PathBuf};

use syn::{parse_macro_input, LitStr};

mod model;
mod parser;
mod generator;
mod validator;

use parser::parse_model;
use generator::generate_code;

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
/// the invoking crate whenever the model changes.
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
/// located or read, the YAML violates the schema, it cannot be parsed into
/// the model, or a semantic check in code generation rejects it.
///
/// # Traceability
/// - REQ-GEN-010: Reads YAML at compile time
/// - REQ-GEN-011: Generates structs, traits, implementations
/// - REQ-GEN-012: Validates YAML against `schema/model.schema.json` (see
///   `validator`); any violation becomes a `compile_error!` listing each
///   instance path and message. Semantic checks run afterwards.
/// - REQ-GEN-013: Includes traceability in generated code
#[proc_macro]
pub fn middleware_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    let requested = input.value();

    let path = match resolve_model_path(&requested, proc_macro::Span::call_site().local_file()) {
        Ok(path) => path,
        Err(msg) => return quote! { compile_error!(#msg); }.into(),
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
            return quote! { compile_error!(#msg); }.into();
        }
    };

    // Register the model file as a build input of the invoking crate.
    let tracked = match std::fs::canonicalize(&path) {
        Ok(canonical) => match canonical.to_str() {
            Some(abs) => quote! { const _: &str = include_str!(#abs); },
            None => quote! {},
        },
        Err(_) => quote! {},
    };

    // REQ-GEN-012: validate against the embedded JSON schema before parsing
    if let Err(e) = validator::validate_yaml(&yaml_content) {
        return syn::Error::new(input.span(), e).to_compile_error().into();
    }

    // Parse YAML into model
    let model = match parse_model(&yaml_content) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("Failed to parse YAML: {}", e);
            return quote! { compile_error!(#msg); }.into();
        }
    };

    // Generate code
    match generate_code(&model) {
        Ok(tokens) => quote! { #tracked #tokens }.into(),
        Err(e) => {
            let msg = format!("Code generation failed: {}", e);
            quote! { compile_error!(#msg); }.into()
        }
    }
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
    let yaml_content = input.value();

    // REQ-GEN-012: validate against the embedded JSON schema before parsing
    if let Err(e) = validator::validate_yaml(&yaml_content) {
        return syn::Error::new(input.span(), e).to_compile_error().into();
    }

    // Parse YAML into model
    let model = match parse_model(&yaml_content) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("Failed to parse YAML: {}", e);
            return quote! { compile_error!(#msg); }.into();
        }
    };

    // Generate code
    match generate_code(&model) {
        Ok(tokens) => tokens.into(),
        Err(e) => {
            let msg = format!("Code generation failed: {}", e);
            quote! { compile_error!(#msg); }.into()
        }
    }
}

#[cfg(test)]
// test code: a panic is a test failure, the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn relative_path_joins_invoking_file_directory() {
        let resolved =
            resolve_model_path("../fixtures/m.yaml", Some(PathBuf::from("/ws/crate/tests/t.rs")));
        assert_eq!(resolved, Ok(PathBuf::from("/ws/crate/tests/../fixtures/m.yaml")));
    }

    #[test]
    fn relative_invoking_file_stays_relative_to_rustc_cwd() {
        let resolved = resolve_model_path("m.yaml", Some(PathBuf::from("crate/src/lib.rs")));
        assert_eq!(resolved, Ok(PathBuf::from("crate/src/m.yaml")));
    }

    #[test]
    fn absolute_path_is_used_unchanged() {
        let resolved = resolve_model_path("/abs/m.yaml", Some(PathBuf::from("/ws/src/lib.rs")));
        assert_eq!(resolved, Ok(PathBuf::from("/abs/m.yaml")));
    }

    #[test]
    fn missing_invoking_file_is_an_error() {
        let err = resolve_model_path("m.yaml", None).unwrap_err();
        assert!(err.contains("no source file on disk"), "{}", err);
    }
}
