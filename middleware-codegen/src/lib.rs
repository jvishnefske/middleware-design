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
/// ```ignore
/// middleware_model!("path/to/model.yaml");
/// ```
///
/// # Generated Code
///
/// The YAML is first checked against `schema/model.schema.json`; any
/// violation becomes a compile error naming its JSON pointer path.
///
/// The macro generates:
/// - Enums with an explicit `#[repr]` for states and events
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
    let path = input.value();

    // Read YAML file at compile time
    let yaml_content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) => {
            let msg = format!("Failed to read YAML file '{}': {}", path, e);
            return quote! { compile_error!(#msg); }.into();
        }
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
        Ok(tokens) => tokens.into(),
        Err(e) => {
            let msg = format!("Code generation failed: {}", e);
            quote! { compile_error!(#msg); }.into()
        }
    }
}

/// Generates code from inline YAML content.
///
/// # Usage
///
/// ```ignore
/// middleware_model_inline! {
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
