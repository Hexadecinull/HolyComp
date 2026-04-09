//! Runtime types for the HolyC tree-walk interpreter.
//!
//! [`Value`] and [`RuntimeError`] are defined in `holyc_stdlib::builtins`
//! and re-exported here so every interpreter module imports from one place.

// Re-export the canonical runtime value type and error from stdlib.
pub use holyc_stdlib::builtins::{RuntimeError, Value};

use std::collections::HashMap;
use holyc_frontend::ast::{Param, Stmt};

// ── Environment (scope chain) ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Env {
    scopes: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self {
        Env { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: impl Into<String>, val: Value) {
        self.scopes.last_mut().unwrap().insert(name.into(), val);
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) {
                return Some(v);
            }
        }
        None
    }

    /// Update an existing binding anywhere in the scope chain.
    /// Returns `true` if the variable was found, `false` if it does not exist.
    pub fn set(&mut self, name: &str, val: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_owned(), val);
                return true;
            }
        }
        false
    }
}

// ── Function definitions ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct FuncDef {
    pub params: Vec<Param>,
    pub body:   Vec<Stmt>,
}

/// A builtin function implemented in Rust.
pub type BuiltinFn = fn(&[Value]) -> Result<Value, RuntimeError>;

#[derive(Clone)]
pub enum Callable {
    UserDef(FuncDef),
    Builtin(BuiltinFn),
}

impl std::fmt::Debug for Callable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Callable::UserDef(_) => write!(f, "<user function>"),
            Callable::Builtin(_) => write!(f, "<builtin>"),
        }
    }
}

// ── Control-flow signal ───────────────────────────────────────────────────────

/// Propagates `return`, `break`, and `continue` up the call stack via `Err`.
#[derive(Debug)]
pub enum Signal {
    Return(Value),
    Break,
    Continue,
    /// A runtime error that aborts execution.
    Error(RuntimeError),
}

impl Signal {
    /// Lift a [`RuntimeError`] into a [`Signal`] for use with `map_err`.
    pub fn from_rt(e: RuntimeError) -> Self {
        Signal::Error(e)
    }
}

impl std::fmt::Display for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Signal::Return(v)  => write!(f, "return {v}"),
            Signal::Break      => write!(f, "break"),
            Signal::Continue   => write!(f, "continue"),
            Signal::Error(e)   => write!(f, "{e}"),
        }
    }
}
