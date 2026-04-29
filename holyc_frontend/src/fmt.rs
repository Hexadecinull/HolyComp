//! HolyC source formatter.
//!
//! Parses a HolyC source file and re-emits it with consistent, opinionated
//! style that mirrors TempleOS conventions:
//!
//! - 2-space indentation
//! - Opening braces on the same line as the statement
//! - One space around binary operators
//! - Spaces after commas
//! - No trailing whitespace
//!
//! The formatter is **AST-driven**: it re-serialises the parsed tree, so
//! comments and some whitespace are not preserved (use `// NOFORMAT` to
//! opt out of a region in a future version).

use std::fmt::Write;

use crate::ast::{
    AssignOp, BinOp, Expr, ExprKind, Module, Stmt, StmtKind, SwitchCase, TopLevel, TopLevelKind,
    UnaryOp, Visibility,
};
use crate::types::HolyType;

// ── Public API ────────────────────────────────────────────────────────────────

/// Format a parsed [`Module`] back to HolyC source text.
pub fn format_module(module: &Module) -> String {
    let mut f = Fmt::new();
    f.module(module);
    f.out
}

// ── Internal formatter ────────────────────────────────────────────────────────

struct Fmt {
    out: String,
    indent: usize,
}

impl Fmt {
    fn new() -> Self {
        Self {
            out: String::new(),
            indent: 0,
        }
    }

    fn write(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn nl(&mut self) {
        self.out.push('\n');
    }

    fn pad(&mut self) {
        for _ in 0..self.indent * 2 {
            self.out.push(' ');
        }
    }

    fn padwrite(&mut self, s: &str) {
        self.pad();
        self.write(s);
    }

    fn module(&mut self, m: &Module) {
        for (i, item) in m.items.iter().enumerate() {
            if i > 0 {
                self.nl();
            }
            self.top_level(item);
        }
    }

    fn top_level(&mut self, tl: &TopLevel) {
        match &tl.kind {
            TopLevelKind::FuncDef {
                visibility,
                ret_ty,
                name,
                params,
                body,
            } => {
                self.visibility(visibility);
                self.ty(ret_ty);
                write!(self.out, " {name}(").ok();
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.ty(&p.ty);
                    write!(self.out, " {}", p.name).ok();
                }
                self.writeln(") {");
                self.indent += 1;
                for s in body {
                    self.stmt(s);
                }
                self.indent -= 1;
                self.writeln("}");
            },
            TopLevelKind::FuncDecl {
                ret_ty,
                name,
                params,
            } => {
                self.ty(ret_ty);
                write!(self.out, " {name}(").ok();
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.ty(&p.ty);
                    write!(self.out, " {}", p.name).ok();
                }
                self.writeln(");");
            },
            TopLevelKind::GlobalVar {
                visibility,
                ty,
                name,
                init,
            } => {
                self.visibility(visibility);
                self.ty(ty);
                write!(self.out, " {name}").ok();
                if let Some(e) = init {
                    self.write(" = ");
                    self.expr(e);
                }
                self.writeln(";");
            },
            TopLevelKind::ClassDef { name, fields } => {
                writeln!(self.out, "class {name} {{").ok();
                for f in fields {
                    self.write("  ");
                    self.ty(&f.ty);
                    write!(self.out, " {}", f.name).ok();
                    if let Some(b) = f.bits {
                        write!(self.out, ":{b}").ok();
                    }
                    self.writeln(";");
                }
                self.writeln("};");
            },
            TopLevelKind::TypeDef { ty, alias } => {
                self.write("typedef ");
                self.ty(ty);
                writeln!(self.out, " {alias};").ok();
            },
            TopLevelKind::Define { name, value } => {
                self.write("#define ");
                self.write(name);
                if let Some(v) = value {
                    self.write(" ");
                    self.expr(v);
                }
                self.nl();
            },
            TopLevelKind::Include { path, is_system } => {
                if *is_system {
                    writeln!(self.out, "#include <{path}>").ok();
                } else {
                    writeln!(self.out, "#include \"{path}\"").ok();
                }
            },
        }
    }

    fn visibility(&mut self, v: &Visibility) {
        match v {
            Visibility::Public => self.write("public "),
            Visibility::Private => self.write("private "),
            Visibility::Default => {},
        }
    }

    fn ty(&mut self, ty: &HolyType) {
        write!(self.out, "{ty}").ok();
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Expr(e) => {
                self.pad();
                self.expr(e);
                self.writeln(";");
            },
            StmtKind::VarDecl { ty, name, init } => {
                self.pad();
                self.ty(ty);
                write!(self.out, " {name}").ok();
                if let Some(e) = init {
                    self.write(" = ");
                    self.expr(e);
                }
                self.writeln(";");
            },
            StmtKind::Return(expr) => {
                self.padwrite("return");
                if let Some(e) = expr {
                    self.write(" ");
                    self.expr(e);
                }
                self.writeln(";");
            },
            StmtKind::Break => {
                self.padwrite("break;\n");
            },
            StmtKind::Continue => {
                self.padwrite("continue;\n");
            },
            StmtKind::Block(stmts) => {
                self.padwrite("{\n");
                self.indent += 1;
                for s in stmts {
                    self.stmt(s);
                }
                self.indent -= 1;
                self.padwrite("}\n");
            },
            StmtKind::If {
                cond,
                then_body,
                else_body,
            } => {
                self.padwrite("if (");
                self.expr(cond);
                self.write(") ");
                self.stmt_inline(then_body);
                if let Some(eb) = else_body {
                    self.padwrite("else ");
                    self.stmt_inline(eb);
                }
            },
            StmtKind::While { cond, body } => {
                self.padwrite("while (");
                self.expr(cond);
                self.write(") ");
                self.stmt_inline(body);
            },
            StmtKind::DoWhile { body, cond } => {
                self.padwrite("do ");
                self.stmt_inline(body);
                self.padwrite("while (");
                self.expr(cond);
                self.writeln(");");
            },
            StmtKind::For {
                init,
                cond,
                step,
                body,
            } => {
                self.padwrite("for (");
                if let Some(i) = init {
                    self.stmt_semi_inline(i);
                } else {
                    self.write(";");
                }
                self.write(" ");
                if let Some(c) = cond {
                    self.expr(c);
                }
                self.write("; ");
                if let Some(s) = step {
                    self.expr(s);
                }
                self.write(") ");
                self.stmt_inline(body);
            },
            StmtKind::Switch { expr, cases } => {
                self.padwrite("switch (");
                self.expr(expr);
                self.writeln(") {");
                for case in cases {
                    self.switch_case(case);
                }
                self.padwrite("}\n");
            },
            StmtKind::Asm(text) => {
                self.padwrite("asm {\n");
                for line in text.lines() {
                    self.pad();
                    self.write("  ");
                    self.writeln(line.trim());
                }
                self.padwrite("}\n");
            },
        }
    }

    /// Emit a statement that immediately follows a control-flow keyword,
    /// on the same line if it's a block, or indented if it's a single statement.
    fn stmt_inline(&mut self, s: &Stmt) {
        if let StmtKind::Block(stmts) = &s.kind {
            self.writeln("{");
            self.indent += 1;
            for s in stmts {
                self.stmt(s);
            }
            self.indent -= 1;
            self.padwrite("}\n");
        } else {
            self.nl();
            self.indent += 1;
            self.stmt(s);
            self.indent -= 1;
        }
    }

    /// Emit a statement as a semicolon-terminated expression (for `for` inits).
    fn stmt_semi_inline(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::VarDecl { ty, name, init } => {
                self.ty(ty);
                write!(self.out, " {name}").ok();
                if let Some(e) = init {
                    self.write(" = ");
                    self.expr(e);
                }
                self.write(";");
            },
            StmtKind::Expr(e) => {
                self.expr(e);
                self.write(";");
            },
            _ => self.write(";"),
        }
    }

    fn switch_case(&mut self, c: &SwitchCase) {
        self.pad();
        if let Some(v) = &c.value {
            self.write("case ");
            self.expr(v);
            self.writeln(":");
        } else {
            self.writeln("default:");
        }
        self.indent += 1;
        for s in &c.body {
            self.stmt(s);
        }
        self.indent -= 1;
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::IntLit(n) => write!(self.out, "{n}").ok(),
            ExprKind::FloatLit(f) => write!(self.out, "{f}").ok(),
            ExprKind::StringLit(s) => write!(self.out, "\"{}\"", escape_str(s)).ok(),
            ExprKind::CharLit(c) => {
                let escaped = match *c {
                    b'\n' => "\\n".to_owned(),
                    b'\t' => "\\t".to_owned(),
                    b'\r' => "\\r".to_owned(),
                    b'\0' => "\\0".to_owned(),
                    b'\\' => "\\\\".to_owned(),
                    b'\'' => "\\'".to_owned(),
                    c => (c as char).to_string(),
                };
                write!(self.out, "'{escaped}'").ok()
            },
            ExprKind::BoolLit(b) => write!(self.out, "{}", if *b { "TRUE" } else { "FALSE" }).ok(),
            ExprKind::Null => write!(self.out, "NULL").ok(),
            ExprKind::Ident(n) => write!(self.out, "{n}").ok(),

            ExprKind::Unary { op, operand } => {
                match op {
                    UnaryOp::PostInc => {
                        self.expr(operand);
                        self.write("++");
                    },
                    UnaryOp::PostDec => {
                        self.expr(operand);
                        self.write("--");
                    },
                    _ => {
                        self.write(unary_prefix(op));
                        self.expr_paren(operand, 30);
                    },
                }
                None
            },

            ExprKind::Binary { op, lhs, rhs } => {
                self.expr_paren(lhs, binop_prec(op) + 1);
                write!(self.out, " {} ", binop_str(op)).ok();
                self.expr_paren(rhs, binop_prec(op));
                None
            },

            ExprKind::Assign { op, lhs, rhs } => {
                self.expr(lhs);
                write!(self.out, " {} ", assign_str(op)).ok();
                self.expr(rhs);
                None
            },

            ExprKind::Ternary { cond, then, else_ } => {
                self.expr(cond);
                self.write(" ? ");
                self.expr(then);
                self.write(" : ");
                self.expr(else_);
                None
            },

            ExprKind::Call { callee, args } => {
                self.expr(callee);
                self.write("(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.expr(a);
                }
                self.write(")");
                None
            },

            ExprKind::Index { base, idx } => {
                self.expr(base);
                self.write("[");
                self.expr(idx);
                self.write("]");
                None
            },

            ExprKind::Member {
                base,
                field,
                is_ptr,
            } => {
                self.expr(base);
                self.write(if *is_ptr { "->" } else { "." });
                self.write(field);
                None
            },

            ExprKind::Cast { ty, expr } => {
                self.write("(");
                self.ty(ty);
                self.write(")");
                self.expr(expr);
                None
            },

            ExprKind::SizeOfType(ty) => {
                self.write("sizeof(");
                self.ty(ty);
                self.write(")");
                None
            },

            ExprKind::SizeOfExpr(e) => {
                self.write("sizeof(");
                self.expr(e);
                self.write(")");
                None
            },
        };
    }

    /// Emit `expr`, wrapping in parentheses if its precedence is below `min_prec`.
    fn expr_paren(&mut self, e: &Expr, min_prec: u8) {
        let prec = expr_prec(e);
        if prec < min_prec {
            self.write("(");
            self.expr(e);
            self.write(")");
        } else {
            self.expr(e);
        }
    }
}

// ── Precedence / string helpers ───────────────────────────────────────────────

fn expr_prec(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Binary { op, .. } => binop_prec(op),
        ExprKind::Unary { .. } => 30,
        ExprKind::Assign { .. } | ExprKind::Ternary { .. } => 2,
        _ => 40,
    }
}

fn binop_prec(op: &BinOp) -> u8 {
    match op {
        BinOp::LogOr => 4,
        BinOp::LogAnd => 6,
        BinOp::BitOr => 8,
        BinOp::BitXor => 10,
        BinOp::BitAnd => 12,
        BinOp::Eq | BinOp::Ne => 14,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 16,
        BinOp::Shl | BinOp::Shr => 18,
        BinOp::Add | BinOp::Sub => 20,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 22,
    }
}

fn binop_str(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Rem => "%",
        BinOp::BitAnd => "&",
        BinOp::BitOr => "|",
        BinOp::BitXor => "^",
        BinOp::Shl => "<<",
        BinOp::Shr => ">>",
        BinOp::LogAnd => "&&",
        BinOp::LogOr => "||",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
    }
}

fn assign_str(op: &AssignOp) -> &'static str {
    match op {
        AssignOp::Assign => "=",
        AssignOp::Add => "+=",
        AssignOp::Sub => "-=",
        AssignOp::Mul => "*=",
        AssignOp::Div => "/=",
        AssignOp::Rem => "%=",
        AssignOp::BitAnd => "&=",
        AssignOp::BitOr => "|=",
        AssignOp::BitXor => "^=",
        AssignOp::Shl => "<<=",
        AssignOp::Shr => ">>=",
    }
}

fn unary_prefix(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::BitNot => "~",
        UnaryOp::LogNot => "!",
        UnaryOp::PreInc => "++",
        UnaryOp::PreDec => "--",
        UnaryOp::Deref => "*",
        UnaryOp::AddrOf => "&",
        UnaryOp::PostInc | UnaryOp::PostDec => "",
    }
}

// ── String / char escaping ───────────────────────────────────────────────────

/// Re-escape a string value so it can be safely embedded in a HolyC string
/// literal (e.g. `\n` → `\n`, `"` → `\"`).
fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => out.push_str(r"\n"),
            '\t' => out.push_str(r"\t"),
            '\r' => out.push_str(r"\r"),
            '\0' => out.push_str(r"\0"),
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str("\\\""),
            c => out.push(c),
        }
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Lexer, Parser};

    fn parse(src: &str) -> Module {
        let tokens = Lexer::new(src).tokenize().expect("lex");
        Parser::new(tokens).parse_module().expect("parse")
    }

    /// Strip span byte-offsets from a Debug string so ASTs can be compared
    /// structurally without caring about source positions.
    fn strip_spans(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            out.push(c);
            if out.ends_with("span: (") {
                for ch in chars.by_ref() {
                    if ch == ')' {
                        break;
                    }
                }
                out.push_str("0, 0)");
            }
        }
        out
    }

    fn roundtrip(src: &str) {
        let m1 = parse(src);
        let formatted = format_module(&m1);
        let m2 = parse(&formatted);
        assert_eq!(
            strip_spans(&format!("{m1:?}")),
            strip_spans(&format!("{m2:?}")),
            "roundtrip changed AST.\nFormatted:\n{formatted}"
        );
    }

    #[test]
    fn fmt_hello() {
        roundtrip("U0 Main() { Print(\"Hello, World!\\n\"); }");
    }

    #[test]
    fn fmt_arithmetic() {
        roundtrip("I64 F(I64 a, I64 b) { return a + b * 2; }");
    }

    #[test]
    fn fmt_if_else() {
        roundtrip("U0 F(I64 x) { if (x > 0) { x = 1; } else { x = -1; } }");
    }

    #[test]
    fn fmt_for_loop() {
        roundtrip("U0 F() { for (I64 i = 0; i < 10; i++) { Print(\"%d\\n\", i); } }");
    }

    #[test]
    fn fmt_struct() {
        roundtrip("class Point { I64 x; I64 y; };");
    }

    #[test]
    fn fmt_typedef() {
        roundtrip("typedef I64 Score;");
    }

    #[test]
    fn fmt_ternary() {
        roundtrip("I64 Abs(I64 x) { return x >= 0 ? x : -x; }");
    }

    #[test]
    fn fmt_switch() {
        roundtrip(
            "U0 F(I64 n) { switch (n) { case 1: Print(\"one\\n\"); break; default: break; } }",
        );
    }
}
