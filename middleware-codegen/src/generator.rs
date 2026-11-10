//! Code generator for Rust code from model definitions.
//!
//! [`generate_code`] turns a validated [`Model`] into one `pub mod <namespace>`
//! containing, in order: traceability constants, enums, structs, messages,
//! `PortMessage` implementations for port message types, math block
//! constants, state machine modules and components.
//!
//! Generation is pure: each section is built by a function from model data
//! to tokens, and any inconsistency in the model (unknown references,
//! invalid names, contradictory limits) is returned as `Err(String)`, which
//! the macro turns into a `compile_error!`.
//!
//! Submodules:
//! - `naming`: identifier validation and case conversion.
//! - `state_machine`: transition tables, `Hooks` traits and `step` functions.
//! - `component`: component structs, typed ports and `PortMessage` impls.
//!
//! # Traceability
//! - REQ-GEN-011: Generates Rust structs, traits, implementations
//! - REQ-GEN-013: Generated code includes traceability comments
//! - REQ-FSM-002, REQ-FSM-011, REQ-FSM-012: Generated state machine hooks

mod component;
mod naming;
mod state_machine;

use proc_macro2::{Literal, TokenStream};
use quote::{format_ident, quote};

use crate::model::{EnumDef, Field, MathBlock, MessageDef, Model, StructDef};
use crate::parser::validate_model;

/// Generates Rust code from a model.
///
/// # Parameters
/// - `model`: The parsed model
///
/// # Returns
/// `Ok(TokenStream)` with generated code, `Err(String)` on failure
///
/// # Errors
/// Returns an error if the model fails [`validate_model`] or any section
/// generator rejects it.
///
/// # Traceability
/// - REQ-GEN-011: Generates structs, traits, implementations
/// - REQ-GEN-013: Includes traceability in generated code
pub fn generate_code(model: &Model) -> Result<TokenStream, String> {
    validate_model(model)?;

    let namespace = format_ident!("{}", model.namespace);

    let enum_code = generate_enums(&model.types.enums);
    let struct_code = generate_structs(&model.types.structs);
    let message_code = generate_messages(&model.messages);
    let port_message_code = component::generate_port_messages(model)?;
    let math_block_code = generate_math_blocks(&model.math_blocks)?;
    let state_machine_code =
        state_machine::generate_state_machines(&model.state_machines, &model.types.enums)?;
    let component_code = component::generate_components(model)?;
    let traceability_code = generate_traceability(model);

    Ok(quote! {
        /// Generated module for namespace: #namespace
        ///
        /// This code was generated from a YAML model file.
        /// Do not edit manually - changes will be overwritten.
        pub mod #namespace {
            #traceability_code
            #enum_code
            #struct_code
            #message_code
            #port_message_code
            #math_block_code
            #state_machine_code
            #component_code
        }
    })
}

/// Returns the first item that repeats an earlier one under `same`.
///
/// The scan is in order; the returned item is the later of the two equal
/// items (the first whose predecessor list already contains an equal item).
/// Uses only iterators, so it cannot index out of bounds.
fn first_duplicate<T>(items: &[T], same: impl Fn(&T, &T) -> bool) -> Option<&T> {
    items
        .iter()
        .enumerate()
        .find(|(i, item)| items.iter().take(*i).any(|earlier| same(earlier, item)))
        .map(|(_, item)| item)
}

/// Builds a `#[doc = "Traces: ..."]` attribute, or nothing if `traces` is empty.
fn traces_doc(traces: &[String]) -> TokenStream {
    if traces.is_empty() {
        quote! {}
    } else {
        let doc = format!("Traces: {}", traces.join(", "));
        quote! { #[doc = #doc] }
    }
}

/// Generates enum definitions.
///
/// Each enum gets explicit discriminants `0..COUNT` in declaration order, the
/// `COUNT` and `ALL` constants, a `const fn to_repr(self)` returning the
/// discriminant as the `repr` type through an exhaustive `match` (no `as`
/// cast), and `impl From<Enum> for <repr>` delegating to it.
///
/// # Traceability
/// - REQ-FSM-001: States as enums with repr
fn generate_enums(enums: &[EnumDef]) -> TokenStream {
    let enum_tokens: Vec<TokenStream> = enums.iter().map(|e| {
        let name = format_ident!("{}", e.name);
        let repr = format_ident!("{}", e.repr);
        let traces = traces_doc(&e.traces);

        let variants: Vec<TokenStream> = e.variants.iter().enumerate().map(|(i, v)| {
            let variant_name = format_ident!("{}", v);
            let idx = Literal::usize_unsuffixed(i);
            quote! { #variant_name = #idx }
        }).collect();

        let variant_count = e.variants.len();
        let variant_names: Vec<_> = e.variants.iter().map(|v| format_ident!("{}", v)).collect();
        let discriminants: Vec<Literal> =
            (0..variant_count).map(Literal::usize_unsuffixed).collect();
        let to_repr_doc = format!("Discriminant of `self` as `{}`, without an `as` cast.", e.repr);

        quote! {
            #traces
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            #[repr(#repr)]
            pub enum #name {
                #(#variants),*
            }

            impl #name {
                /// Number of variants in this enum.
                pub const COUNT: usize = #variant_count;

                /// All variants as a const array.
                pub const ALL: [Self; #variant_count] = [
                    #(Self::#variant_names),*
                ];

                #[doc = #to_repr_doc]
                pub const fn to_repr(self) -> #repr {
                    match self {
                        #(Self::#variant_names => #discriminants),*
                    }
                }
            }

            impl ::core::convert::From<#name> for #repr {
                fn from(value: #name) -> Self {
                    value.to_repr()
                }
            }
        }
    }).collect();

    quote! { #(#enum_tokens)* }
}

/// Generates struct definitions.
fn generate_structs(structs: &[StructDef]) -> TokenStream {
    let struct_tokens: Vec<TokenStream> = structs.iter().map(|s| {
        let name = format_ident!("{}", s.name);
        let traces = traces_doc(&s.traces);

        let fields = generate_fields(&s.fields);

        quote! {
            #traces
            #[derive(Debug, Clone, Copy, PartialEq)]
            pub struct #name {
                #fields
            }
        }
    }).collect();

    quote! { #(#struct_tokens)* }
}

/// Generates field definitions.
fn generate_fields(fields: &[Field]) -> TokenStream {
    let field_tokens: Vec<TokenStream> = fields.iter().map(|f| {
        let name = format_ident!("{}", f.name);
        let ty = parse_type(&f.field_type);
        quote! { pub #name: #ty }
    }).collect();

    quote! { #(#field_tokens),* }
}

/// Parses a type string into a TokenStream.
fn parse_type(type_str: &str) -> TokenStream {
    match type_str {
        "f32" => quote! { f32 },
        "f64" => quote! { f64 },
        "i8" => quote! { i8 },
        "i16" => quote! { i16 },
        "i32" => quote! { i32 },
        "i64" => quote! { i64 },
        "u8" => quote! { u8 },
        "u16" => quote! { u16 },
        "u32" => quote! { u32 },
        "u64" => quote! { u64 },
        "bool" => quote! { bool },
        other => {
            let ident = format_ident!("{}", other);
            quote! { #ident }
        }
    }
}

/// Generates message definitions.
fn generate_messages(messages: &[MessageDef]) -> TokenStream {
    let msg_tokens: Vec<TokenStream> = messages.iter().map(|m| {
        let name = format_ident!("{}", m.name);
        let traces = traces_doc(&m.traces);

        let fields = generate_fields(&m.fields);

        quote! {
            #traces
            #[derive(Debug, Clone, Copy, PartialEq)]
            pub struct #name {
                #fields
            }
        }
    }).collect();

    quote! { #(#msg_tokens)* }
}

/// Generates math block configurations.
///
/// Each block becomes a module of `f32` constants named after the block in
/// upper case. PID blocks emit `KP`, `KI`, `KD`, `OUTPUT_MIN`, `OUTPUT_MAX`,
/// `INTEGRAL_MIN` and `INTEGRAL_MAX`; unset values default to gains 1/0/0,
/// output limits -1..1, `integral_max` 1 and `integral_min` equal to
/// `-integral_max` (symmetric anti-windup, matching the runtime
/// `PidLimits`). Filter blocks emit their frequency constants. Blocks of
/// kind `state_space` produce no code yet.
///
/// # Errors
/// Returns an error if a PID block has `output_min > output_max` or
/// `integral_min > integral_max`.
///
/// # Traceability
/// - REQ-GEN-003: Math block configuration
fn generate_math_blocks(blocks: &[MathBlock]) -> Result<TokenStream, String> {
    let block_tokens = blocks
        .iter()
        .map(generate_math_block)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#block_tokens)* })
}

/// Generates the constants module for one math block.
fn generate_math_block(b: &MathBlock) -> Result<TokenStream, String> {
    let name = format_ident!("{}", b.name.to_uppercase());
    let traces = traces_doc(&b.traces);

    match b.kind.as_str() {
        "pid" => {
            let kp = b.config.kp.unwrap_or(1.0);
            let ki = b.config.ki.unwrap_or(0.0);
            let kd = b.config.kd.unwrap_or(0.0);
            let output_min = b.config.output_min.unwrap_or(-1.0);
            let output_max = b.config.output_max.unwrap_or(1.0);
            let integral_max = b.config.integral_max.unwrap_or(1.0);
            let integral_min = b.config.integral_min.unwrap_or(-integral_max);
            if output_min > output_max {
                return Err(format!(
                    "Math block '{}': output_min {} exceeds output_max {}",
                    b.name, output_min, output_max
                ));
            }
            if integral_min > integral_max {
                return Err(format!(
                    "Math block '{}': integral_min {} exceeds integral_max {}",
                    b.name, integral_min, integral_max
                ));
            }

            Ok(quote! {
                #traces
                /// PID controller configuration.
                pub mod #name {
                    /// Proportional gain.
                    pub const KP: f32 = #kp;
                    /// Integral gain.
                    pub const KI: f32 = #ki;
                    /// Derivative gain.
                    pub const KD: f32 = #kd;
                    /// Minimum output value.
                    pub const OUTPUT_MIN: f32 = #output_min;
                    /// Maximum output value.
                    pub const OUTPUT_MAX: f32 = #output_max;
                    /// Minimum integral value (anti-windup).
                    pub const INTEGRAL_MIN: f32 = #integral_min;
                    /// Maximum integral value (anti-windup).
                    pub const INTEGRAL_MAX: f32 = #integral_max;
                }
            })
        }
        "low_pass" | "high_pass" => {
            let cutoff_hz = b.config.cutoff_hz.unwrap_or(10.0);
            let sample_rate_hz = b.config.sample_rate_hz.unwrap_or(100.0);
            let doc = format!("{} filter configuration.", b.kind);

            Ok(quote! {
                #traces
                #[doc = #doc]
                pub mod #name {
                    /// Cutoff frequency in Hz.
                    pub const CUTOFF_HZ: f32 = #cutoff_hz;
                    /// Sample rate in Hz.
                    pub const SAMPLE_RATE_HZ: f32 = #sample_rate_hz;
                }
            })
        }
        "notch" => {
            let notch_hz = b.config.notch_hz.unwrap_or(b.config.cutoff_hz.unwrap_or(60.0));
            let bandwidth_hz = b.config.bandwidth_hz.unwrap_or(5.0);
            let sample_rate_hz = b.config.sample_rate_hz.unwrap_or(100.0);

            Ok(quote! {
                #traces
                /// Notch filter configuration.
                pub mod #name {
                    /// Notch frequency in Hz.
                    pub const NOTCH_HZ: f32 = #notch_hz;
                    /// Bandwidth in Hz.
                    pub const BANDWIDTH_HZ: f32 = #bandwidth_hz;
                    /// Sample rate in Hz.
                    pub const SAMPLE_RATE_HZ: f32 = #sample_rate_hz;
                }
            })
        }
        _ => Ok(quote! {}),
    }
}

/// Generates traceability information.
///
/// # Traceability
/// - REQ-GEN-004: Traceability support
/// - REQ-GEN-013: Traceability in generated code
fn generate_traceability(model: &Model) -> TokenStream {
    if model.requirements.is_empty() {
        return quote! {};
    }

    let req_count = model.requirements.len();
    let req_ids: Vec<&str> = model.requirements.iter().map(|r| r.id.as_str()).collect();
    let req_descriptions: Vec<&str> = model.requirements.iter().map(|r| r.description.as_str()).collect();
    let req_levels: Vec<&str> = model.requirements.iter().map(|r| r.level.as_str()).collect();

    quote! {
        /// Traceability information for this module.
        pub mod traceability {
            /// Number of requirements.
            pub const REQUIREMENT_COUNT: usize = #req_count;

            /// Requirement IDs.
            pub const REQUIREMENT_IDS: [&str; #req_count] = [
                #(#req_ids),*
            ];

            /// Requirement descriptions.
            pub const REQUIREMENT_DESCRIPTIONS: [&str; #req_count] = [
                #(#req_descriptions),*
            ];

            /// Requirement levels (DO-178C DAL).
            pub const REQUIREMENT_LEVELS: [&str; #req_count] = [
                #(#req_levels),*
            ];
        }
    }
}

#[cfg(test)]
// test code: a panic is a test failure, the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;
    use crate::parser::parse_model;

    #[test]
    fn test_generate_enum() {
        let yaml = r#"
version: "1.0"
namespace: "test"
types:
  enums:
    - name: MyState
      repr: u8
      variants:
        - Idle
        - Running
"#;
        let model = parse_model(yaml).unwrap();
        let code = generate_code(&model).unwrap();
        let code_str = code.to_string();
        assert!(code_str.contains("pub enum MyState"));
        assert!(code_str.contains("Idle"));
        assert!(code_str.contains("Running"));
    }

    #[test]
    fn test_generate_struct() {
        let yaml = r#"
version: "1.0"
namespace: "test"
types:
  structs:
    - name: MyData
      fields:
        - name: value
          type: f32
        - name: count
          type: u32
"#;
        let model = parse_model(yaml).unwrap();
        let code = generate_code(&model).unwrap();
        let code_str = code.to_string();
        assert!(code_str.contains("pub struct MyData"));
        assert!(code_str.contains("value"));
        assert!(code_str.contains("f32"));
    }

    #[test]
    fn first_duplicate_returns_later_of_equal_items() {
        let same = |a: &(u8, char), b: &(u8, char)| a.0 == b.0;
        assert_eq!(first_duplicate(&[(1, 'a'), (2, 'b'), (1, 'c')], same), Some(&(1, 'c')));
        assert_eq!(first_duplicate(&[(1, 'a'), (2, 'b')], same), None);
        assert_eq!(first_duplicate::<(u8, char)>(&[], same), None);
    }

    #[test]
    fn enum_conversion_is_a_match_not_a_cast() {
        let yaml = r#"
version: "1.0"
namespace: "test"
types:
  enums:
    - name: Mode
      repr: u16
      variants: [Off, On]
"#;
        let model = parse_model(yaml).unwrap();
        let code = generate_code(&model).unwrap().to_string();
        assert!(code.contains("pub const fn to_repr (self) -> u16"), "{}", code);
        assert!(code.contains("Self :: On => 1"), "{}", code);
        assert!(code.contains("From < Mode > for u16"), "{}", code);
        // Doc strings mention "as"; an actual cast would render as `as u16`.
        assert!(!code.contains(" as u16 ") && !code.contains(" as usize "), "{}", code);
    }

    #[test]
    fn test_pid_inverted_integral_limits_rejected() {
        let yaml = r#"
version: "1.0"
namespace: "test"
math_blocks:
  - name: p
    kind: pid
    config:
      integral_min: 0.5
      integral_max: 0.1
"#;
        let model = parse_model(yaml).unwrap();
        let err = generate_code(&model).unwrap_err();
        assert!(err.contains("integral_min"), "{}", err);
    }

    #[test]
    fn test_pid_inverted_output_limits_rejected() {
        let yaml = r#"
version: "1.0"
namespace: "test"
math_blocks:
  - name: p
    kind: pid
    config:
      output_min: 1.0
      output_max: -1.0
"#;
        let model = parse_model(yaml).unwrap();
        let err = generate_code(&model).unwrap_err();
        assert!(err.contains("output_min"), "{}", err);
    }
}
