//! Identifier validation and case conversion for generated code.
//!
//! Model files name hooks, ports and commands with plain strings. Before a
//! string becomes a Rust identifier it is checked here, so a malformed name
//! is reported as a code-generation error rather than a proc-macro panic.
//! All functions are pure.
//!
//! # Traceability
//! - REQ-GEN-011: Generated identifiers for structs, traits and enums
//! - REQ-GEN-012: Invalid names are reported as errors

use proc_macro2::Ident;
use quote::format_ident;

/// Returns `true` if `name` matches `^[a-z][a-z0-9_]*$`.
pub fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {
            chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        }
        _ => false,
    }
}

/// Returns `true` if `name` matches `^[A-Z][a-zA-Z0-9]*$`.
pub fn is_pascal_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_uppercase() => chars.all(|c| c.is_ascii_alphanumeric()),
        _ => false,
    }
}

/// Converts a snake_case name to PascalCase (`reset_fault` becomes `ResetFault`).
///
/// Underscores are dropped and the character following each one is
/// upper-cased. Names that are already PascalCase are returned unchanged.
pub fn to_pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Validates `name` as snake_case and returns it as an identifier.
///
/// # Errors
/// Returns a message naming `what` (for example "guard") if `name` is not
/// snake_case.
pub fn snake_ident(name: &str, what: &str) -> Result<Ident, String> {
    if is_snake_case(name) {
        Ok(format_ident!("{}", name))
    } else {
        Err(format!("{} name '{}' must be snake_case", what, name))
    }
}

/// Converts a snake_case or PascalCase name to a PascalCase identifier.
///
/// # Errors
/// Returns a message naming `what` if `name` is neither snake_case nor
/// PascalCase.
pub fn pascal_ident(name: &str, what: &str) -> Result<Ident, String> {
    if is_pascal_case(name) {
        Ok(format_ident!("{}", name))
    } else if is_snake_case(name) {
        Ok(format_ident!("{}", to_pascal_case(name)))
    } else {
        Err(format!("{} name '{}' must be snake_case or PascalCase", what, name))
    }
}

#[cfg(test)]
// test code: a panic is a test failure, the intended reporting mechanism
#[cfg_attr(test, allow(clippy::unwrap_used))]
mod tests {
    use super::*;

    #[test]
    fn snake_case_detection() {
        assert!(is_snake_case("enable_control"));
        assert!(is_snake_case("a1"));
        assert!(!is_snake_case("EnableControl"));
        assert!(!is_snake_case("1abc"));
        assert!(!is_snake_case(""));
        assert!(!is_snake_case("has-dash"));
    }

    #[test]
    fn pascal_case_detection() {
        assert!(is_pascal_case("ResetFault"));
        assert!(!is_pascal_case("reset_fault"));
        assert!(!is_pascal_case("Reset_Fault"));
        assert!(!is_pascal_case(""));
    }

    #[test]
    fn pascal_conversion() {
        assert_eq!(to_pascal_case("reset_fault"), "ResetFault");
        assert_eq!(to_pascal_case("arm"), "Arm");
        assert_eq!(to_pascal_case("a__b"), "AB");
        assert_eq!(to_pascal_case("Already"), "Already");
    }

    #[test]
    fn ident_validation() {
        assert!(snake_ident("ok_name", "guard").is_ok());
        assert!(snake_ident("BadName", "guard").is_err());
        assert_eq!(pascal_ident("reset_fault", "command").unwrap(), "ResetFault");
        assert_eq!(pascal_ident("Arm", "command").unwrap(), "Arm");
        assert!(pascal_ident("bad-name", "command").is_err());
    }
}
