//! LLVM IR code generation for HolyC.
//!
//! Guarded behind the `jit` Cargo feature so the crate compiles on hosts
//! without LLVM headers.  Enable with `--features jit` (requires llvm-17-dev).

// ── Feature-off stub ─────────────────────────────────────────────────────────

#[cfg(not(feature = "jit"))]
#[allow(unused_imports)]
pub use stub::{CodegenError, CodegenSession};

#[cfg(not(feature = "jit"))]
mod stub {
    use holyc_frontend::{ast::Module, TypeEnv};
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum CodegenError {
        #[error("LLVM JIT backend disabled — rebuild with `--features jit`")]
        Disabled,
    }

    pub struct CodegenSession;

    impl CodegenSession {
        pub fn new() -> Self {
            Self
        }
        pub fn emit_ir(&self, _: &str, _: &Module, _: TypeEnv) -> Result<String, CodegenError> {
            Err(CodegenError::Disabled)
        }
        pub fn jit_run(&self, _: &str, _: &Module, _: TypeEnv) -> Result<(), CodegenError> {
            Err(CodegenError::Disabled)
        }
        pub fn emit_object(
            &self,
            _: &str,
            _: &Module,
            _: TypeEnv,
            _: &std::path::Path,
            _: Option<&str>,
            _: u8,
        ) -> Result<(), CodegenError> {
            Err(CodegenError::Disabled)
        }
        pub fn emit_asm_file(
            &self,
            _: &str,
            _: &Module,
            _: TypeEnv,
            _: &std::path::Path,
            _: Option<&str>,
            _: u8,
        ) -> Result<(), CodegenError> {
            Err(CodegenError::Disabled)
        }
        pub fn emit_executable(
            &self,
            _: &str,
            _: &Module,
            _: TypeEnv,
            _: &std::path::Path,
            _: Option<&str>,
            _: u8,
        ) -> Result<(), CodegenError> {
            Err(CodegenError::Disabled)
        }
    }
    impl Default for CodegenSession {
        fn default() -> Self {
            Self::new()
        }
    }
}

// ── Full LLVM implementation ──────────────────────────────────────────────────

#[cfg(feature = "jit")]
#[allow(unused_imports)]
pub use full::{CodegenError, CodegenSession};

#[cfg(feature = "jit")]
mod full {
    use std::collections::HashMap;

    use holyc_frontend::{
        ast::{AssignOp, BinOp, ExprKind, Module, StmtKind, TopLevelKind, UnaryOp},
        types::HolyType,
        TypeEnv,
    };
    use inkwell::targets::{
        CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
    };
    use inkwell::{
        builder::{Builder, BuilderError},
        context::Context,
        module::Module as LlvmModule,
        types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum},
        values::{
            BasicMetadataValueEnum, BasicValueEnum, FunctionValue, GlobalValue, PointerValue,
        },
        AddressSpace, FloatPredicate, IntPredicate, OptimizationLevel,
    };
    use std::path::Path;
    use thiserror::Error;

    // ── Error ─────────────────────────────────────────────────────────────────

    #[derive(Error, Debug)]
    pub enum CodegenError {
        #[error("unsupported: {0}")]
        Unsupported(String),
        #[error("type error: {0}")]
        Type(String),
        #[error("LLVM error: {0}")]
        Llvm(String),
        #[error("undefined: {0}")]
        Undefined(String),
    }

    impl From<BuilderError> for CodegenError {
        fn from(e: BuilderError) -> Self {
            Self::Llvm(e.to_string())
        }
    }

    // ── Expression result ─────────────────────────────────────────────────────

    #[derive(Clone)]
    enum Operand<'ctx> {
        Int(inkwell::values::IntValue<'ctx>),
        Float(inkwell::values::FloatValue<'ctx>),
        Ptr(PointerValue<'ctx>),
        Void,
    }

    impl<'ctx> Operand<'ctx> {
        fn as_meta(&self) -> Option<BasicMetadataValueEnum<'ctx>> {
            match self {
                Self::Int(v) => Some((*v).into()),
                Self::Float(v) => Some((*v).into()),
                Self::Ptr(v) => Some((*v).into()),
                Self::Void => None,
            }
        }
        fn as_basic(&self) -> Option<BasicValueEnum<'ctx>> {
            match self {
                Self::Int(v) => Some((*v).into()),
                Self::Float(v) => Some((*v).into()),
                Self::Ptr(v) => Some((*v).into()),
                Self::Void => None,
            }
        }
    }

    // ── Codegen context ───────────────────────────────────────────────────────

    struct Cg<'ctx> {
        ctx: &'ctx Context,
        module: LlvmModule<'ctx>,
        builder: Builder<'ctx>,
        type_env: TypeEnv,
        /// name → (llvm fn, holyc return type)
        fn_table: HashMap<String, (FunctionValue<'ctx>, HolyType)>,
        /// name → (alloca, holyc type)
        locals: HashMap<String, (PointerValue<'ctx>, HolyType)>,
        /// name → (global, holyc type)
        globals: HashMap<String, (GlobalValue<'ctx>, HolyType)>,
        /// (break_bb, continue_bb) stack
        loop_stack: Vec<(
            inkwell::basic_block::BasicBlock<'ctx>,
            inkwell::basic_block::BasicBlock<'ctx>,
        )>,
    }

    impl<'ctx> Cg<'ctx> {
        fn new(ctx: &'ctx Context, name: &str, type_env: TypeEnv) -> Self {
            Self {
                ctx,
                module: ctx.create_module(name),
                builder: ctx.create_builder(),
                type_env,
                fn_table: HashMap::new(),
                locals: HashMap::new(),
                globals: HashMap::new(),
                loop_stack: Vec::new(),
            }
        }

        // ── Type mapping ──────────────────────────────────────────────────────

        fn llty(&self, ty: &HolyType) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
            Ok(match self.type_env.resolve(ty) {
                HolyType::I8 | HolyType::U8 => self.ctx.i8_type().into(),
                HolyType::I16 | HolyType::U16 => self.ctx.i16_type().into(),
                HolyType::I32 | HolyType::U32 => self.ctx.i32_type().into(),
                HolyType::I64 | HolyType::U64 => self.ctx.i64_type().into(),
                HolyType::F32 => self.ctx.f32_type().into(),
                HolyType::F64 => self.ctx.f64_type().into(),
                HolyType::Bool => self.ctx.bool_type().into(),
                HolyType::Ptr(_) | HolyType::FnPtr { .. } => {
                    self.ctx.i8_type().ptr_type(AddressSpace::default()).into()
                },
                HolyType::Named(ref n) => {
                    let sz = self
                        .type_env
                        .structs
                        .get(n)
                        .map(|l| l.size as u32)
                        .ok_or_else(|| CodegenError::Type(format!("unknown struct `{n}`")))?;
                    self.ctx.i8_type().array_type(sz).into()
                },
                HolyType::Array { elem, len } => {
                    let n = len.unwrap_or(0) as u32;
                    match self.llty(&elem)? {
                        BasicTypeEnum::IntType(t) => t.array_type(n).into(),
                        BasicTypeEnum::FloatType(t) => t.array_type(n).into(),
                        _ => return Err(CodegenError::Unsupported("array of complex type".into())),
                    }
                },
                HolyType::Void => return Err(CodegenError::Type("void has no BasicType".into())),
            })
        }

        fn llty_meta(&self, ty: &HolyType) -> Result<BasicMetadataTypeEnum<'ctx>, CodegenError> {
            if matches!(self.type_env.resolve(ty), HolyType::Named(_)) {
                return Ok(self.ctx.i8_type().ptr_type(AddressSpace::default()).into());
            }
            Ok(match self.llty(ty)? {
                BasicTypeEnum::IntType(t) => t.into(),
                BasicTypeEnum::FloatType(t) => t.into(),
                BasicTypeEnum::PointerType(t) => t.into(),
                BasicTypeEnum::ArrayType(t) => t.into(),
                t => return Err(CodegenError::Type(format!("bad param type: {t:?}"))),
            })
        }

        fn is_float(&self, ty: &HolyType) -> bool {
            matches!(self.type_env.resolve(ty), HolyType::F32 | HolyType::F64)
        }

        // ── Extern declarations ───────────────────────────────────────────────

        fn decl_printf(&self) -> FunctionValue<'ctx> {
            self.module.get_function("printf").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "printf",
                    self.ctx.i32_type().fn_type(&[ptr.into()], true),
                    None,
                )
            })
        }
        fn decl_malloc(&self) -> FunctionValue<'ctx> {
            self.module.get_function("malloc").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "malloc",
                    ptr.fn_type(&[self.ctx.i64_type().into()], false),
                    None,
                )
            })
        }
        fn decl_free(&self) -> FunctionValue<'ctx> {
            self.module.get_function("free").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "free",
                    self.ctx.void_type().fn_type(&[ptr.into()], false),
                    None,
                )
            })
        }
        fn decl_memset(&self) -> FunctionValue<'ctx> {
            self.module.get_function("memset").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "memset",
                    ptr.fn_type(
                        &[
                            ptr.into(),
                            self.ctx.i32_type().into(),
                            self.ctx.i64_type().into(),
                        ],
                        false,
                    ),
                    None,
                )
            })
        }
        fn decl_memcpy(&self) -> FunctionValue<'ctx> {
            self.module.get_function("memcpy").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "memcpy",
                    ptr.fn_type(&[ptr.into(), ptr.into(), self.ctx.i64_type().into()], false),
                    None,
                )
            })
        }
        fn decl_strlen(&self) -> FunctionValue<'ctx> {
            self.module.get_function("strlen").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "strlen",
                    self.ctx.i64_type().fn_type(&[ptr.into()], false),
                    None,
                )
            })
        }
        fn decl_strcmp(&self) -> FunctionValue<'ctx> {
            self.module.get_function("strcmp").unwrap_or_else(|| {
                let ptr = self.ctx.i8_type().ptr_type(AddressSpace::default());
                self.module.add_function(
                    "strcmp",
                    self.ctx
                        .i32_type()
                        .fn_type(&[ptr.into(), ptr.into()], false),
                    None,
                )
            })
        }
        fn decl_f64_1(&self, name: &str) -> FunctionValue<'ctx> {
            self.module.get_function(name).unwrap_or_else(|| {
                let f = self.ctx.f64_type();
                self.module
                    .add_function(name, f.fn_type(&[f.into()], false), None)
            })
        }
        fn decl_pow(&self) -> FunctionValue<'ctx> {
            self.module.get_function("pow").unwrap_or_else(|| {
                let f = self.ctx.f64_type();
                self.module
                    .add_function("pow", f.fn_type(&[f.into(), f.into()], false), None)
            })
        }
        fn decl_exit(&self) -> FunctionValue<'ctx> {
            self.module.get_function("exit").unwrap_or_else(|| {
                self.module.add_function(
                    "exit",
                    self.ctx
                        .void_type()
                        .fn_type(&[self.ctx.i32_type().into()], false),
                    None,
                )
            })
        }

        fn const_str(&self, s: &str) -> PointerValue<'ctx> {
            let arr = self.ctx.const_string(s.as_bytes(), true);
            let g = self.module.add_global(arr.get_type(), None, ".str");
            g.set_initializer(&arr);
            g.set_constant(true);
            g.set_linkage(inkwell::module::Linkage::Private);
            g.as_pointer_value()
        }

        // ── Module passes ─────────────────────────────────────────────────────

        fn compile_module(&mut self, ast: &Module) -> Result<(), CodegenError> {
            // Pass 0 – register struct/typedef.
            for item in &ast.items {
                match &item.kind {
                    TopLevelKind::ClassDef { name, fields } => {
                        let raw: Vec<_> = fields
                            .iter()
                            .map(|f| (f.name.clone(), f.ty.clone()))
                            .collect();
                        self.type_env.compute_layout(name.clone(), &raw);
                    },
                    TopLevelKind::TypeDef { ty, alias } => {
                        self.type_env.add_typedef(alias.clone(), ty.clone());
                    },
                    _ => {},
                }
            }

            // Pass 1 – forward-declare functions.
            for item in &ast.items {
                match &item.kind {
                    TopLevelKind::FuncDef {
                        ret_ty,
                        name,
                        params,
                        ..
                    }
                    | TopLevelKind::FuncDecl {
                        ret_ty,
                        name,
                        params,
                    } => {
                        if self.fn_table.contains_key(name) {
                            continue;
                        }
                        let pts: Vec<BasicMetadataTypeEnum<'ctx>> = params
                            .iter()
                            .map(|p| self.llty_meta(&p.ty))
                            .collect::<Result<_, _>>()?;
                        let fnt = if matches!(self.type_env.resolve(ret_ty), HolyType::Void) {
                            self.ctx.void_type().fn_type(&pts, false)
                        } else {
                            self.llty(ret_ty)?.fn_type(&pts, false)
                        };
                        let fv = self.module.add_function(name, fnt, None);
                        self.fn_table.insert(name.clone(), (fv, ret_ty.clone()));
                    },
                    _ => {},
                }
            }

            // Pass 2 – global variables.
            for item in &ast.items {
                if let TopLevelKind::GlobalVar { ty, name, init, .. } = &item.kind {
                    if matches!(self.type_env.resolve(ty), HolyType::Void) {
                        continue;
                    }
                    let llty = self.llty(ty)?;
                    let g = self.module.add_global(llty, None, name);
                    g.set_linkage(inkwell::module::Linkage::Internal);
                    let zero: BasicValueEnum = match llty {
                        BasicTypeEnum::IntType(t) => t.const_zero().into(),
                        BasicTypeEnum::FloatType(t) => t.const_zero().into(),
                        _ => llty.into_int_type().const_zero().into(),
                    };
                    g.set_initializer(&zero);
                    // Propagate simple constant initialisers.
                    if let Some(e) = init {
                        match &e.kind {
                            ExprKind::IntLit(n) => {
                                if let BasicTypeEnum::IntType(t) = llty {
                                    g.set_initializer(&t.const_int(*n, false));
                                }
                            },
                            ExprKind::FloatLit(f) => {
                                if let BasicTypeEnum::FloatType(t) = llty {
                                    g.set_initializer(&t.const_float(*f));
                                }
                            },
                            _ => {},
                        }
                    }
                    self.globals.insert(name.clone(), (g, ty.clone()));
                }
            }

            // Pass 3 – function bodies.
            for item in &ast.items {
                if let TopLevelKind::FuncDef {
                    name, params, body, ..
                } = &item.kind
                {
                    let (fv, _) = self.fn_table[name];
                    self.locals.clear();
                    let entry = self.ctx.append_basic_block(fv, "entry");
                    self.builder.position_at_end(entry);

                    for (i, p) in params.iter().enumerate() {
                        let llty = self.llty(&p.ty)?;
                        let slot = self.builder.build_alloca(llty, &p.name)?;
                        self.builder
                            .build_store(slot, fv.get_nth_param(i as u32).unwrap())?;
                        self.locals.insert(p.name.clone(), (slot, p.ty.clone()));
                    }

                    self.emit_stmts(fv, body)?;

                    if self
                        .builder
                        .get_insert_block()
                        .and_then(|b| b.get_terminator())
                        .is_none()
                    {
                        self.builder.build_return(None)?;
                    }
                }
            }
            Ok(())
        }

        // ── Statement emission ────────────────────────────────────────────────

        fn emit_stmts(
            &mut self,
            f: FunctionValue<'ctx>,
            stmts: &[holyc_frontend::ast::Stmt],
        ) -> Result<(), CodegenError> {
            for s in stmts {
                if self
                    .builder
                    .get_insert_block()
                    .and_then(|b| b.get_terminator())
                    .is_some()
                {
                    break;
                }
                self.emit_stmt(f, s)?;
            }
            Ok(())
        }

        fn emit_stmt(
            &mut self,
            f: FunctionValue<'ctx>,
            stmt: &holyc_frontend::ast::Stmt,
        ) -> Result<(), CodegenError> {
            match &stmt.kind {
                StmtKind::Expr(e) => {
                    self.emit_expr(e)?;
                },

                StmtKind::VarDecl { ty, name, init } => {
                    let resolved = self.type_env.resolve(ty);
                    let llty = if let HolyType::Named(_) = &resolved {
                        let sz = self.type_env.size_of(ty).unwrap_or(8) as u32;
                        BasicTypeEnum::ArrayType(self.ctx.i8_type().array_type(sz))
                    } else {
                        self.llty(ty)?
                    };
                    let slot = self.builder.build_alloca(llty, name)?;
                    if let Some(e) = init {
                        if let Some(bv) = self.emit_expr(e)?.as_basic() {
                            self.builder.build_store(slot, bv)?;
                        }
                    }
                    self.locals.insert(name.clone(), (slot, ty.clone()));
                },

                StmtKind::Return(expr) => match expr {
                    None => {
                        self.builder.build_return(None)?;
                    },
                    Some(e) => match self.emit_expr(e)?.as_basic() {
                        Some(bv) => {
                            self.builder.build_return(Some(&bv))?;
                        },
                        None => {
                            self.builder.build_return(None)?;
                        },
                    },
                },

                StmtKind::Block(s) => self.emit_stmts(f, s)?,

                StmtKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    let cv = self.emit_expr(cond)?;
                    let ci1 = self.to_bool(cv)?;
                    let then_bb = self.ctx.append_basic_block(f, "if.then");
                    let else_bb = self.ctx.append_basic_block(f, "if.else");
                    let merge_bb = self.ctx.append_basic_block(f, "if.merge");
                    self.builder
                        .build_conditional_branch(ci1, then_bb, else_bb)?;

                    self.builder.position_at_end(then_bb);
                    self.emit_stmt(f, then_body)?;
                    if then_bb.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb)?;
                    }

                    self.builder.position_at_end(else_bb);
                    if let Some(eb) = else_body {
                        self.emit_stmt(f, eb)?;
                    }
                    if else_bb.get_terminator().is_none() {
                        self.builder.build_unconditional_branch(merge_bb)?;
                    }

                    self.builder.position_at_end(merge_bb);
                },

                StmtKind::While { cond, body } => {
                    let cond_bb = self.ctx.append_basic_block(f, "wh.cond");
                    let body_bb = self.ctx.append_basic_block(f, "wh.body");
                    let exit_bb = self.ctx.append_basic_block(f, "wh.exit");
                    self.builder.build_unconditional_branch(cond_bb)?;
                    self.loop_stack.push((exit_bb, cond_bb));

                    self.builder.position_at_end(cond_bb);
                    let cv = self.emit_expr(cond)?;
                    let ci1 = self.to_bool(cv)?;
                    self.builder
                        .build_conditional_branch(ci1, body_bb, exit_bb)?;

                    self.builder.position_at_end(body_bb);
                    self.emit_stmt(f, body)?;
                    // Use current insert block (may differ from body_bb after nested ifs).
                    if self
                        .builder
                        .get_insert_block()
                        .and_then(|b| b.get_terminator())
                        .is_none()
                    {
                        self.builder.build_unconditional_branch(cond_bb)?;
                    }

                    self.loop_stack.pop();
                    self.builder.position_at_end(exit_bb);
                },

                StmtKind::DoWhile { body, cond } => {
                    let body_bb = self.ctx.append_basic_block(f, "do.body");
                    let cond_bb = self.ctx.append_basic_block(f, "do.cond");
                    let exit_bb = self.ctx.append_basic_block(f, "do.exit");
                    self.builder.build_unconditional_branch(body_bb)?;
                    self.loop_stack.push((exit_bb, cond_bb));

                    self.builder.position_at_end(body_bb);
                    self.emit_stmt(f, body)?;
                    if self
                        .builder
                        .get_insert_block()
                        .and_then(|b| b.get_terminator())
                        .is_none()
                    {
                        self.builder.build_unconditional_branch(cond_bb)?;
                    }

                    self.builder.position_at_end(cond_bb);
                    let cv = self.emit_expr(cond)?;
                    let ci1 = self.to_bool(cv)?;
                    self.builder
                        .build_conditional_branch(ci1, body_bb, exit_bb)?;

                    self.loop_stack.pop();
                    self.builder.position_at_end(exit_bb);
                },

                StmtKind::For {
                    init,
                    cond,
                    step,
                    body,
                } => {
                    let cond_bb = self.ctx.append_basic_block(f, "for.cond");
                    let body_bb = self.ctx.append_basic_block(f, "for.body");
                    let step_bb = self.ctx.append_basic_block(f, "for.step");
                    let exit_bb = self.ctx.append_basic_block(f, "for.exit");
                    if let Some(s) = init {
                        self.emit_stmt(f, s)?;
                    }
                    self.builder.build_unconditional_branch(cond_bb)?;
                    self.loop_stack.push((exit_bb, step_bb));

                    self.builder.position_at_end(cond_bb);
                    if let Some(c) = cond {
                        let cv = self.emit_expr(c)?;
                        let ci1 = self.to_bool(cv)?;
                        self.builder
                            .build_conditional_branch(ci1, body_bb, exit_bb)?;
                    } else {
                        self.builder.build_unconditional_branch(body_bb)?;
                    }

                    self.builder.position_at_end(body_bb);
                    self.emit_stmt(f, body)?;
                    if self
                        .builder
                        .get_insert_block()
                        .and_then(|b| b.get_terminator())
                        .is_none()
                    {
                        self.builder.build_unconditional_branch(step_bb)?;
                    }

                    self.builder.position_at_end(step_bb);
                    if let Some(s) = step {
                        self.emit_expr(s)?;
                    }
                    self.builder.build_unconditional_branch(cond_bb)?;

                    self.loop_stack.pop();
                    self.builder.position_at_end(exit_bb);
                },

                StmtKind::Switch { expr, cases } => {
                    let val = self.emit_expr(expr)?;
                    let Operand::Int(sv) = val else {
                        return Err(CodegenError::Type("switch requires integer".into()));
                    };
                    let exit_bb = self.ctx.append_basic_block(f, "sw.exit");
                    let mut default_bb = exit_bb;
                    let mut bbs: Vec<(
                        &holyc_frontend::ast::SwitchCase,
                        inkwell::basic_block::BasicBlock,
                    )> = Vec::new();
                    for case in cases {
                        let bb = self.ctx.append_basic_block(f, "sw.case");
                        if case.value.is_none() {
                            default_bb = bb;
                        }
                        bbs.push((case, bb));
                    }
                    // Build cases list for build_switch (inkwell 0.4 takes them upfront).
                    let mut case_list: Vec<(
                        inkwell::values::IntValue<'ctx>,
                        inkwell::basic_block::BasicBlock<'ctx>,
                    )> = Vec::new();
                    for (case, bb) in &bbs {
                        if let Some(ve) = &case.value {
                            if let Operand::Int(iv) = self.emit_expr(ve)? {
                                if let Some(c) = iv
                                    .get_zero_extended_constant()
                                    .or_else(|| iv.get_sign_extended_constant().map(|v| v as u64))
                                {
                                    case_list.push((self.ctx.i64_type().const_int(c, false), *bb));
                                }
                            }
                        }
                    }
                    self.builder.build_switch(sv, default_bb, &case_list)?;
                    self.loop_stack.push((exit_bb, exit_bb));
                    for (case, bb) in &bbs {
                        self.builder.position_at_end(*bb);
                        self.emit_stmts(f, &case.body)?;
                        if bb.get_terminator().is_none() {
                            self.builder.build_unconditional_branch(exit_bb)?;
                        }
                    }
                    self.loop_stack.pop();
                    self.builder.position_at_end(exit_bb);
                },

                StmtKind::Break => {
                    if let Some(&(exit_bb, _)) = self.loop_stack.last() {
                        self.builder.build_unconditional_branch(exit_bb)?;
                    }
                },
                StmtKind::Continue => {
                    if let Some(&(_, cont_bb)) = self.loop_stack.last() {
                        self.builder.build_unconditional_branch(cont_bb)?;
                    }
                },
                StmtKind::Asm(text) => {
                    // Emit the raw text as an LLVM inline asm call with
                    // AT&T syntax, sideeffects=true, no I/O constraints.
                    // This is best-effort: complex asm with inputs/outputs
                    // requires explicit constraint strings not in the AST yet.
                    if !text.is_empty() {
                        let void_fn_ty = self.ctx.void_type().fn_type(&[], false);
                        let asm_ptr = self.ctx.create_inline_asm(
                            void_fn_ty,
                            text.clone(),
                            String::new(),
                            true,  // side effects
                            false, // align stack
                            Some(inkwell::InlineAsmDialect::ATT),
                            false, // can throw
                        );
                        self.builder
                            .build_indirect_call(void_fn_ty, asm_ptr, &[], "asm")
                            .ok();
                    }
                },
            }
            Ok(())
        }

        // ── Expression emission ───────────────────────────────────────────────

        fn emit_expr(
            &mut self,
            expr: &holyc_frontend::ast::Expr,
        ) -> Result<Operand<'ctx>, CodegenError> {
            match &expr.kind {
                ExprKind::IntLit(n) => Ok(Operand::Int(self.ctx.i64_type().const_int(*n, false))),
                ExprKind::FloatLit(f) => Ok(Operand::Float(self.ctx.f64_type().const_float(*f))),
                ExprKind::BoolLit(b) => Ok(Operand::Int(
                    self.ctx.bool_type().const_int(*b as u64, false),
                )),
                ExprKind::CharLit(c) => {
                    Ok(Operand::Int(self.ctx.i8_type().const_int(*c as u64, false)))
                },
                ExprKind::StringLit(s) => Ok(Operand::Ptr(self.const_str(s))),
                ExprKind::Null => Ok(Operand::Ptr(
                    self.ctx
                        .i8_type()
                        .ptr_type(AddressSpace::default())
                        .const_null(),
                )),

                ExprKind::Ident(name) => {
                    if let Some((slot, ty)) = self.locals.get(name).cloned() {
                        if matches!(self.type_env.resolve(&ty), HolyType::Named(_)) {
                            return Ok(Operand::Ptr(slot));
                        }
                        let llty = self.llty(&ty)?;
                        let v = self.builder.build_load(llty, slot, name)?;
                        return Ok(self.bv_to_op(v, &ty));
                    }
                    if let Some((g, ty)) = self.globals.get(name).cloned() {
                        let llty = self.llty(&ty)?;
                        let v = self.builder.build_load(llty, g.as_pointer_value(), name)?;
                        return Ok(self.bv_to_op(v, &ty));
                    }
                    Err(CodegenError::Undefined(format!("variable `{name}`")))
                },

                ExprKind::Assign { op, lhs, rhs } => self.emit_assign(op, lhs, rhs),
                ExprKind::Binary { op, lhs, rhs } => self.emit_binary(*op, lhs, rhs),
                ExprKind::Unary { op, operand } => self.emit_unary(*op, operand),

                ExprKind::Ternary { cond, then, else_ } => {
                    let ci1 = {
                        let cv = self.emit_expr(cond)?;
                        self.to_bool(cv)?
                    };
                    let tv = self.emit_expr(then)?;
                    let ev = self.emit_expr(else_)?;
                    Ok(match (tv, ev) {
                        (Operand::Int(t), Operand::Int(e)) => Operand::Int(
                            self.builder.build_select(ci1, t, e, "t")?.into_int_value(),
                        ),
                        (Operand::Float(t), Operand::Float(e)) => Operand::Float(
                            self.builder
                                .build_select(ci1, t, e, "t")?
                                .into_float_value(),
                        ),
                        (Operand::Ptr(t), Operand::Ptr(e)) => Operand::Ptr(
                            self.builder
                                .build_select(ci1, t, e, "t")?
                                .into_pointer_value(),
                        ),
                        _ => return Err(CodegenError::Type("ternary type mismatch".into())),
                    })
                },

                ExprKind::Call { callee, args } => {
                    let ExprKind::Ident(name) = &callee.kind else {
                        return Err(CodegenError::Unsupported("indirect call".into()));
                    };
                    let name = name.clone();
                    let mut av: Vec<BasicMetadataValueEnum<'ctx>> = Vec::new();
                    for a in args {
                        if let Some(m) = self.emit_expr(a)?.as_meta() {
                            av.push(m);
                        }
                    }
                    self.emit_call(&name, av)
                },

                ExprKind::Cast { ty, expr } => {
                    let v = self.emit_expr(expr)?;
                    let t = self.type_env.resolve(ty);
                    self.coerce(v, &t)
                },

                ExprKind::SizeOfType(ty) => {
                    let sz = self.type_env.size_of(ty).unwrap_or(0);
                    Ok(Operand::Int(self.ctx.i64_type().const_int(sz, false)))
                },

                ExprKind::SizeOfExpr(_) => {
                    // Full typeof inference is a later phase; return 8.
                    Ok(Operand::Int(self.ctx.i64_type().const_int(8, false)))
                },

                ExprKind::Index { base, idx } => {
                    let Operand::Ptr(ptr) = self.emit_expr(base)? else {
                        return Err(CodegenError::Type("subscript base must be pointer".into()));
                    };
                    let Operand::Int(idx_i) = self.emit_expr(idx)? else {
                        return Err(CodegenError::Type("subscript index must be integer".into()));
                    };
                    let i64t = self.ctx.i64_type();
                    let ep = unsafe { self.builder.build_gep(i64t, ptr, &[idx_i], "ep")? };
                    Ok(Operand::Int(
                        self.builder.build_load(i64t, ep, "el")?.into_int_value(),
                    ))
                },

                ExprKind::Member {
                    base,
                    field,
                    is_ptr,
                } => {
                    let bv = self.emit_expr(base)?;
                    let sp = match (is_ptr, &bv) {
                        (_, Operand::Ptr(p)) => *p,
                        _ => return Err(CodegenError::Type("member: need struct pointer".into())),
                    };
                    let (fl, fty) = self
                        .type_env
                        .structs
                        .values()
                        .find_map(|l| l.field(field).map(|f| (f.clone(), f.ty.clone())))
                        .ok_or_else(|| CodegenError::Undefined(format!("field `{field}`")))?;
                    let i8t = self.ctx.i8_type();
                    let off = self.ctx.i64_type().const_int(fl.offset, false);
                    let fp = unsafe { self.builder.build_gep(i8t, sp, &[off], "fp")? };
                    if matches!(self.type_env.resolve(&fty), HolyType::Named(_)) {
                        return Ok(Operand::Ptr(fp));
                    }
                    let llty = self.llty(&fty)?;
                    Ok(self.bv_to_op(self.builder.build_load(llty, fp, field)?, &fty))
                },
            }
        }

        // ── Binary / unary helpers ────────────────────────────────────────────

        fn emit_binary(
            &mut self,
            op: BinOp,
            le: &holyc_frontend::ast::Expr,
            re: &holyc_frontend::ast::Expr,
        ) -> Result<Operand<'ctx>, CodegenError> {
            if matches!(op, BinOp::LogAnd | BinOp::LogOr) {
                let li1 = {
                    let lv = self.emit_expr(le)?;
                    self.to_bool(lv)?
                };
                let ri1 = {
                    let rv = self.emit_expr(re)?;
                    self.to_bool(rv)?
                };
                let res = if op == BinOp::LogAnd {
                    self.builder.build_and(li1, ri1, "and")?
                } else {
                    self.builder.build_or(li1, ri1, "or")?
                };
                return Ok(Operand::Int(self.builder.build_int_z_extend(
                    res,
                    self.ctx.i64_type(),
                    "b2i",
                )?));
            }
            let lv = self.emit_expr(le)?;
            let rv = self.emit_expr(re)?;
            if matches!((&lv, &rv), (Operand::Float(_), _) | (_, Operand::Float(_))) {
                let lf = self.to_f64(lv)?;
                let rf = self.to_f64(rv)?;
                return self.float_op(op, lf, rf);
            }
            let li = self.to_i64(lv)?;
            let ri = self.to_i64(rv)?;
            Ok(match op {
                BinOp::Add => Operand::Int(self.builder.build_int_add(li, ri, "+")?),
                BinOp::Sub => Operand::Int(self.builder.build_int_sub(li, ri, "-")?),
                BinOp::Mul => Operand::Int(self.builder.build_int_mul(li, ri, "*")?),
                BinOp::Div => Operand::Int(self.builder.build_int_signed_div(li, ri, "/")?),
                BinOp::Rem => Operand::Int(self.builder.build_int_signed_rem(li, ri, "%")?),
                BinOp::BitAnd => Operand::Int(self.builder.build_and(li, ri, "&")?),
                BinOp::BitOr => Operand::Int(self.builder.build_or(li, ri, "|")?),
                BinOp::BitXor => Operand::Int(self.builder.build_xor(li, ri, "^")?),
                BinOp::Shl => Operand::Int(self.builder.build_left_shift(li, ri, "<<")?),
                BinOp::Shr => Operand::Int(self.builder.build_right_shift(li, ri, true, ">>")?),
                BinOp::Eq => {
                    Operand::Int(
                        self.builder
                            .build_int_compare(IntPredicate::EQ, li, ri, "eq")?,
                    )
                },
                BinOp::Ne => {
                    Operand::Int(
                        self.builder
                            .build_int_compare(IntPredicate::NE, li, ri, "ne")?,
                    )
                },
                BinOp::Lt => {
                    Operand::Int(
                        self.builder
                            .build_int_compare(IntPredicate::SLT, li, ri, "lt")?,
                    )
                },
                BinOp::Le => {
                    Operand::Int(
                        self.builder
                            .build_int_compare(IntPredicate::SLE, li, ri, "le")?,
                    )
                },
                BinOp::Gt => {
                    Operand::Int(
                        self.builder
                            .build_int_compare(IntPredicate::SGT, li, ri, "gt")?,
                    )
                },
                BinOp::Ge => {
                    Operand::Int(
                        self.builder
                            .build_int_compare(IntPredicate::SGE, li, ri, "ge")?,
                    )
                },
                BinOp::LogAnd | BinOp::LogOr => unreachable!(),
            })
        }

        fn float_op(
            &mut self,
            op: BinOp,
            lf: inkwell::values::FloatValue<'ctx>,
            rf: inkwell::values::FloatValue<'ctx>,
        ) -> Result<Operand<'ctx>, CodegenError> {
            Ok(match op {
                BinOp::Add => Operand::Float(self.builder.build_float_add(lf, rf, "f+")?),
                BinOp::Sub => Operand::Float(self.builder.build_float_sub(lf, rf, "f-")?),
                BinOp::Mul => Operand::Float(self.builder.build_float_mul(lf, rf, "f*")?),
                BinOp::Div => Operand::Float(self.builder.build_float_div(lf, rf, "f/")?),
                BinOp::Eq => Operand::Int(self.builder.build_float_compare(
                    FloatPredicate::OEQ,
                    lf,
                    rf,
                    "feq",
                )?),
                BinOp::Ne => Operand::Int(self.builder.build_float_compare(
                    FloatPredicate::ONE,
                    lf,
                    rf,
                    "fne",
                )?),
                BinOp::Lt => Operand::Int(self.builder.build_float_compare(
                    FloatPredicate::OLT,
                    lf,
                    rf,
                    "flt",
                )?),
                BinOp::Le => Operand::Int(self.builder.build_float_compare(
                    FloatPredicate::OLE,
                    lf,
                    rf,
                    "fle",
                )?),
                BinOp::Gt => Operand::Int(self.builder.build_float_compare(
                    FloatPredicate::OGT,
                    lf,
                    rf,
                    "fgt",
                )?),
                BinOp::Ge => Operand::Int(self.builder.build_float_compare(
                    FloatPredicate::OGE,
                    lf,
                    rf,
                    "fge",
                )?),
                _ => return Err(CodegenError::Unsupported(format!("float {op:?}"))),
            })
        }

        fn emit_unary(
            &mut self,
            op: UnaryOp,
            operand: &holyc_frontend::ast::Expr,
        ) -> Result<Operand<'ctx>, CodegenError> {
            match op {
                UnaryOp::Neg => {
                    let v = self.emit_expr(operand)?;
                    Ok(match v {
                        Operand::Int(i) => Operand::Int(self.builder.build_int_neg(i, "neg")?),
                        Operand::Float(f) => {
                            Operand::Float(self.builder.build_float_neg(f, "fneg")?)
                        },
                        _ => return Err(CodegenError::Type("neg: not a number".into())),
                    })
                },
                UnaryOp::BitNot => {
                    let v = self.emit_expr(operand)?;
                    let i = self.to_i64(v)?;
                    Ok(Operand::Int(self.builder.build_not(i, "bnot")?))
                },
                UnaryOp::LogNot => {
                    let v = self.emit_expr(operand)?;
                    let b = self.to_bool(v)?;
                    let nb = self.builder.build_not(b, "lnot")?;
                    Ok(Operand::Int(self.builder.build_int_z_extend(
                        nb,
                        self.ctx.i64_type(),
                        "ln64",
                    )?))
                },
                UnaryOp::AddrOf => {
                    if let ExprKind::Ident(name) = &operand.kind {
                        if let Some((s, _)) = self.locals.get(name) {
                            return Ok(Operand::Ptr(*s));
                        }
                        if let Some((g, _)) = self.globals.get(name) {
                            return Ok(Operand::Ptr(g.as_pointer_value()));
                        }
                    }
                    Err(CodegenError::Unsupported("addr-of non-ident".into()))
                },
                UnaryOp::Deref => {
                    let Operand::Ptr(ptr) = self.emit_expr(operand)? else {
                        return Err(CodegenError::Type("deref: not a pointer".into()));
                    };
                    let i64t = self.ctx.i64_type();
                    Ok(Operand::Int(
                        self.builder.build_load(i64t, ptr, "dr")?.into_int_value(),
                    ))
                },
                UnaryOp::PreInc | UnaryOp::PreDec => {
                    let d = if op == UnaryOp::PreInc { 1i64 } else { -1 };
                    Ok(Operand::Int(self.inc_dec(operand, d)?))
                },
                UnaryOp::PostInc | UnaryOp::PostDec => {
                    let old = self.emit_expr(operand)?;
                    let d = if op == UnaryOp::PostInc { 1i64 } else { -1 };
                    self.inc_dec(operand, d)?;
                    Ok(old)
                },
            }
        }

        fn inc_dec(
            &mut self,
            expr: &holyc_frontend::ast::Expr,
            delta: i64,
        ) -> Result<inkwell::values::IntValue<'ctx>, CodegenError> {
            let ExprKind::Ident(name) = &expr.kind else {
                return Err(CodegenError::Unsupported("inc/dec non-ident".into()));
            };
            let i64t = self.ctx.i64_type();
            let d = i64t.const_int(delta.unsigned_abs(), delta < 0);
            let (slot, ty) = self
                .locals
                .get(name)
                .cloned()
                .or_else(|| {
                    self.globals
                        .get(name)
                        .map(|(g, t)| (g.as_pointer_value(), t.clone()))
                })
                .ok_or_else(|| CodegenError::Undefined(name.clone()))?;
            let llty = self.llty(&ty)?;
            let old = self.builder.build_load(llty, slot, name)?.into_int_value();
            let new_val = if delta > 0 {
                self.builder.build_int_add(old, d, "inc")?
            } else {
                self.builder.build_int_sub(old, d, "dec")?
            };
            self.builder.build_store(slot, new_val)?;
            Ok(new_val)
        }

        fn emit_assign(
            &mut self,
            op: &AssignOp,
            lhs: &holyc_frontend::ast::Expr,
            rhs: &holyc_frontend::ast::Expr,
        ) -> Result<Operand<'ctx>, CodegenError> {
            // *ptr = val
            if let ExprKind::Unary {
                op: UnaryOp::Deref,
                operand,
            } = &lhs.kind
            {
                let Operand::Ptr(ptr) = self.emit_expr(operand)? else {
                    return Err(CodegenError::Type("deref assign: not pointer".into()));
                };
                let rv = self.emit_expr(rhs)?;
                if let Some(bv) = rv.as_basic() {
                    self.builder.build_store(ptr, bv)?;
                }
                return Ok(rv);
            }
            // base.field = val / ptr->field = val
            if let ExprKind::Member { base, field, .. } = &lhs.kind {
                let Operand::Ptr(sp) = self.emit_expr(base)? else {
                    return Err(CodegenError::Type("member assign: need ptr".into()));
                };
                let fl = self
                    .type_env
                    .structs
                    .values()
                    .find_map(|l| l.field(field).cloned())
                    .ok_or_else(|| CodegenError::Undefined(format!("field `{field}`")))?;
                let i8t = self.ctx.i8_type();
                let off = self.ctx.i64_type().const_int(fl.offset, false);
                let fp = unsafe { self.builder.build_gep(i8t, sp, &[off], "fp")? };
                let rv = self.emit_expr(rhs)?;
                if let Some(bv) = rv.as_basic() {
                    self.builder.build_store(fp, bv)?;
                }
                return Ok(rv);
            }
            // arr[i] = val
            if let ExprKind::Index { base, idx } = &lhs.kind {
                let Operand::Ptr(ptr) = self.emit_expr(base)? else {
                    return Err(CodegenError::Type("index assign: not pointer".into()));
                };
                let Operand::Int(idx_i) = self.emit_expr(idx)? else {
                    return Err(CodegenError::Type("index must be integer".into()));
                };
                let i64t = self.ctx.i64_type();
                let ep = unsafe { self.builder.build_gep(i64t, ptr, &[idx_i], "ep")? };
                let rv = self.emit_expr(rhs)?;
                if let Some(bv) = rv.as_basic() {
                    self.builder.build_store(ep, bv)?;
                }
                return Ok(rv);
            }
            // Normal variable.
            let ExprKind::Ident(name) = &lhs.kind else {
                return Err(CodegenError::Unsupported("complex lhs".into()));
            };
            let rv = self.emit_expr(rhs)?;
            let (slot, ty) = self
                .locals
                .get(name)
                .cloned()
                .or_else(|| {
                    self.globals
                        .get(name)
                        .map(|(g, t)| (g.as_pointer_value(), t.clone()))
                })
                .ok_or_else(|| CodegenError::Undefined(name.clone()))?;
            let final_val = if *op == AssignOp::Assign {
                rv.clone()
            } else {
                let llty = self.llty(&ty)?;
                let cur = self.bv_to_op(self.builder.build_load(llty, slot, "cur")?, &ty);
                let li = self.to_i64(cur)?;
                let ri = self.to_i64(rv.clone())?;
                Operand::Int(match op {
                    AssignOp::Add => self.builder.build_int_add(li, ri, "ca")?,
                    AssignOp::Sub => self.builder.build_int_sub(li, ri, "cs")?,
                    AssignOp::Mul => self.builder.build_int_mul(li, ri, "cm")?,
                    AssignOp::Div => self.builder.build_int_signed_div(li, ri, "cd")?,
                    AssignOp::Rem => self.builder.build_int_signed_rem(li, ri, "cr")?,
                    AssignOp::BitAnd => self.builder.build_and(li, ri, "cba")?,
                    AssignOp::BitOr => self.builder.build_or(li, ri, "cbo")?,
                    AssignOp::BitXor => self.builder.build_xor(li, ri, "cbx")?,
                    AssignOp::Shl => self.builder.build_left_shift(li, ri, "csl")?,
                    AssignOp::Shr => self.builder.build_right_shift(li, ri, true, "csr")?,
                    AssignOp::Assign => unreachable!(),
                })
            };
            if let Some(bv) = final_val.as_basic() {
                self.builder.build_store(slot, bv)?;
            }
            Ok(final_val)
        }

        fn emit_call(
            &mut self,
            name: &str,
            av: Vec<BasicMetadataValueEnum<'ctx>>,
        ) -> Result<Operand<'ctx>, CodegenError> {
            macro_rules! call_void {
                ($f:expr) => {{
                    self.builder.build_call($f, &av, "")?;
                    return Ok(Operand::Void);
                }};
            }
            macro_rules! call_int {
                ($f:expr, $n:expr) => {{
                    let r = self.builder.build_call($f, &av, $n)?;
                    return Ok(Operand::Int(
                        r.try_as_basic_value().left().unwrap().into_int_value(),
                    ));
                }};
            }
            macro_rules! call_f64 {
                ($f:expr, $n:expr) => {{
                    let r = self.builder.build_call($f, &av, $n)?;
                    return Ok(Operand::Float(
                        r.try_as_basic_value().left().unwrap().into_float_value(),
                    ));
                }};
            }
            macro_rules! call_ptr {
                ($f:expr, $n:expr) => {{
                    let r = self.builder.build_call($f, &av, $n)?;
                    return Ok(Operand::Ptr(
                        r.try_as_basic_value().left().unwrap().into_pointer_value(),
                    ));
                }};
            }

            match name {
                "Print" | "print" | "printf" => {
                    let f = self.decl_printf();
                    call_void!(f);
                },
                "MAlloc" | "malloc" => {
                    let f = self.decl_malloc();
                    call_ptr!(f, "ml");
                },
                "Free" | "free" => {
                    let f = self.decl_free();
                    call_void!(f);
                },
                "MemSet" | "memset" => {
                    let f = self.decl_memset();
                    call_void!(f);
                },
                "MemCpy" | "memcpy" => {
                    let f = self.decl_memcpy();
                    call_void!(f);
                },
                "StrLen" | "strlen" => {
                    let f = self.decl_strlen();
                    call_int!(f, "sl");
                },
                "StrCmp" | "strcmp" => {
                    let f = self.decl_strcmp();
                    let r = self.builder.build_call(f, &av, "sc")?;
                    let i32v = r.try_as_basic_value().left().unwrap().into_int_value();
                    return Ok(Operand::Int(self.builder.build_int_s_extend(
                        i32v,
                        self.ctx.i64_type(),
                        "sc64",
                    )?));
                },
                "Sin" | "sin" => {
                    let f = self.decl_f64_1("sin");
                    call_f64!(f, "sin");
                },
                "Cos" | "cos" => {
                    let f = self.decl_f64_1("cos");
                    call_f64!(f, "cos");
                },
                "Sqrt" | "sqrt" => {
                    let f = self.decl_f64_1("sqrt");
                    call_f64!(f, "sqr");
                },
                "Pow" | "pow" => {
                    let f = self.decl_pow();
                    call_f64!(f, "pow");
                },
                "Abs" | "abs" => {
                    if let Some(&BasicMetadataValueEnum::IntValue(v)) = av.first() {
                        let zero = self.ctx.i64_type().const_zero();
                        let neg = self.builder.build_int_neg(v, "n")?;
                        let c = self
                            .builder
                            .build_int_compare(IntPredicate::SGT, v, zero, "p")?;
                        return Ok(Operand::Int(
                            self.builder
                                .build_select(c, v, neg, "abs")?
                                .into_int_value(),
                        ));
                    }
                    return Ok(Operand::Int(self.ctx.i64_type().const_zero()));
                },
                "Exit" | "exit" => {
                    let f = self.decl_exit();
                    self.builder.build_call(f, &av, "")?;
                    self.builder.build_unreachable()?;
                    return Ok(Operand::Void);
                },
                _ => {},
            }
            // User-defined.
            if let Some((fv, ret_ty)) = self.fn_table.get(name).cloned() {
                let r = self.builder.build_call(fv, &av, "call")?;
                if matches!(self.type_env.resolve(&ret_ty), HolyType::Void) {
                    return Ok(Operand::Void);
                }
                let bv = r
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| CodegenError::Type(format!("`{name}` returned no value")))?;
                return Ok(self.bv_to_op(bv, &ret_ty));
            }
            Err(CodegenError::Undefined(format!("function `{name}`")))
        }

        // ── Coercion ──────────────────────────────────────────────────────────

        fn to_i64(
            &self,
            op: Operand<'ctx>,
        ) -> Result<inkwell::values::IntValue<'ctx>, CodegenError> {
            let i64t = self.ctx.i64_type();
            match op {
                Operand::Int(v) => {
                    let w = v.get_type().get_bit_width();
                    if w < 64 {
                        Ok(self.builder.build_int_s_extend(v, i64t, "sx")?)
                    } else if w > 64 {
                        Ok(self.builder.build_int_truncate(v, i64t, "tr")?)
                    } else {
                        Ok(v)
                    }
                },
                Operand::Float(f) => Ok(self.builder.build_float_to_signed_int(f, i64t, "f2i")?),
                Operand::Ptr(p) => Ok(self.builder.build_ptr_to_int(p, i64t, "p2i")?),
                Operand::Void => Ok(i64t.const_zero()),
            }
        }

        fn to_f64(
            &self,
            op: Operand<'ctx>,
        ) -> Result<inkwell::values::FloatValue<'ctx>, CodegenError> {
            let f64t = self.ctx.f64_type();
            match op {
                Operand::Float(f) => {
                    if f.get_type() == self.ctx.f32_type() {
                        Ok(self.builder.build_float_ext(f, f64t, "fe")?)
                    } else {
                        Ok(f)
                    }
                },
                Operand::Int(i) => Ok(self.builder.build_signed_int_to_float(i, f64t, "i2f")?),
                _ => Err(CodegenError::Type("cannot convert to f64".into())),
            }
        }

        fn to_bool(
            &self,
            op: Operand<'ctx>,
        ) -> Result<inkwell::values::IntValue<'ctx>, CodegenError> {
            match op {
                Operand::Int(v) => {
                    let z = v.get_type().const_zero();
                    Ok(self
                        .builder
                        .build_int_compare(IntPredicate::NE, v, z, "b")?)
                },
                Operand::Float(f) => {
                    let z = f.get_type().const_zero();
                    Ok(self
                        .builder
                        .build_float_compare(FloatPredicate::ONE, f, z, "fb")?)
                },
                Operand::Ptr(p) => {
                    let i64t = self.ctx.i64_type();
                    let pi = self.builder.build_ptr_to_int(p, i64t, "pi")?;
                    let ni =
                        self.builder
                            .build_ptr_to_int(p.get_type().const_null(), i64t, "ni")?;
                    Ok(self
                        .builder
                        .build_int_compare(IntPredicate::NE, pi, ni, "pne")?)
                },
                Operand::Void => Ok(self.ctx.bool_type().const_zero()),
            }
        }

        fn coerce(
            &self,
            val: Operand<'ctx>,
            target: &HolyType,
        ) -> Result<Operand<'ctx>, CodegenError> {
            if self.is_float(target) {
                return Ok(Operand::Float(self.to_f64(val)?));
            }
            if matches!(target, HolyType::Ptr(_)) {
                return match val {
                    Operand::Ptr(_) => Ok(val),
                    Operand::Int(i) => Ok(Operand::Ptr(self.builder.build_int_to_ptr(
                        i,
                        self.ctx.i8_type().ptr_type(AddressSpace::default()),
                        "i2p",
                    )?)),
                    _ => Err(CodegenError::Type("cast to pointer".into())),
                };
            }
            Ok(Operand::Int(self.to_i64(val)?))
        }

        fn bv_to_op(&self, bv: BasicValueEnum<'ctx>, ty: &HolyType) -> Operand<'ctx> {
            match self.type_env.resolve(ty) {
                HolyType::F32 | HolyType::F64 => Operand::Float(bv.into_float_value()),
                HolyType::Ptr(_) | HolyType::FnPtr { .. } => Operand::Ptr(bv.into_pointer_value()),
                _ => Operand::Int(bv.into_int_value()),
            }
        }

        fn emit_main_trampoline(&mut self) {
            // Emit `int main() { Main(); return 0; }` if Main() exists but main() does not,
            // so the system linker can find the AOT entry point.
            if self.module.get_function("main").is_some() {
                return;
            }
            let Some(holy_main) = self.module.get_function("Main") else {
                return;
            };
            let i32t = self.ctx.i32_type();
            let main_fn = self
                .module
                .add_function("main", i32t.fn_type(&[], false), None);
            let bb = self.ctx.append_basic_block(main_fn, "entry");
            self.builder.position_at_end(bb);
            self.builder.build_call(holy_main, &[], "").ok();
            self.builder.build_return(Some(&i32t.const_zero())).ok();
        }
    }

    fn u8_to_opt(level: u8) -> OptimizationLevel {
        match level {
            0 => OptimizationLevel::None,
            1 => OptimizationLevel::Less,
            3 => OptimizationLevel::Aggressive,
            _ => OptimizationLevel::Default,
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /// Owns the LLVM `Context` for a compilation session.
    pub struct CodegenSession {
        context: Context,
    }

    impl CodegenSession {
        pub fn new() -> Self {
            Self {
                context: Context::create(),
            }
        }

        /// Compile `ast` to LLVM IR text.
        pub fn emit_ir(
            &self,
            name: &str,
            ast: &Module,
            env: TypeEnv,
        ) -> Result<String, CodegenError> {
            let mut cg = Cg::new(&self.context, name, env);
            cg.compile_module(ast)?;
            Ok(cg.module.print_to_string().to_string())
        }

        /// JIT-compile `ast` and run its `Main()` entry point.
        pub fn jit_run(&self, name: &str, ast: &Module, env: TypeEnv) -> Result<(), CodegenError> {
            let mut cg = Cg::new(&self.context, name, env);
            cg.compile_module(ast)?;
            let ee = cg
                .module
                .create_jit_execution_engine(OptimizationLevel::Default)
                .map_err(|e| CodegenError::Llvm(e.to_string()))?;
            for entry in &["Main", "main"] {
                if let Some(fv) = cg.module.get_function(entry) {
                    unsafe {
                        ee.run_function_as_main(fv, &[]);
                    }
                    return Ok(());
                }
            }
            Err(CodegenError::Undefined("no Main() entry point".into()))
        }

        /// Compile to a native object file.
        pub fn emit_object(
            &self,
            name: &str,
            ast: &Module,
            env: TypeEnv,
            out_path: &Path,
            target_triple: Option<&str>,
            opt: u8,
        ) -> Result<(), CodegenError> {
            let mut cg = Cg::new(&self.context, name, env);
            cg.compile_module(ast)?;
            cg.emit_main_trampoline();
            let tm = Self::make_tm(target_triple, u8_to_opt(opt))?;
            tm.write_to_file(&cg.module, FileType::Object, out_path)
                .map_err(|e| CodegenError::Llvm(e.to_string()))
        }

        /// Compile to AT&T-syntax assembly.
        pub fn emit_asm_file(
            &self,
            name: &str,
            ast: &Module,
            env: TypeEnv,
            out_path: &Path,
            target_triple: Option<&str>,
            opt: u8,
        ) -> Result<(), CodegenError> {
            let mut cg = Cg::new(&self.context, name, env);
            cg.compile_module(ast)?;
            cg.emit_main_trampoline();
            let tm = Self::make_tm(target_triple, u8_to_opt(opt))?;
            tm.write_to_file(&cg.module, FileType::Assembly, out_path)
                .map_err(|e| CodegenError::Llvm(e.to_string()))
        }

        /// Compile to a native executable by writing an object file then
        /// invoking the system `cc` linker.
        pub fn emit_executable(
            &self,
            name: &str,
            ast: &Module,
            env: TypeEnv,
            out_path: &Path,
            target_triple: Option<&str>,
            opt: u8,
        ) -> Result<(), CodegenError> {
            let obj_path = out_path.with_extension("o");
            self.emit_object(name, ast, env, &obj_path, target_triple, opt)?;
            let status = std::process::Command::new("cc")
                .args([
                    &obj_path,
                    std::path::Path::new("-o"),
                    out_path,
                    std::path::Path::new("-lm"),
                ])
                .status()
                .map_err(|e| CodegenError::Llvm(format!("cc: {e}")))?;
            let _ = std::fs::remove_file(&obj_path);
            if status.success() {
                Ok(())
            } else {
                Err(CodegenError::Llvm(format!("linker failed: {status}")))
            }
        }

        fn make_tm(
            triple_str: Option<&str>,
            opt: OptimizationLevel,
        ) -> Result<TargetMachine, CodegenError> {
            Target::initialize_native(&InitializationConfig::default())
                .map_err(|e| CodegenError::Llvm(format!("native init: {e}")))?;
            Target::initialize_all(&InitializationConfig::default());
            let triple = match triple_str {
                Some(t) => TargetTriple::create(t),
                None => TargetMachine::get_default_triple(),
            };
            let target = Target::from_triple(&triple)
                .map_err(|e| CodegenError::Llvm(format!("triple: {e}")))?;
            target
                .create_target_machine(
                    &triple,
                    "generic",
                    "",
                    opt,
                    RelocMode::PIC,
                    CodeModel::Default,
                )
                .ok_or_else(|| CodegenError::Llvm("TargetMachine creation failed".into()))
        }
    }

    impl Default for CodegenSession {
        fn default() -> Self {
            Self::new()
        }
    }
}
