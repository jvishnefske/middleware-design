//! # no_std build check for generated model code
//!
//! Expands `examples/attitude_controller.yaml` (path relative to this source
//! file, as `middleware_model!` resolves it) with `middleware_model!` inside
//! a `#![no_std]` crate. The check is the build itself: run
//! `cargo build -p nostd-gen --target thumbv6m-none-eabi` (RP2040,
//! Cortex-M0+). If it succeeds, the generated code needs neither `std` nor
//! `alloc` on the reference embedded target.
//!
//! The library has no runtime behaviour. `tests/platform.rs` runs that
//! cross build as a subprocess (in `target/nostd-check`, so it does not wait
//! on the outer build lock) and fails if the target is not installed. The
//! same file holds the other platform checks: the target dependency graph
//! enables no `std`/`alloc` feature, `middleware/src` uses no heap or `std`
//! in non-test code, the workspace lint policy is forced on every member,
//! and the error enums, fallible signatures and by-value state transitions
//! have the shape section 8 of `design.md` requires. Generated behaviour is
//! verified by the host test suites in `middleware/tests/`.
//!
//! # Traceability
//! - REQ-PLT-001, REQ-PLT-023: generated code builds for a `no_std` target
//! - REQ-PLT-002: no `alloc` feature or heap type on the target build
//! - REQ-PLT-020, REQ-PLT-022: workspace lint policy checks
#![no_std]

middleware_codegen::middleware_model!("../../../examples/attitude_controller.yaml");
