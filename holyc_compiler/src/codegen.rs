//! LLVM IR code generation for HolyC.
//!
//! This module will drive `inkwell` to produce LLVM IR from a parsed
//! [`Module`].  The structure below documents the intended design without
//! requiring `inkwell` to be present at compile time.
//!
//! # Roadmap
//!
//! 1. Wire up `inkwell` (uncomment dependency in `Cargo.toml`).
//! 2. Implement `CodegenCtx::compile_func` for simple arithmetic functions.
//! 3. Add struct / pointer support once the type system is finalised.
//! 4. Implement JIT execution via `inkwell::execution_engine`.
//! 5. Add AOT object-file emission.

// The entire module is intentional forward-scaffold: public items will be used
// once inkwell is wired in.  Suppress dead-code lints for the duration.
#![allow(dead_code)]

use holyc_frontend::ast::Module;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodegenError {
    #[error("unsupported feature: {0}")]
    Unsupported(String),

    #[error("type error during codegen: {0}")]
    Type(String),

    #[error("LLVM error: {0}")]
    Llvm(String),
}

/// Code-generation context.
///
/// When `inkwell` is enabled this will own the `inkwell::context::Context`
/// and `inkwell::module::Module`.
pub struct CodegenCtx {
    // context: inkwell::context::Context,
    // module:  inkwell::module::Module<'ctx>,
    // builder: inkwell::builder::Builder<'ctx>,
}

impl CodegenCtx {
    pub fn new(_module_name: &str) -> Self {
        // let context = inkwell::context::Context::create();
        // let module  = context.create_module(module_name);
        // let builder = context.create_builder();
        CodegenCtx {}
    }

    /// Compile a parsed HolyC module to LLVM IR.
    pub fn compile(&mut self, _module: &Module) -> Result<(), CodegenError> {
        Err(CodegenError::Unsupported(
            "LLVM backend not yet active — enable the `inkwell` feature".into(),
        ))
    }

    /// Emit LLVM IR text to a string.
    pub fn emit_ir(&self) -> String {
        // self.module.print_to_string().to_string()
        String::from("; LLVM IR not yet generated\n")
    }
}
