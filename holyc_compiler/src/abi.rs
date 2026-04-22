//! ABI (calling convention) layer.
//!
//! HolyC on TempleOS used a custom calling convention that passed the first
//! few arguments in registers on x86-64 (similar to System V AMD64 ABI but
//! with some TempleOS-specific quirks).  This module documents those
//! conventions and will provide the translation to LLVM's calling conventions
//! once `inkwell` is wired in.
//!
//! # HolyC calling convention notes (x86-64 / TempleOS)
//!
//! * Integer / pointer arguments: passed in RDI, RSI, RDX, RCX, R8, R9
//!   (matching System V AMD64).
//! * Floating-point arguments: XMM0–XMM7 (matching System V AMD64).
//! * Return value: RAX (integer) or XMM0 (float).
//! * Stack is 16-byte aligned at the call site.
//! * No red zone (TempleOS kernel code could interrupt at any point).
//!
//! For host portability (Linux / macOS / Windows) HolyComp maps to the
//! platform's native ABI so that compiled code can call into Rust builtins
//! without a trampoline.

// Public API — will be consumed by codegen.rs once inkwell is wired in.
#![allow(dead_code)]

use holyc_frontend::types::HolyType;

/// Classification of how a value is passed according to the ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgClass {
    /// Passed in a general-purpose register / return in RAX.
    Integer,
    /// Passed in an SSE register / return in XMM0.
    Sse,
    /// Passed on the stack.
    Memory,
    /// `U0` / void — no value passed or returned.
    Void,
}

/// Classify a [`HolyType`] for the calling convention.
pub fn classify(ty: &HolyType) -> ArgClass {
    match ty {
        HolyType::Void => ArgClass::Void,
        HolyType::F32 | HolyType::F64 => ArgClass::Sse,
        HolyType::Ptr(_) | HolyType::FnPtr { .. } => ArgClass::Integer,
        _ if ty.is_integer() || *ty == HolyType::Bool => ArgClass::Integer,
        HolyType::Array { .. } | HolyType::Named(_) => ArgClass::Memory,
        _ => ArgClass::Memory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_integers() {
        assert_eq!(classify(&HolyType::I64), ArgClass::Integer);
        assert_eq!(classify(&HolyType::U8), ArgClass::Integer);
        assert_eq!(classify(&HolyType::Bool), ArgClass::Integer);
    }

    #[test]
    fn classify_floats() {
        assert_eq!(classify(&HolyType::F32), ArgClass::Sse);
        assert_eq!(classify(&HolyType::F64), ArgClass::Sse);
    }

    #[test]
    fn classify_void() {
        assert_eq!(classify(&HolyType::Void), ArgClass::Void);
    }

    #[test]
    fn classify_pointer() {
        assert_eq!(
            classify(&HolyType::Ptr(Box::new(HolyType::I64))),
            ArgClass::Integer
        );
    }
}
