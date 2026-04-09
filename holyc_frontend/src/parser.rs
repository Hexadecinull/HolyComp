//! Recursive-descent parser for HolyC, with Pratt-style expression parsing.
//!
//! Entry point: [`Parser::parse_module`].

use crate::{
    ast::*,
    error::{ParseError, Span, Spanned},
    token::Token,
    types::HolyType,
};

// ── Parser state ──────────────────────────────────────────────────────────────

pub struct Parser {
    tokens:  Vec<Spanned<Token>>,
    cursor:  usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned<Token>>) -> Self {
        Parser { tokens, cursor: 0 }
    }

    // ── Top-level entry point ────────────────────────────────────────────────

    pub fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut items = Vec::new();
        while !self.is_eof() {
            items.push(self.parse_top_level()?);
        }
        Ok(Module { items })
    }

    // ── Top-level declarations ────────────────────────────────────────────────

    fn parse_top_level(&mut self) -> Result<TopLevel, ParseError> {
        let start = self.cur_span().0;

        // #define / #include
        if let Some(tl) = self.try_parse_preprocessor()? {
            return Ok(tl);
        }

        // class / struct definition
        if self.check(Token::Class) || self.check(Token::Union) {
            return self.parse_class_def(start);
        }

        // typedef
        if self.check(Token::Typedef) {
            return self.parse_typedef(start);
        }

        // Visibility modifier
        let visibility = if self.eat(Token::Public)   { Visibility::Public   }
                    else if self.eat(Token::Private)  { Visibility::Private  }
                    else                              { Visibility::Default  };

        // Type + name
        let ty   = self.parse_type()?;
        let name = self.expect_ident("declaration name")?;

        // Function definition or declaration
        if self.check(Token::LParen) {
            let params = self.parse_param_list()?;
            if self.check(Token::LBrace) {
                let body = self.parse_block_body()?;
                let span = (start, self.prev_span().1);
                return Ok(TopLevel {
                    kind: TopLevelKind::FuncDef { visibility, ret_ty: ty, name, params, body },
                    span,
                });
            } else {
                self.expect(Token::Semi, "`;` after function declaration")?;
                let span = (start, self.prev_span().1);
                return Ok(TopLevel {
                    kind: TopLevelKind::FuncDecl { ret_ty: ty, name, params },
                    span,
                });
            }
        }

        // Global variable
        let init = if self.eat(Token::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect(Token::Semi, "`;` after global variable")?;
        let span = (start, self.prev_span().1);
        Ok(TopLevel {
            kind: TopLevelKind::GlobalVar { visibility, ty, name, init },
            span,
        })
    }

    fn try_parse_preprocessor(&mut self) -> Result<Option<TopLevel>, ParseError> {
        let start = self.cur_span().0;
        match self.cur_tok().clone() {
            Token::Define => {
                self.advance();
                let name = self.expect_ident("#define name")?;
                // optional value expression on the same conceptual line
                let value = if !self.check(Token::Semi) && !self.is_eof() {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                let _ = self.eat(Token::Semi); // optional
                Ok(Some(TopLevel {
                    kind: TopLevelKind::Define { name, value },
                    span: (start, self.prev_span().1),
                }))
            }
            Token::Include => {
                self.advance();
                // expect `"path"` or `<path>`
                match self.cur_tok().clone() {
                    Token::StringLit(path) => {
                        self.advance();
                        Ok(Some(TopLevel {
                            kind: TopLevelKind::Include { path, is_system: false },
                            span: (start, self.prev_span().1),
                        }))
                    }
                    Token::Lt => {
                        self.advance();
                        let name = self.expect_ident("include path")?;
                        self.expect(Token::Gt, "`>` to close include path")?;
                        Ok(Some(TopLevel {
                            kind: TopLevelKind::Include { path: name, is_system: true },
                            span: (start, self.prev_span().1),
                        }))
                    }
                    other => Err(ParseError::UnexpectedToken {
                        found:    other.describe(),
                        expected: "include path string".into(),
                        line:     self.cur_line(),
                    }),
                }
            }
            _ => Ok(None),
        }
    }

    fn parse_class_def(&mut self, start: usize) -> Result<TopLevel, ParseError> {
        self.advance(); // consume `class` / `union`
        let name = self.expect_ident("class name")?;
        self.expect(Token::LBrace, "`{` to open class body")?;
        let mut fields = Vec::new();
        while !self.check(Token::RBrace) && !self.is_eof() {
            let fspan = self.cur_span();
            let ty   = self.parse_type()?;
            // possibly multiple names: `I64 x, y;`
            loop {
                let fname = self.expect_ident("field name")?;
                let bits  = if self.eat(Token::Colon) {
                    let n = match self.cur_tok().clone() {
                        Token::IntLit(n) => { self.advance(); n as u8 }
                        _ => return Err(self.unexpected("bitfield width")),
                    };
                    Some(n)
                } else {
                    None
                };
                fields.push(Field { ty: ty.clone(), name: fname, bits, span: (fspan.0, self.prev_span().1) });
                if !self.eat(Token::Comma) { break; }
            }
            self.expect(Token::Semi, "`;` after field declaration")?;
        }
        self.expect(Token::RBrace, "`}` to close class body")?;
        let _ = self.eat(Token::Semi);
        Ok(TopLevel {
            kind: TopLevelKind::ClassDef { name, fields },
            span: (start, self.prev_span().1),
        })
    }

    fn parse_typedef(&mut self, start: usize) -> Result<TopLevel, ParseError> {
        self.advance(); // consume `typedef`
        let ty    = self.parse_type()?;
        let alias = self.expect_ident("typedef alias")?;
        self.expect(Token::Semi, "`;` after typedef")?;
        Ok(TopLevel {
            kind: TopLevelKind::TypeDef { ty, alias },
            span: (start, self.prev_span().1),
        })
    }

    // ── Types ─────────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Result<HolyType, ParseError> {
        let base = match self.cur_tok().clone() {
            Token::I8   => { self.advance(); HolyType::I8  }
            Token::U8   => { self.advance(); HolyType::U8  }
            Token::I16  => { self.advance(); HolyType::I16 }
            Token::U16  => { self.advance(); HolyType::U16 }
            Token::I32  => { self.advance(); HolyType::I32 }
            Token::U32  => { self.advance(); HolyType::U32 }
            Token::I64  => { self.advance(); HolyType::I64 }
            Token::U64  => { self.advance(); HolyType::U64 }
            Token::F32  => { self.advance(); HolyType::F32 }
            Token::F64  => { self.advance(); HolyType::F64 }
            Token::Bool => { self.advance(); HolyType::Bool }
            Token::U0   => { self.advance(); HolyType::Void }
            Token::Ident(name) => { self.advance(); HolyType::Named(name) }
            other => return Err(ParseError::UnexpectedToken {
                found:    other.describe(),
                expected: "type name".into(),
                line:     self.cur_line(),
            }),
        };
        // Pointer suffix(es): `I64*`, `I64**`, …
        let mut ty = base;
        while self.eat(Token::Star) {
            ty = HolyType::Ptr(Box::new(ty));
        }
        Ok(ty)
    }

    // ── Parameter lists ───────────────────────────────────────────────────────

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        self.expect(Token::LParen, "`(` to open parameter list")?;
        let mut params = Vec::new();
        if !self.check(Token::RParen) {
            loop {
                let span  = self.cur_span();
                let ty    = self.parse_type()?;
                let name  = self.expect_ident("parameter name")?;
                params.push(Param { ty, name, span: (span.0, self.prev_span().1) });
                if !self.eat(Token::Comma) { break; }
            }
        }
        self.expect(Token::RParen, "`)` to close parameter list")?;
        Ok(params)
    }

    // ── Statements ────────────────────────────────────────────────────────────

    /// Parse the body of a `{…}` block and return the inner statements.
    fn parse_block_body(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect(Token::LBrace, "`{` to open block")?;
        let mut stmts = Vec::new();
        while !self.check(Token::RBrace) && !self.is_eof() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(Token::RBrace, "`}` to close block")?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.cur_span().0;

        match self.cur_tok().clone() {
            // Block
            Token::LBrace => {
                let body = self.parse_block_body()?;
                return Ok(Stmt { kind: StmtKind::Block(body), span: (start, self.prev_span().1) });
            }

            // if
            Token::If => {
                self.advance();
                self.expect(Token::LParen, "`(` after `if`")?;
                let cond = self.parse_expr()?;
                self.expect(Token::RParen, "`)` after if-condition")?;
                let then_body = Box::new(self.parse_stmt()?);
                let else_body = if self.eat(Token::Else) {
                    Some(Box::new(self.parse_stmt()?))
                } else {
                    None
                };
                return Ok(Stmt {
                    kind: StmtKind::If { cond, then_body, else_body },
                    span: (start, self.prev_span().1),
                });
            }

            // while
            Token::While => {
                self.advance();
                self.expect(Token::LParen, "`(` after `while`")?;
                let cond = self.parse_expr()?;
                self.expect(Token::RParen, "`)` after while-condition")?;
                let body = Box::new(self.parse_stmt()?);
                return Ok(Stmt {
                    kind: StmtKind::While { cond, body },
                    span: (start, self.prev_span().1),
                });
            }

            // do … while
            Token::Do => {
                self.advance();
                let body = Box::new(self.parse_stmt()?);
                self.expect(Token::While, "`while` after do-body")?;
                self.expect(Token::LParen, "`(` after do-while")?;
                let cond = self.parse_expr()?;
                self.expect(Token::RParen, "`)` after do-while condition")?;
                self.expect(Token::Semi, "`;` after do-while")?;
                return Ok(Stmt {
                    kind: StmtKind::DoWhile { body, cond },
                    span: (start, self.prev_span().1),
                });
            }

            // for
            Token::For => {
                self.advance();
                self.expect(Token::LParen, "`(` after `for`")?;
                let init = if self.check(Token::Semi) {
                    None
                } else {
                    Some(Box::new(self.parse_stmt_no_semi()?))
                };
                self.expect(Token::Semi, "`;` in for-init")?;
                let cond = if self.check(Token::Semi) { None } else { Some(self.parse_expr()?) };
                self.expect(Token::Semi, "`;` in for-condition")?;
                let step = if self.check(Token::RParen) { None } else { Some(self.parse_expr()?) };
                self.expect(Token::RParen, "`)` to close for-header")?;
                let body = Box::new(self.parse_stmt()?);
                return Ok(Stmt {
                    kind: StmtKind::For { init, cond, step, body },
                    span: (start, self.prev_span().1),
                });
            }

            // switch
            Token::Switch => {
                self.advance();
                self.expect(Token::LParen, "`(` after `switch`")?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen, "`)` after switch-expression")?;
                self.expect(Token::LBrace, "`{` after switch header")?;
                let mut cases = Vec::new();
                while !self.check(Token::RBrace) && !self.is_eof() {
                    let cspan = self.cur_span();
                    let value = if self.eat(Token::Case) {
                        let v = self.parse_expr()?;
                        self.expect(Token::Colon, "`:` after case value")?;
                        Some(v)
                    } else if self.eat(Token::Default) {
                        self.expect(Token::Colon, "`:` after default")?;
                        None
                    } else {
                        return Err(self.unexpected("case or default"));
                    };
                    let mut body = Vec::new();
                    while !self.check(Token::Case)
                        && !self.check(Token::Default)
                        && !self.check(Token::RBrace)
                        && !self.is_eof()
                    {
                        body.push(self.parse_stmt()?);
                    }
                    cases.push(SwitchCase {
                        value,
                        body,
                        span: (cspan.0, self.prev_span().1),
                    });
                }
                self.expect(Token::RBrace, "`}` to close switch")?;
                return Ok(Stmt {
                    kind: StmtKind::Switch { expr, cases },
                    span: (start, self.prev_span().1),
                });
            }

            // return
            Token::Return => {
                self.advance();
                let val = if self.check(Token::Semi) { None } else { Some(self.parse_expr()?) };
                self.expect(Token::Semi, "`;` after return")?;
                return Ok(Stmt {
                    kind: StmtKind::Return(val),
                    span: (start, self.prev_span().1),
                });
            }

            // break / continue
            Token::Break    => { self.advance(); self.expect(Token::Semi, "`;`")?;
                return Ok(Stmt { kind: StmtKind::Break, span: (start, self.prev_span().1) }); }
            Token::Continue => { self.advance(); self.expect(Token::Semi, "`;`")?;
                return Ok(Stmt { kind: StmtKind::Continue, span: (start, self.prev_span().1) }); }

            // asm { … }
            Token::Asm => {
                self.advance();
                self.expect(Token::LBrace, "`{` after asm")?;
                // Collect raw source until matching `}`
                let asm_start = self.cur_span().0;
                let mut depth = 1u32;
                while depth > 0 && !self.is_eof() {
                    if self.check(Token::LBrace)  { depth += 1; }
                    if self.check(Token::RBrace)  { depth -= 1; if depth == 0 { break; } }
                    self.advance();
                }
                let asm_text = String::new(); // placeholder – real impl would capture raw text
                let _ = asm_start;
                self.expect(Token::RBrace, "`}` to close asm block")?;
                return Ok(Stmt {
                    kind: StmtKind::Asm(asm_text),
                    span: (start, self.prev_span().1),
                });
            }

            _ => {}
        }

        // Declaration: starts with a type keyword or a known identifier type
        if self.cur_tok().is_type_keyword() {
            let ty   = self.parse_type()?;
            let name = self.expect_ident("variable name")?;
            // Optional array size
            let ty = if self.eat(Token::LBracket) {
                let len = if self.check(Token::RBracket) {
                    None
                } else {
                    match self.cur_tok().clone() {
                        Token::IntLit(n) => { self.advance(); Some(n) }
                        _ => return Err(self.unexpected("array length")),
                    }
                };
                self.expect(Token::RBracket, "`]` after array size")?;
                HolyType::Array { elem: Box::new(ty), len }
            } else {
                ty
            };
            let init = if self.eat(Token::Eq) { Some(self.parse_expr()?) } else { None };
            self.expect(Token::Semi, "`;` after variable declaration")?;
            return Ok(Stmt {
                kind: StmtKind::VarDecl { ty, name, init },
                span: (start, self.prev_span().1),
            });
        }

        // Expression statement
        let expr = self.parse_expr()?;
        self.expect(Token::Semi, "`;` after expression")?;
        Ok(Stmt {
            kind: StmtKind::Expr(expr),
            span: (start, self.prev_span().1),
        })
    }

    /// Parse a statement that does NOT consume the trailing `;` (for `for` init).
    fn parse_stmt_no_semi(&mut self) -> Result<Stmt, ParseError> {
        let start = self.cur_span().0;
        if self.cur_tok().is_type_keyword() {
            let ty   = self.parse_type()?;
            let name = self.expect_ident("variable name")?;
            let init = if self.eat(Token::Eq) { Some(self.parse_expr()?) } else { None };
            return Ok(Stmt {
                kind: StmtKind::VarDecl { ty, name, init },
                span: (start, self.prev_span().1),
            });
        }
        let expr = self.parse_expr()?;
        Ok(Stmt { kind: StmtKind::Expr(expr), span: (start, self.prev_span().1) })
    }

    // ── Pratt expression parser ───────────────────────────────────────────────

    pub fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_pratt(0)
    }

    fn parse_pratt(&mut self, min_bp: u8) -> Result<Expr, ParseError> {
        let start = self.cur_span().0;
        let mut lhs = self.parse_prefix()?;

        loop {
            // Postfix operators: `++`, `--`, `(`, `[`, `.`, `->`
            let lhs_end = self.prev_span().1;

            if self.check(Token::PlusPlus) || self.check(Token::MinusMinus) {
                let op = if self.eat(Token::PlusPlus) { UnaryOp::PostInc } else { self.advance(); UnaryOp::PostDec };
                lhs = Expr {
                    kind: ExprKind::Unary { op, operand: Box::new(lhs) },
                    span: (start, self.prev_span().1),
                };
                continue;
            }

            if self.check(Token::LParen) {
                self.advance();
                let mut args = Vec::new();
                if !self.check(Token::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.eat(Token::Comma) { break; }
                    }
                }
                self.expect(Token::RParen, "`)` to close call")?;
                lhs = Expr {
                    kind: ExprKind::Call { callee: Box::new(lhs), args },
                    span: (start, self.prev_span().1),
                };
                continue;
            }

            if self.check(Token::LBracket) {
                self.advance();
                let idx = self.parse_expr()?;
                self.expect(Token::RBracket, "`]` after subscript")?;
                lhs = Expr {
                    kind: ExprKind::Index { base: Box::new(lhs), idx: Box::new(idx) },
                    span: (start, self.prev_span().1),
                };
                continue;
            }

            if self.check(Token::Dot) || self.check(Token::Arrow) {
                let is_ptr = self.check(Token::Arrow);
                self.advance();
                let field = self.expect_ident("field name")?;
                lhs = Expr {
                    kind: ExprKind::Member { base: Box::new(lhs), field, is_ptr },
                    span: (start, self.prev_span().1),
                };
                continue;
            }

            // Infix binary / assignment operators
            let (l_bp, r_bp, op_kind) = match self.cur_tok() {
                Token::Eq          => (2, 1, OpKind::Assign(AssignOp::Assign)),
                Token::PlusEq      => (2, 1, OpKind::Assign(AssignOp::Add)),
                Token::MinusEq     => (2, 1, OpKind::Assign(AssignOp::Sub)),
                Token::StarEq      => (2, 1, OpKind::Assign(AssignOp::Mul)),
                Token::SlashEq     => (2, 1, OpKind::Assign(AssignOp::Div)),
                Token::PercentEq   => (2, 1, OpKind::Assign(AssignOp::Rem)),
                Token::AmpEq       => (2, 1, OpKind::Assign(AssignOp::BitAnd)),
                Token::PipeEq      => (2, 1, OpKind::Assign(AssignOp::BitOr)),
                Token::CaretEq     => (2, 1, OpKind::Assign(AssignOp::BitXor)),
                Token::ShlEq       => (2, 1, OpKind::Assign(AssignOp::Shl)),
                Token::ShrEq       => (2, 1, OpKind::Assign(AssignOp::Shr)),
                Token::Question    => (3, 0, OpKind::Ternary),
                Token::PipePipe    => (4, 5, OpKind::Bin(BinOp::LogOr)),
                Token::AmpAmp      => (6, 7, OpKind::Bin(BinOp::LogAnd)),
                Token::Pipe        => (8, 9, OpKind::Bin(BinOp::BitOr)),
                Token::Caret       => (10,11, OpKind::Bin(BinOp::BitXor)),
                Token::Amp         => (12,13, OpKind::Bin(BinOp::BitAnd)),
                Token::EqEq        => (14,15, OpKind::Bin(BinOp::Eq)),
                Token::BangEq      => (14,15, OpKind::Bin(BinOp::Ne)),
                Token::Lt          => (16,17, OpKind::Bin(BinOp::Lt)),
                Token::LtEq        => (16,17, OpKind::Bin(BinOp::Le)),
                Token::Gt          => (16,17, OpKind::Bin(BinOp::Gt)),
                Token::GtEq        => (16,17, OpKind::Bin(BinOp::Ge)),
                Token::Shl         => (18,19, OpKind::Bin(BinOp::Shl)),
                Token::Shr         => (18,19, OpKind::Bin(BinOp::Shr)),
                Token::Plus        => (20,21, OpKind::Bin(BinOp::Add)),
                Token::Minus       => (20,21, OpKind::Bin(BinOp::Sub)),
                Token::Star        => (22,23, OpKind::Bin(BinOp::Mul)),
                Token::Slash       => (22,23, OpKind::Bin(BinOp::Div)),
                Token::Percent     => (22,23, OpKind::Bin(BinOp::Rem)),
                _ => break,
            };

            if l_bp < min_bp { break; }

            self.advance();

            let span_start = start;
            match op_kind {
                OpKind::Bin(bin_op) => {
                    let rhs = self.parse_pratt(r_bp)?;
                    lhs = Expr {
                        kind: ExprKind::Binary { op: bin_op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                        span: (span_start, self.prev_span().1),
                    };
                }
                OpKind::Assign(aop) => {
                    let rhs = self.parse_pratt(r_bp)?;
                    lhs = Expr {
                        kind: ExprKind::Assign { op: aop, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                        span: (span_start, self.prev_span().1),
                    };
                }
                OpKind::Ternary => {
                    let then_e = self.parse_expr()?;
                    self.expect(Token::Colon, "`:` in ternary")?;
                    let else_e = self.parse_pratt(r_bp)?;
                    lhs = Expr {
                        kind: ExprKind::Ternary {
                            cond:  Box::new(lhs),
                            then:  Box::new(then_e),
                            else_: Box::new(else_e),
                        },
                        span: (span_start, self.prev_span().1),
                    };
                }
            }

            let _ = lhs_end;
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        let start = self.cur_span().0;

        match self.cur_tok().clone() {
            // Unary prefix operators
            Token::Bang  => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::LogNot, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }
            Token::Tilde => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::BitNot, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }
            Token::Minus => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::Neg, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }
            Token::Star  => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::Deref, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }
            Token::Amp   => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::AddrOf, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }
            Token::PlusPlus  => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::PreInc, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }
            Token::MinusMinus => { self.advance(); let e = self.parse_pratt(30)?;
                Ok(Expr { kind: ExprKind::Unary { op: UnaryOp::PreDec, operand: Box::new(e) }, span: (start, self.prev_span().1) }) }

            // sizeof
            Token::Sizeof => {
                self.advance();
                self.expect(Token::LParen, "`(` after sizeof")?;
                // Try to parse as type; fall back to expression
                let kind = if self.cur_tok().is_type_keyword() {
                    let ty = self.parse_type()?;
                    ExprKind::SizeOfType(ty)
                } else {
                    let e = self.parse_expr()?;
                    ExprKind::SizeOfExpr(Box::new(e))
                };
                self.expect(Token::RParen, "`)` after sizeof")?;
                Ok(Expr { kind, span: (start, self.prev_span().1) })
            }

            // Grouped expression / cast: `(expr)` or `(Type)expr`
            Token::LParen => {
                self.advance();
                // Cast: `(TypeKeyword)`
                if self.cur_tok().is_type_keyword() {
                    let ty = self.parse_type()?;
                    self.expect(Token::RParen, "`)` after cast type")?;
                    let e = self.parse_pratt(30)?;
                    return Ok(Expr {
                        kind: ExprKind::Cast { ty, expr: Box::new(e) },
                        span: (start, self.prev_span().1),
                    });
                }
                let inner = self.parse_expr()?;
                self.expect(Token::RParen, "`)` to close grouped expression")?;
                Ok(Expr { kind: inner.kind, span: (start, self.prev_span().1) })
            }

            // Literals
            Token::IntLit(n)    => { let n = n; self.advance(); Ok(Expr { kind: ExprKind::IntLit(n),   span: (start, self.prev_span().1) }) }
            Token::FloatLit(f)  => { let f = f; self.advance(); Ok(Expr { kind: ExprKind::FloatLit(f), span: (start, self.prev_span().1) }) }
            Token::StringLit(s) => { let s = s; self.advance(); Ok(Expr { kind: ExprKind::StringLit(s), span: (start, self.prev_span().1) }) }
            Token::CharLit(c)   => { let c = c; self.advance(); Ok(Expr { kind: ExprKind::CharLit(c),  span: (start, self.prev_span().1) }) }
            Token::True         => { self.advance(); Ok(Expr { kind: ExprKind::BoolLit(true),  span: (start, self.prev_span().1) }) }
            Token::False        => { self.advance(); Ok(Expr { kind: ExprKind::BoolLit(false), span: (start, self.prev_span().1) }) }
            Token::Null         => { self.advance(); Ok(Expr { kind: ExprKind::Null,           span: (start, self.prev_span().1) }) }

            // Identifier
            Token::Ident(name) => {
                let name = name;
                self.advance();
                Ok(Expr { kind: ExprKind::Ident(name), span: (start, self.prev_span().1) })
            }

            other => Err(ParseError::UnexpectedToken {
                found:    other.describe(),
                expected: "expression".into(),
                line:     self.cur_line(),
            }),
        }
    }

    // ── Token stream helpers ──────────────────────────────────────────────────

    fn cur_tok(&self) -> &Token {
        &self.tokens[self.cursor].0
    }

    fn cur_span(&self) -> Span {
        self.tokens[self.cursor].1
    }

    fn prev_span(&self) -> Span {
        if self.cursor == 0 { (0, 0) }
        else { self.tokens[self.cursor - 1].1 }
    }

    fn cur_line(&self) -> u32 {
        // We don't track line in span; return a placeholder.
        // A real impl would carry line info in Spanned.
        0
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.tokens.len() || *self.cur_tok() == Token::Eof
    }

    fn advance(&mut self) {
        if !self.is_eof() {
            self.cursor += 1;
        }
    }

    fn check(&self, tok: Token) -> bool {
        *self.cur_tok() == tok
    }

    /// Consume if the current token matches, returning `true`.
    fn eat(&mut self, tok: Token) -> bool {
        if self.check(tok) { self.advance(); true } else { false }
    }

    fn expect(&mut self, tok: Token, context: &str) -> Result<(), ParseError> {
        if self.check(tok) {
            self.advance();
            Ok(())
        } else if self.is_eof() {
            Err(ParseError::UnexpectedEof { expected: context.into() })
        } else {
            Err(ParseError::UnexpectedToken {
                found:    self.cur_tok().describe(),
                expected: context.into(),
                line:     self.cur_line(),
            })
        }
    }

    fn expect_ident(&mut self, context: &str) -> Result<String, ParseError> {
        match self.cur_tok().clone() {
            Token::Ident(name) => { self.advance(); Ok(name) }
            _ if self.is_eof() => Err(ParseError::UnexpectedEof { expected: context.into() }),
            _ => Err(ParseError::UnexpectedToken {
                found:    self.cur_tok().describe(),
                expected: context.into(),
                line:     self.cur_line(),
            }),
        }
    }

    fn unexpected(&self, expected: &str) -> ParseError {
        if self.is_eof() {
            ParseError::UnexpectedEof { expected: expected.into() }
        } else {
            ParseError::UnexpectedToken {
                found:    self.cur_tok().describe(),
                expected: expected.into(),
                line:     self.cur_line(),
            }
        }
    }
}

// ── Internal helper ───────────────────────────────────────────────────────────

enum OpKind {
    Bin(BinOp),
    Assign(AssignOp),
    Ternary,
}
