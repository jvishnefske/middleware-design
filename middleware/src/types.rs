//! # Type System
//!
//! Type-safe wrappers for physical units, primitives, and bounded collections.
//!
//! # Traceability
//!
//! - REQ-TYP-001, REQ-TYP-002, REQ-TYP-003, REQ-TYP-004: Physical units
//! - REQ-TYP-010, REQ-TYP-011, REQ-TYP-012: Primitive wrappers
//! - REQ-TYP-020, REQ-TYP-021, REQ-TYP-022: Bounded collections

pub mod unit;
pub mod primitive;
pub mod bounded;

// Re-export key types
pub use unit::*;
pub use primitive::*;
pub use bounded::*;
