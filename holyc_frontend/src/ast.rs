//! Abstract Syntax Tree nodes for HolyC.
//!
//! Every node carries a [`Span`] so diagnostics can point back to source.

use crate::{error::Span, types::HolyType};

// ── Shared helpers ────────────────────────────────────────────────────────────

/// A name together with its source span.
#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// A typed function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub ty:   HolyType,
    pub name: String,
    pub span: Span,
}

/// A struct / class field.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub ty:   HolyType,
    pub name: String,
    /// Optional bit-width for bitfields.
    pub bits: Option<u8>,
    pub span: Span,
}

/// A single case inside a `switch` statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    /// `None` = `default:`.
    pub value: Option<Expr>,
    pub body:  Vec<Stmt>,
    pub span:  Span,
}

// ── Operators ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,    // -
    BitNot, // ~
    LogNot, // !
    PreInc, // ++x
    PreDec, // --x
    PostInc,// x++
    PostDec,// x--
    Deref,  // *x
    AddrOf, // &x
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add, Sub, Mul, Div, Rem,
    // Bitwise
    BitAnd, BitOr, BitXor, Shl, Shr,
    // Logical
    LogAnd, LogOr,
    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,                             // =
    Add, Sub, Mul, Div, Rem,            // += -= *= /= %=
    BitAnd, BitOr, BitXor, Shl, Shr,   // &= |= ^= <<= >>=
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    // Literals
    IntLit(u64),
    FloatLit(f64),
    StringLit(String),
    CharLit(u8),
    BoolLit(bool),
    Null,

    // Variable / function reference
    Ident(String),

    // Operations
    Unary  { op: UnaryOp, operand: Box<Expr> },
    Binary { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
    Assign { op: AssignOp, lhs: Box<Expr>, rhs: Box<Expr> },

    // Ternary: `cond ? then : else`
    Ternary { cond: Box<Expr>, then: Box<Expr>, else_: Box<Expr> },

    // Calls and subscripting
    Call  { callee: Box<Expr>, args: Vec<Expr> },
    Index { base: Box<Expr>, idx: Box<Expr> },

    // Member access: `.` and `->`
    Member { base: Box<Expr>, field: String, is_ptr: bool },

    // Cast: `(I32)expr`
    Cast { ty: HolyType, expr: Box<Expr> },

    // sizeof
    SizeOfExpr(Box<Expr>),
    SizeOfType(HolyType),
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// A stand-alone expression, usually a call or assignment.
    Expr(Expr),

    /// Variable declaration: `I64 x = 5;`
    VarDecl {
        ty:   HolyType,
        name: String,
        init: Option<Expr>,
    },

    /// `return;` or `return expr;`
    Return(Option<Expr>),

    /// `if (cond) then_body [else else_body]`
    If {
        cond:      Expr,
        then_body: Box<Stmt>,
        else_body: Option<Box<Stmt>>,
    },

    /// `while (cond) body`
    While { cond: Expr, body: Box<Stmt> },

    /// `do body while (cond);`
    DoWhile { body: Box<Stmt>, cond: Expr },

    /// `for (init; cond; step) body`
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
    },

    /// `switch (expr) { case … }`
    Switch { expr: Expr, cases: Vec<SwitchCase> },

    Break,
    Continue,

    /// `{ stmts… }`
    Block(Vec<Stmt>),

    /// `asm { raw_text }`
    Asm(String),
}

// ── Top-level declarations ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct TopLevel {
    pub kind: TopLevelKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopLevelKind {
    /// `RetTy FuncName(Param, …) { body }`
    FuncDef {
        visibility: Visibility,
        ret_ty:     HolyType,
        name:       String,
        params:     Vec<Param>,
        body:       Vec<Stmt>,
    },

    /// Forward declaration: `RetTy FuncName(Param, …);`
    FuncDecl {
        ret_ty: HolyType,
        name:   String,
        params: Vec<Param>,
    },

    /// `I64 global_var = expr;`
    GlobalVar {
        visibility: Visibility,
        ty:         HolyType,
        name:       String,
        init:       Option<Expr>,
    },

    /// `class Foo { … };`
    ClassDef { name: String, fields: Vec<Field> },

    /// `typedef I64 MyInt;`
    TypeDef { ty: HolyType, alias: String },

    /// `#define NAME expr`
    Define { name: String, value: Option<Expr> },

    /// `#include "file.HC"` or `#include <file.HC>`
    Include { path: String, is_system: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Default,
    Public,
    Private,
}

/// The complete parse output for a single source file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Module {
    pub items: Vec<TopLevel>,
}
