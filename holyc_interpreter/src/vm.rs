//! Tree-walk interpreter for HolyC.
//!
//! Loads a parsed [`Module`], registers builtins, then executes statements
//! by recursively evaluating AST nodes.  Control flow (return / break /
//! continue) is propagated via `Err(Signal)` so that `?` can unwind frames.

use std::collections::HashMap;

use holyc_frontend::ast::{AssignOp, BinOp, ExprKind, Module, StmtKind, TopLevelKind, UnaryOp};
use holyc_frontend::layout::TypeEnv;
use holyc_frontend::types::HolyType;

use crate::heap::{Heap, DEFAULT_HEAP_SIZE};
use crate::runtime::{Callable, Env, FuncDef, Signal};
use holyc_stdlib::builtins::{self, RuntimeError, Value};

const MAX_CALL_DEPTH: usize = 512;

// ── Interpreter ───────────────────────────────────────────────────────────────

pub struct Interpreter {
    env: Env,
    funcs: HashMap<String, Callable>,
    pub heap: Heap,
    call_depth: usize,
    /// Type environment: struct layouts + typedef aliases.
    pub type_env: TypeEnv,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_heap_size(DEFAULT_HEAP_SIZE)
    }

    pub fn with_heap_size(heap_bytes: usize) -> Self {
        let mut interp = Interpreter {
            env: Env::new(),
            funcs: HashMap::new(),
            heap: Heap::new(heap_bytes),
            call_depth: 0,
            type_env: TypeEnv::new(),
        };
        interp.register_builtins();
        interp
    }

    fn register_builtins(&mut self) {
        macro_rules! reg {
            ($name:expr, $fn:expr) => {
                self.funcs.insert($name.into(), Callable::Builtin($fn));
            };
        }
        reg!("Print", builtins::print);
        reg!("print", builtins::print);
        reg!("printf", builtins::print);
        reg!("Exit", builtins::exit);
        // Math
        reg!("Abs", builtins::abs);
        reg!("Sin", builtins::sin);
        reg!("Cos", builtins::cos);
        reg!("Sqrt", builtins::sqrt);
        reg!("Pow", builtins::pow);
        // String (value-level; Ptr variants resolved inline in call_func)
        reg!("StrCmp", builtins::strcmp);
        reg!("strcmp", builtins::strcmp);
        reg!("StrCpy", builtins::strcpy);
        reg!("strcpy", builtins::strcpy);
        reg!("StrCat", builtins::strcat);
        reg!("strcat", builtins::strcat);
        reg!("StrStr", builtins::strstr);
        reg!("strstr", builtins::strstr);
        reg!("StrToI64", builtins::str_to_i64);
        // Random
        reg!("Rand", builtins::rand);
        reg!("rand", builtins::rand);
        reg!("RandI64", builtins::rand_range);
        reg!("SRand", builtins::srand);
        reg!("srand", builtins::srand);
        // Time
        reg!("Time", builtins::time_now);
        // String formatting
        reg!("SPrint", builtins::sprint);
        reg!("sprint", builtins::sprint);
        // Memory
        reg!("MemCmp", builtins::memcmp_stub);
        reg!("memcmp", builtins::memcmp_stub);
        // Heap builtins are handled inline in call_func (need &mut self.heap).
    }

    // ── Module execution ──────────────────────────────────────────────────────

    pub fn exec_module(&mut self, module: &Module) -> Result<(), RuntimeError> {
        // Pass 1: hoist function definitions and register struct sizes.
        for item in &module.items {
            match &item.kind {
                TopLevelKind::FuncDef {
                    name, params, body, ..
                } => {
                    self.funcs.insert(
                        name.clone(),
                        Callable::UserDef(FuncDef {
                            params: params.clone(),
                            body: body.clone(),
                        }),
                    );
                },
                TopLevelKind::ClassDef { name, fields } => {
                    let raw: Vec<(String, HolyType)> = fields
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

        // Pass 2: execute global variable initialisers.
        for item in &module.items {
            if let TopLevelKind::GlobalVar { name, init, .. } = &item.kind {
                let val = if let Some(expr) = init {
                    self.eval_expr(expr)?
                } else {
                    Value::Int(0)
                };
                self.env.define(name.clone(), val);
            }
        }

        // Execute `Main` if present.
        if self.funcs.contains_key("Main") {
            self.call_func("Main", &[])?;
        }

        Ok(())
    }

    // ── Function calls ────────────────────────────────────────────────────────

    pub fn call_func(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(RuntimeError::Custom("stack overflow".into()));
        }

        // ── Heap builtins handled inline (need &mut self.heap) ────────────────
        match name {
            "MAlloc" | "malloc" => {
                let size = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
                return self.heap.alloc(size).map(Value::Ptr);
            },
            "Free" | "free" => {
                let ptr = args.first().and_then(|v| v.as_int()).unwrap_or(0) as usize;
                return self.heap.free(ptr).map(|_| Value::Void);
            },
            "MemSet" | "memset" => {
                if args.len() < 3 {
                    return Err(RuntimeError::Custom("MemSet requires 3 arguments".into()));
                }
                let ptr = args[0].as_int().unwrap_or(0) as usize;
                let val = args[1].as_int().unwrap_or(0) as u8;
                let len = args[2].as_int().unwrap_or(0) as usize;
                return self.heap.memset(ptr, val, len).map(|_| Value::Void);
            },
            "MemCpy" | "memcpy" => {
                if args.len() < 3 {
                    return Err(RuntimeError::Custom("MemCpy requires 3 arguments".into()));
                }
                let dst = args[0].as_int().unwrap_or(0) as usize;
                let src = args[1].as_int().unwrap_or(0) as usize;
                let len = args[2].as_int().unwrap_or(0) as usize;
                return self.heap.memcpy(dst, src, len).map(|_| Value::Void);
            },
            "StrLen" | "strlen" => {
                // Override the stdlib stub to read from the heap when given a Ptr.
                match args.first() {
                    Some(Value::Str(s)) => return Ok(Value::Int(s.len() as i64)),
                    Some(Value::Ptr(p)) => {
                        let s = self.heap.read_cstr(*p)?;
                        return Ok(Value::Int(s.len() as i64));
                    },
                    _ => {
                        return Err(RuntimeError::Custom(
                            "StrLen: expected string or pointer".into(),
                        ))
                    },
                }
            },
            _ => {},
        }

        // ── Resolve Ptr args to Str for string builtins ───────────────────────
        // String literals are heap-interned pointers. Before passing to builtins
        // that expect Value::Str, resolve any Ptr to a C string from the heap.
        const STR_BUILTINS: &[&str] = &[
            "Print", "print", "printf", "StrLen", "strlen", "StrCmp", "strcmp", "StrCpy", "strcpy",
            "StrCat", "strcat", "StrStr", "strstr", "StrToI64",
        ];
        let resolved_args: Vec<Value>;
        let args = if STR_BUILTINS.contains(&name) {
            resolved_args = args
                .iter()
                .map(|v| match v {
                    Value::Ptr(p) if *p != 0 => self
                        .heap
                        .read_cstr(*p)
                        .map(Value::Str)
                        .unwrap_or_else(|_| v.clone()),
                    other => other.clone(),
                })
                .collect();
            resolved_args.as_slice()
        } else {
            args
        };

        let callable = self
            .funcs
            .get(name)
            .ok_or_else(|| RuntimeError::Custom(format!("undefined function `{name}`")))?
            .clone();

        self.invoke(callable, name, args)
    }

    fn invoke(
        &mut self,
        callable: Callable,
        _name: &str,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        match callable {
            Callable::Builtin(f) => f(args),
            Callable::UserDef(def) => {
                if args.len() != def.params.len() {
                    return Err(RuntimeError::ArgCount {
                        expected: def.params.len(),
                        got: args.len(),
                    });
                }
                self.call_depth += 1;
                self.env.push_scope();
                for (param, val) in def.params.iter().zip(args.iter()) {
                    self.env.define(param.name.clone(), val.clone());
                }
                let result = self.exec_stmts(&def.body);
                self.env.pop_scope();
                self.call_depth -= 1;
                match result {
                    Ok(()) => Ok(Value::Void),
                    Err(Signal::Return(v)) => Ok(v),
                    Err(Signal::Break) => Ok(Value::Void),
                    Err(Signal::Continue) => Ok(Value::Void),
                    Err(Signal::Error(e)) => Err(e),
                }
            },
        }
    }

    // ── Statement execution ───────────────────────────────────────────────────

    fn exec_stmts(&mut self, stmts: &[holyc_frontend::ast::Stmt]) -> Result<(), Signal> {
        for stmt in stmts {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    fn exec_stmt(&mut self, stmt: &holyc_frontend::ast::Stmt) -> Result<(), Signal> {
        match &stmt.kind {
            StmtKind::Expr(expr) => {
                self.eval_expr(expr).map_err(Signal::Error)?;
            },

            StmtKind::VarDecl { ty, name, init } => {
                // For struct-typed variables, allocate on the heap and bind a Ptr.
                let resolved_ty = self.type_env.resolve(ty);
                let val = if let HolyType::Named(struct_name) = &resolved_ty {
                    if let Some(layout) = self.type_env.structs.get(struct_name).cloned() {
                        let ptr = self
                            .heap
                            .alloc(layout.size as usize)
                            .map_err(Signal::Error)?;
                        // If there's an initialiser expression it must be a
                        // pointer to another struct; copy field-by-field.
                        if let Some(e) = init {
                            let src = self.eval_expr(e).map_err(Signal::Error)?;
                            if let Value::Ptr(src_ptr) = src {
                                self.heap
                                    .memcpy(ptr, src_ptr, layout.size as usize)
                                    .map_err(Signal::Error)?;
                            }
                        }
                        Value::Ptr(ptr)
                    } else {
                        // Unknown named type — fall back to 0 / expression
                        match init {
                            Some(e) => self.eval_expr(e).map_err(Signal::Error)?,
                            None => Value::Int(0),
                        }
                    }
                } else {
                    match init {
                        Some(e) => self.eval_expr(e).map_err(Signal::Error)?,
                        None => Value::Int(0),
                    }
                };
                self.env.define(name.clone(), val);
            },

            StmtKind::Return(expr) => {
                let val = match expr {
                    Some(e) => self.eval_expr(e).map_err(Signal::Error)?,
                    None => Value::Void,
                };
                return Err(Signal::Return(val));
            },

            StmtKind::Break => return Err(Signal::Break),
            StmtKind::Continue => return Err(Signal::Continue),

            StmtKind::Block(stmts) => {
                self.env.push_scope();
                let r = self.exec_stmts(stmts);
                self.env.pop_scope();
                r?;
            },

            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                let v = self.eval_expr(cond).map_err(Signal::Error)?;
                if v.is_truthy() {
                    self.exec_stmt(then_body)?;
                } else if let Some(eb) = else_body {
                    self.exec_stmt(eb)?;
                }
            },

            StmtKind::While { cond, body } => loop {
                let v = self.eval_expr(cond).map_err(Signal::Error)?;
                if !v.is_truthy() {
                    break;
                }
                match self.exec_stmt(body) {
                    Ok(()) => {},
                    Err(Signal::Break) => break,
                    Err(Signal::Continue) => continue,
                    Err(e) => return Err(e),
                }
            },

            StmtKind::DoWhile { body, cond } => loop {
                match self.exec_stmt(body) {
                    Ok(()) => {},
                    Err(Signal::Break) => break,
                    Err(Signal::Continue) => {},
                    Err(e) => return Err(e),
                }
                let v = self.eval_expr(cond).map_err(Signal::Error)?;
                if !v.is_truthy() {
                    break;
                }
            },

            StmtKind::For {
                init,
                cond,
                step,
                body,
            } => {
                self.env.push_scope();
                if let Some(init_stmt) = init {
                    self.exec_stmt(init_stmt)?;
                }
                'for_loop: loop {
                    if let Some(c) = cond {
                        let v = self.eval_expr(c).map_err(Signal::Error)?;
                        if !v.is_truthy() {
                            break;
                        }
                    }
                    match self.exec_stmt(body) {
                        Ok(()) => {},
                        Err(Signal::Break) => break 'for_loop,
                        Err(Signal::Continue) => {},
                        Err(e) => {
                            self.env.pop_scope();
                            return Err(e);
                        },
                    }
                    if let Some(s) = step {
                        self.eval_expr(s).map_err(Signal::Error)?;
                    }
                }
                self.env.pop_scope();
            },

            StmtKind::Switch { expr, cases } => {
                let val = self.eval_expr(expr).map_err(Signal::Error)?;
                let mut matched = false;
                'switch: for case in cases {
                    let should_run = matched
                        || match &case.value {
                            None => true,
                            Some(e) => {
                                let cv = self.eval_expr(e).map_err(Signal::Error)?;
                                values_equal(&val, &cv)
                            },
                        };
                    if should_run {
                        matched = true;
                        match self.exec_stmts(&case.body) {
                            Ok(()) => {},
                            Err(Signal::Break) => break 'switch,
                            Err(e) => return Err(e),
                        }
                    }
                }
            },

            StmtKind::Asm(_) => {
                // Inline asm is a no-op in the tree-walk interpreter.
                // The LLVM backend handles it properly.
            },
        }
        Ok(())
    }

    // ── Expression evaluation ─────────────────────────────────────────────────

    pub fn eval_expr(&mut self, expr: &holyc_frontend::ast::Expr) -> Result<Value, RuntimeError> {
        match &expr.kind {
            ExprKind::IntLit(n) => Ok(Value::Int(*n as i64)),
            ExprKind::FloatLit(f) => Ok(Value::Float(*f)),
            ExprKind::StringLit(s) => {
                // Intern the string into the heap so pointer arithmetic works.
                let ptr = self.heap.intern_str(s)?;
                Ok(Value::Ptr(ptr))
            },
            ExprKind::CharLit(c) => Ok(Value::Char(*c)),
            ExprKind::BoolLit(b) => Ok(Value::Bool(*b)),
            ExprKind::Null => Ok(Value::Ptr(0)),

            ExprKind::Ident(name) => self
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::Custom(format!("undefined variable `{name}`"))),

            ExprKind::Unary { op, operand } => self.eval_unary(*op, operand),
            ExprKind::Binary { op, lhs, rhs } => self.eval_binary(*op, lhs, rhs),
            ExprKind::Assign { op, lhs, rhs } => self.eval_assign(*op, lhs, rhs),

            ExprKind::Ternary { cond, then, else_ } => {
                let v = self.eval_expr(cond)?;
                if v.is_truthy() {
                    self.eval_expr(then)
                } else {
                    self.eval_expr(else_)
                }
            },

            ExprKind::Call { callee, args } => {
                let func_name = match &callee.kind {
                    ExprKind::Ident(name) => name.clone(),
                    _ => {
                        return Err(RuntimeError::Custom(
                            "indirect/function-pointer calls not yet supported".into(),
                        ))
                    },
                };
                let mut arg_vals = Vec::with_capacity(args.len());
                for a in args {
                    arg_vals.push(self.eval_expr(a)?);
                }
                self.call_func(&func_name, &arg_vals)
            },

            // Casts: pass-through in tree-walk; LLVM backend handles truncation.
            ExprKind::Cast { expr, .. } => self.eval_expr(expr),

            ExprKind::SizeOfType(ty) => {
                let size = self.type_env.size_of(ty).unwrap_or(0);
                Ok(Value::UInt(size))
            },
            ExprKind::SizeOfExpr(e) => {
                let val = self.eval_expr(e)?;
                let size = match &val {
                    Value::Bool(_) | Value::Char(_) => 1,
                    Value::Float(_) | Value::Int(_) | Value::UInt(_) | Value::Ptr(_) => 8,
                    Value::Str(s) => s.len() as u64 + 1,
                    Value::Void => 0,
                };
                Ok(Value::UInt(size))
            },

            // Array subscript: base[idx] — reads an 8-byte word from the heap.
            ExprKind::Index { base, idx } => {
                let base_val = self.eval_expr(base)?;
                let idx_val = self.eval_expr(idx)?;
                let ptr = match base_val {
                    Value::Ptr(p) => p,
                    Value::Int(n) => n as usize,
                    _ => {
                        return Err(RuntimeError::Custom(
                            "subscript: base must be a pointer".into(),
                        ))
                    },
                };
                let i = idx_val.as_int().ok_or_else(|| {
                    RuntimeError::Custom("subscript: index must be an integer".into())
                })?;
                let offset = ptr + (i as usize) * 8;
                let raw = self.heap.read_uint(offset, 8)?;
                Ok(Value::Int(raw as i64))
            },

            // Member access: `base.field` or `base->field`
            ExprKind::Member {
                base,
                field,
                is_ptr,
            } => {
                let base_val = self.eval_expr(base)?;

                // Resolve the heap pointer to the start of the struct.
                let struct_ptr = match (is_ptr, &base_val) {
                    // `->` : base is a pointer to a struct
                    (true, Value::Ptr(0)) => {
                        return Err(RuntimeError::Custom(
                            "null pointer dereference in member access".into(),
                        ))
                    },
                    (true, Value::Ptr(p)) => *p,
                    (true, Value::Int(n)) if *n > 0 => *n as usize,
                    // `.` : struct was heap-allocated; variable holds a Ptr
                    (false, Value::Ptr(p)) => *p,
                    _ => {
                        return Err(RuntimeError::Custom(format!(
                            "member access `{}{}`: expected a struct pointer, got {}",
                            if *is_ptr { "->" } else { "." },
                            field,
                            base_val.type_name(),
                        )))
                    },
                };

                // Find the field layout by scanning all structs.
                // We prefer the deepest match (most-recently-defined struct that
                // has the field), which is the natural resolution order.
                let field_layout = self
                    .type_env
                    .structs
                    .values()
                    .filter_map(|layout| layout.field(field))
                    .last()
                    .cloned()
                    .ok_or_else(|| RuntimeError::Custom(format!("unknown field `{field}`")))?;

                let addr = struct_ptr + field_layout.offset as usize;

                // Struct-typed fields: return a pointer to the sub-object so
                // chained access (e.g. `line.a.x`) works naturally.
                if let HolyType::Named(_) = &field_layout.ty {
                    return Ok(Value::Ptr(addr));
                }

                let raw = self.heap.read_uint(addr, field_layout.size as usize)?;
                Ok(match &field_layout.ty {
                    HolyType::F64 => Value::Float(f64::from_bits(raw)),
                    HolyType::F32 => Value::Float(f32::from_bits(raw as u32) as f64),
                    HolyType::Bool => Value::Bool(raw != 0),
                    HolyType::U8 | HolyType::U16 | HolyType::U32 | HolyType::U64 => {
                        Value::UInt(raw)
                    },
                    HolyType::Ptr(_) | HolyType::FnPtr { .. } => Value::Ptr(raw as usize),
                    _ => Value::Int(raw as i64),
                })
            },
        }
    }

    // ── Unary operators ───────────────────────────────────────────────────────

    fn eval_unary(
        &mut self,
        op: UnaryOp,
        operand: &holyc_frontend::ast::Expr,
    ) -> Result<Value, RuntimeError> {
        match op {
            UnaryOp::Neg => match self.eval_expr(operand)? {
                Value::Int(n) => Ok(Value::Int(-n)),
                Value::Float(f) => Ok(Value::Float(-f)),
                v => Err(type_error("number", v.type_name())),
            },
            UnaryOp::BitNot => match self.eval_expr(operand)? {
                Value::Int(n) => Ok(Value::Int(!n)),
                Value::UInt(n) => Ok(Value::UInt(!n)),
                v => Err(type_error("integer", v.type_name())),
            },
            UnaryOp::LogNot => {
                let v = self.eval_expr(operand)?;
                Ok(Value::Bool(!v.is_truthy()))
            },
            UnaryOp::AddrOf => {
                // For an ident, return a pointer to a heap-allocated copy of the value.
                // Full l-value address tracking is a Phase 2 item; for now we allocate
                // a temporary that lets basic pointer round-trips work.
                let val = self.eval_expr(operand)?;
                let (ptr, width) = match &val {
                    Value::Int(n) => {
                        let p = self.heap.alloc(8)?;
                        self.heap.write_uint(p, 8, *n as u64)?;
                        (p, 8)
                    },
                    Value::UInt(n) => {
                        let p = self.heap.alloc(8)?;
                        self.heap.write_uint(p, 8, *n)?;
                        (p, 8)
                    },
                    Value::Ptr(p) => return Ok(Value::Ptr(*p)),
                    _ => {
                        return Err(RuntimeError::Custom(
                            "address-of: unsupported value type".into(),
                        ))
                    },
                };
                let _ = width;
                Ok(Value::Ptr(ptr))
            },
            UnaryOp::Deref => {
                let v = self.eval_expr(operand)?;
                match v {
                    Value::Ptr(0) => Err(RuntimeError::Custom("null pointer dereference".into())),
                    Value::Ptr(p) => {
                        // Default to reading an 8-byte integer (I64 width).
                        // Typed dereference requires type annotations — Phase 2.
                        let raw = self.heap.read_uint(p, 8)?;
                        Ok(Value::Int(raw as i64))
                    },
                    _ => Err(RuntimeError::Custom(
                        "dereference of non-pointer value".into(),
                    )),
                }
            },

            UnaryOp::PreInc | UnaryOp::PreDec => {
                let delta: i64 = if op == UnaryOp::PreInc { 1 } else { -1 };
                let name = ident_name(operand)?;
                let cur =
                    self.env.get(&name).cloned().ok_or_else(|| {
                        RuntimeError::Custom(format!("undefined variable `{name}`"))
                    })?;
                let new_val = int_add(cur, delta)?;
                self.env.set(&name, new_val.clone());
                Ok(new_val)
            },
            UnaryOp::PostInc | UnaryOp::PostDec => {
                let delta: i64 = if op == UnaryOp::PostInc { 1 } else { -1 };
                let name = ident_name(operand)?;
                let old =
                    self.env.get(&name).cloned().ok_or_else(|| {
                        RuntimeError::Custom(format!("undefined variable `{name}`"))
                    })?;
                let new_val = int_add(old.clone(), delta)?;
                self.env.set(&name, new_val);
                Ok(old)
            },
        }
    }

    // ── Binary operators ──────────────────────────────────────────────────────

    fn eval_binary(
        &mut self,
        op: BinOp,
        lhs: &holyc_frontend::ast::Expr,
        rhs: &holyc_frontend::ast::Expr,
    ) -> Result<Value, RuntimeError> {
        // Short-circuit logical operators
        if op == BinOp::LogAnd {
            let l = self.eval_expr(lhs)?;
            return if !l.is_truthy() {
                Ok(Value::Bool(false))
            } else {
                Ok(Value::Bool(self.eval_expr(rhs)?.is_truthy()))
            };
        }
        if op == BinOp::LogOr {
            let l = self.eval_expr(lhs)?;
            return if l.is_truthy() {
                Ok(Value::Bool(true))
            } else {
                Ok(Value::Bool(self.eval_expr(rhs)?.is_truthy()))
            };
        }

        let l = self.eval_expr(lhs)?;
        let r = self.eval_expr(rhs)?;

        // String concatenation for `+`
        if op == BinOp::Add {
            if let (Value::Str(a), Value::Str(b)) = (&l, &r) {
                return Ok(Value::Str(format!("{a}{b}")));
            }
        }

        // Float promotion: if either operand is a float, use float arithmetic.
        if matches!((&l, &r), (Value::Float(_), _) | (_, Value::Float(_))) {
            let lf = l
                .as_float()
                .ok_or_else(|| type_error("float", l.type_name()))?;
            let rf = r
                .as_float()
                .ok_or_else(|| type_error("float", r.type_name()))?;
            return Ok(match op {
                BinOp::Add => Value::Float(lf + rf),
                BinOp::Sub => Value::Float(lf - rf),
                BinOp::Mul => Value::Float(lf * rf),
                BinOp::Div => {
                    if rf == 0.0 {
                        return Err(RuntimeError::Custom("division by zero".into()));
                    }
                    Value::Float(lf / rf)
                },
                BinOp::Eq => Value::Bool(lf == rf),
                BinOp::Ne => Value::Bool(lf != rf),
                BinOp::Lt => Value::Bool(lf < rf),
                BinOp::Le => Value::Bool(lf <= rf),
                BinOp::Gt => Value::Bool(lf > rf),
                BinOp::Ge => Value::Bool(lf >= rf),
                _ => {
                    return Err(RuntimeError::Custom(format!(
                        "operator {op:?} not defined for floats"
                    )))
                },
            });
        }

        // Integer arithmetic
        let li = l
            .as_int()
            .ok_or_else(|| type_error("integer", l.type_name()))?;
        let ri = r
            .as_int()
            .ok_or_else(|| type_error("integer", r.type_name()))?;
        Ok(match op {
            BinOp::Add => Value::Int(li.wrapping_add(ri)),
            BinOp::Sub => Value::Int(li.wrapping_sub(ri)),
            BinOp::Mul => Value::Int(li.wrapping_mul(ri)),
            BinOp::Div => {
                if ri == 0 {
                    return Err(RuntimeError::Custom("division by zero".into()));
                }
                Value::Int(li / ri)
            },
            BinOp::Rem => {
                if ri == 0 {
                    return Err(RuntimeError::Custom("division by zero".into()));
                }
                Value::Int(li % ri)
            },
            BinOp::BitAnd => Value::Int(li & ri),
            BinOp::BitOr => Value::Int(li | ri),
            BinOp::BitXor => Value::Int(li ^ ri),
            BinOp::Shl => Value::Int(li.wrapping_shl(ri as u32)),
            BinOp::Shr => Value::Int(li.wrapping_shr(ri as u32)),
            BinOp::Eq => Value::Bool(li == ri),
            BinOp::Ne => Value::Bool(li != ri),
            BinOp::Lt => Value::Bool(li < ri),
            BinOp::Le => Value::Bool(li <= ri),
            BinOp::Gt => Value::Bool(li > ri),
            BinOp::Ge => Value::Bool(li >= ri),
            BinOp::LogAnd | BinOp::LogOr => unreachable!(),
        })
    }

    // ── Assignment operators ──────────────────────────────────────────────────

    fn eval_assign(
        &mut self,
        op: AssignOp,
        lhs: &holyc_frontend::ast::Expr,
        rhs: &holyc_frontend::ast::Expr,
    ) -> Result<Value, RuntimeError> {
        // ── Pointer write: *ptr = rval ────────────────────────────────────────
        if let ExprKind::Unary {
            op: UnaryOp::Deref,
            operand,
        } = &lhs.kind
        {
            if op != AssignOp::Assign {
                return Err(RuntimeError::Custom(
                    "compound assignment through pointer not yet supported".into(),
                ));
            }
            let ptr_val = self.eval_expr(operand)?;
            let ptr = match ptr_val {
                Value::Ptr(0) => return Err(RuntimeError::Custom("null pointer write".into())),
                Value::Ptr(p) => p,
                Value::Int(n) => n as usize,
                _ => return Err(RuntimeError::Custom("write through non-pointer".into())),
            };
            let rval = self.eval_expr(rhs)?;
            let raw = match &rval {
                Value::Int(n) => *n as u64,
                Value::UInt(n) => *n,
                Value::Ptr(p) => *p as u64,
                Value::Bool(b) => *b as u64,
                Value::Char(c) => *c as u64,
                _ => {
                    return Err(RuntimeError::Custom(
                        "pointer write: unsupported value type".into(),
                    ))
                },
            };
            self.heap.write_uint(ptr, 8, raw)?;
            return Ok(rval);
        }

        // ── Member assignment: base.field = rval  or  ptr->field = rval ────────
        if let ExprKind::Member {
            base,
            field,
            is_ptr,
        } = &lhs.kind
        {
            if op != AssignOp::Assign {
                return Err(RuntimeError::Custom(
                    "compound assignment to struct field not yet supported".into(),
                ));
            }
            let base_val = self.eval_expr(base)?;
            let struct_ptr = if *is_ptr {
                match base_val {
                    Value::Ptr(0) => {
                        return Err(RuntimeError::Custom(
                            "null pointer dereference in member write".into(),
                        ))
                    },
                    Value::Ptr(p) => p,
                    Value::Int(n) if n > 0 => n as usize,
                    _ => {
                        return Err(RuntimeError::Custom(
                            "member `->`: base must be a pointer".into(),
                        ))
                    },
                }
            } else {
                match base_val {
                    Value::Ptr(p) => p,
                    _ => {
                        return Err(RuntimeError::Custom(
                            "member `.`: base must be heap-allocated".into(),
                        ))
                    },
                }
            };
            let field_layout = self
                .type_env
                .structs
                .values()
                .find_map(|layout| layout.field(field))
                .cloned()
                .ok_or_else(|| RuntimeError::Custom(format!("unknown field `{field}`")))?;

            let rval = self.eval_expr(rhs)?;
            let raw = match &rval {
                Value::Int(n) => *n as u64,
                Value::UInt(n) => *n,
                Value::Float(f) => f.to_bits(),
                Value::Ptr(p) => *p as u64,
                Value::Bool(b) => *b as u64,
                Value::Char(c) => *c as u64,
                _ => {
                    return Err(RuntimeError::Custom(
                        "member write: unsupported value type".into(),
                    ))
                },
            };
            let addr = struct_ptr + field_layout.offset as usize;
            self.heap
                .write_uint(addr, field_layout.size as usize, raw)?;
            return Ok(rval);
        }

        // ── Index assignment: arr[i] = rval ───────────────────────────────────
        if let ExprKind::Index { base, idx } = &lhs.kind {
            if op != AssignOp::Assign {
                return Err(RuntimeError::Custom(
                    "compound assignment through subscript not yet supported".into(),
                ));
            }
            let base_val = self.eval_expr(base)?;
            let idx_val = self.eval_expr(idx)?;
            let ptr = match base_val {
                Value::Ptr(p) => p,
                Value::Int(n) => n as usize,
                _ => {
                    return Err(RuntimeError::Custom(
                        "subscript assign: base must be a pointer".into(),
                    ))
                },
            };
            let i = idx_val
                .as_int()
                .ok_or_else(|| RuntimeError::Custom("subscript: index must be integer".into()))?;
            let rval = self.eval_expr(rhs)?;
            let raw = match &rval {
                Value::Int(n) => *n as u64,
                Value::UInt(n) => *n,
                Value::Ptr(p) => *p as u64,
                Value::Bool(b) => *b as u64,
                Value::Char(c) => *c as u64,
                _ => {
                    return Err(RuntimeError::Custom(
                        "subscript write: unsupported value".into(),
                    ))
                },
            };
            let offset = ptr + (i as usize) * 8;
            self.heap.write_uint(offset, 8, raw)?;
            return Ok(rval);
        }

        // ── Normal variable assignment ─────────────────────────────────────────
        let rval = self.eval_expr(rhs)?;
        let name = ident_name(lhs)?;

        let new_val = if op == AssignOp::Assign {
            rval
        } else {
            let cur = self
                .env
                .get(&name)
                .cloned()
                .ok_or_else(|| RuntimeError::Custom(format!("undefined variable `{name}`")))?;
            let bin_op = match op {
                AssignOp::Add => BinOp::Add,
                AssignOp::Sub => BinOp::Sub,
                AssignOp::Mul => BinOp::Mul,
                AssignOp::Div => BinOp::Div,
                AssignOp::Rem => BinOp::Rem,
                AssignOp::BitAnd => BinOp::BitAnd,
                AssignOp::BitOr => BinOp::BitOr,
                AssignOp::BitXor => BinOp::BitXor,
                AssignOp::Shl => BinOp::Shl,
                AssignOp::Shr => BinOp::Shr,
                AssignOp::Assign => unreachable!(),
            };
            apply_int_binop(bin_op, cur, rval)?
        };

        if !self.env.set(&name, new_val.clone()) {
            self.env.define(name, new_val.clone());
        }
        Ok(new_val)
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

fn ident_name(expr: &holyc_frontend::ast::Expr) -> Result<String, RuntimeError> {
    match &expr.kind {
        ExprKind::Ident(n) => Ok(n.clone()),
        _ => Err(RuntimeError::Custom("expression is not assignable".into())),
    }
}

fn int_add(v: Value, delta: i64) -> Result<Value, RuntimeError> {
    match v {
        Value::Int(n) => Ok(Value::Int(n.wrapping_add(delta))),
        Value::UInt(n) => Ok(Value::UInt(n.wrapping_add(delta as u64))),
        other => Err(type_error("integer", other.type_name())),
    }
}

fn apply_int_binop(op: BinOp, l: Value, r: Value) -> Result<Value, RuntimeError> {
    let li = l
        .as_int()
        .ok_or_else(|| type_error("integer", l.type_name()))?;
    let ri = r
        .as_int()
        .ok_or_else(|| type_error("integer", r.type_name()))?;
    Ok(match op {
        BinOp::Add => Value::Int(li.wrapping_add(ri)),
        BinOp::Sub => Value::Int(li.wrapping_sub(ri)),
        BinOp::Mul => Value::Int(li.wrapping_mul(ri)),
        BinOp::Div => {
            if ri == 0 {
                return Err(RuntimeError::Custom("division by zero".into()));
            }
            Value::Int(li / ri)
        },
        BinOp::Rem => {
            if ri == 0 {
                return Err(RuntimeError::Custom("division by zero".into()));
            }
            Value::Int(li % ri)
        },
        BinOp::BitAnd => Value::Int(li & ri),
        BinOp::BitOr => Value::Int(li | ri),
        BinOp::BitXor => Value::Int(li ^ ri),
        BinOp::Shl => Value::Int(li.wrapping_shl(ri as u32)),
        BinOp::Shr => Value::Int(li.wrapping_shr(ri as u32)),
        _ => {
            return Err(RuntimeError::Custom(format!(
                "{op:?} not valid for compound assignment"
            )))
        },
    })
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::UInt(x), Value::UInt(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Char(x), Value::Char(y)) => x == y,
        _ => false,
    }
}

fn type_error(expected: &str, found: &str) -> RuntimeError {
    RuntimeError::TypeError {
        expected: expected.into(),
        found: found.into(),
    }
}
