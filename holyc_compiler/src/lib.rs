//! Public API surface of the HolyC LLVM compiler backend.
//!
//! The `jit` feature gates the actual LLVM implementation.  Without it,
//! [`CodegenSession`] is a lightweight stub that returns an explanatory error
//! on every call, so the crate always compiles regardless of whether LLVM
//! headers are present on the host.

// `CodegenError` is always part of the public API even when the jit feature is
// disabled and callers never construct its `Disabled` variant in that build.
#![allow(unused_imports)]

pub mod abi;
mod codegen;

pub use codegen::{CodegenError, CodegenSession};
