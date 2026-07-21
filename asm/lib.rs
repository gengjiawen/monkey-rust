//! AOT arm64 (AArch64) assembly backend for Monkey.
//!
//! Design: docs/arm64-asm-backend-design.md. The crate is built twice:
//! the host build exposes `emitter`/`lower` (pure functions producing `.s`
//! text) plus the shared `runtime_core`; the aarch64 cross build additionally
//! provides the `extern "C"` runtime the generated assembly links against.

pub mod emitter;
pub mod lower;
pub mod runtime_backend;
pub mod runtime_core;

#[cfg(not(target_family = "wasm"))]
pub mod runtime;

#[cfg(test)]
mod e2e_test;
#[cfg(test)]
mod emitter_test;
#[cfg(test)]
mod lower_test;
#[cfg(test)]
mod runtime_core_test;
