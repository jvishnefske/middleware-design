//! # no_std build check for generated model code
//!
//! Expands `examples/attitude_controller.yaml` (path relative to this source
//! file, as `middleware_model!` resolves it) with `middleware_model!` inside
//! a `#![no_std]` crate. The check is the build itself: run
//! `cargo build -p nostd-gen --target thumbv6m-none-eabi` (RP2040,
//! Cortex-M0+). If it succeeds, the generated code needs neither `std` nor
//! `alloc` on the reference embedded target.
//!
//! The crate has no runtime behaviour and no tests. Generated behaviour is
//! verified by the host test suites in `middleware/tests/`.
//!
//! # Traceability
//! - REQ-PLT-001: generated code builds for a `no_std` target
#![no_std]

middleware_codegen::middleware_model!("../../../examples/attitude_controller.yaml");
