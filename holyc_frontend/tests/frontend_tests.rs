use holyc_frontend::{token::Token, Lexer, Parser};

// ── Lexer tests ───────────────────────────────────────────────────────────────

#[test]
fn lex_empty_source() {
    let mut lex = Lexer::new("");
    let tokens = lex.tokenize().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].0, Token::Eof);
}

#[test]
fn lex_integer_literals() {
    let mut lex = Lexer::new("0  42  0xFF  0xDEAD");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::IntLit(0));
    assert_eq!(toks[1], Token::IntLit(42));
    assert_eq!(toks[2], Token::IntLit(0xFF));
    assert_eq!(toks[3], Token::IntLit(0xDEAD));
}

#[test]
fn lex_float_literal() {
    let mut lex = Lexer::new("1.25 4.0e10");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert!(matches!(toks[0], Token::FloatLit(f) if (f - 1.25).abs() < 1e-9));
    assert!(matches!(toks[1], Token::FloatLit(f) if (f - 4.0e10).abs() < 1e3));
}

#[test]
fn lex_string_literal() {
    let mut lex = Lexer::new(r#""hello\nworld""#);
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::StringLit("hello\nworld".into()));
}

#[test]
fn lex_char_literal() {
    let mut lex = Lexer::new("'A'  '\\n'");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::CharLit(b'A'));
    assert_eq!(toks[1], Token::CharLit(b'\n'));
}

#[test]
fn lex_keywords() {
    let mut lex = Lexer::new("if else while for return I64 U0 Bool class");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::If);
    assert_eq!(toks[1], Token::Else);
    assert_eq!(toks[2], Token::While);
    assert_eq!(toks[3], Token::For);
    assert_eq!(toks[4], Token::Return);
    assert_eq!(toks[5], Token::I64);
    assert_eq!(toks[6], Token::U0);
    assert_eq!(toks[7], Token::Bool);
    assert_eq!(toks[8], Token::Class);
}

#[test]
fn lex_operators() {
    let src = "++ -- += -= == != <= >= << >> && || -> <<= >>=";
    let mut lex = Lexer::new(src);
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::PlusPlus);
    assert_eq!(toks[1], Token::MinusMinus);
    assert_eq!(toks[2], Token::PlusEq);
    assert_eq!(toks[3], Token::MinusEq);
    assert_eq!(toks[4], Token::EqEq);
    assert_eq!(toks[5], Token::BangEq);
    assert_eq!(toks[6], Token::LtEq);
    assert_eq!(toks[7], Token::GtEq);
    assert_eq!(toks[8], Token::Shl);
    assert_eq!(toks[9], Token::Shr);
    assert_eq!(toks[10], Token::AmpAmp);
    assert_eq!(toks[11], Token::PipePipe);
    assert_eq!(toks[12], Token::Arrow);
    assert_eq!(toks[13], Token::ShlEq);
    assert_eq!(toks[14], Token::ShrEq);
}

#[test]
fn lex_skips_line_comments() {
    let mut lex = Lexer::new("I64 // this is a comment\nU0");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::I64);
    assert_eq!(toks[1], Token::U0);
    assert_eq!(toks[2], Token::Eof);
}

#[test]
fn lex_skips_block_comments() {
    let mut lex = Lexer::new("I64 /* block\ncomment */ U0");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::I64);
    assert_eq!(toks[1], Token::U0);
}

#[test]
fn lex_preprocessor_directives() {
    let mut lex = Lexer::new("#define MAX 100\n#include \"stdlib.HC\"");
    let toks: Vec<Token> = lex
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|(t, _)| t)
        .collect();
    assert_eq!(toks[0], Token::Define);
    assert_eq!(toks[1], Token::Ident("MAX".into()));
    assert_eq!(toks[2], Token::IntLit(100));
    assert_eq!(toks[3], Token::Include);
}

// ── Parser tests ──────────────────────────────────────────────────────────────

fn parse(src: &str) -> holyc_frontend::ast::Module {
    let mut lex = Lexer::new(src);
    let tokens = lex.tokenize().expect("lex failed");
    let mut parser = Parser::new(tokens);
    parser.parse_module().expect("parse failed")
}

#[test]
fn parse_empty_function() {
    let m = parse("U0 Main() {}");
    assert_eq!(m.items.len(), 1);
    use holyc_frontend::ast::TopLevelKind;
    match &m.items[0].kind {
        TopLevelKind::FuncDef {
            name, params, body, ..
        } => {
            assert_eq!(name, "Main");
            assert!(params.is_empty());
            assert!(body.is_empty());
        },
        other => panic!("expected FuncDef, got {other:?}"),
    }
}

#[test]
fn parse_function_with_params() {
    let m = parse("I64 Add(I64 a, I64 b) { return a + b; }");
    use holyc_frontend::ast::{StmtKind, TopLevelKind};
    match &m.items[0].kind {
        TopLevelKind::FuncDef {
            name, params, body, ..
        } => {
            assert_eq!(name, "Add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "a");
            assert_eq!(params[1].name, "b");
            assert!(matches!(body[0].kind, StmtKind::Return(Some(_))));
        },
        other => panic!("expected FuncDef, got {other:?}"),
    }
}

#[test]
fn parse_var_decl_with_init() {
    let m = parse("U0 F() { I64 x = 42; }");
    use holyc_frontend::ast::{ExprKind, StmtKind, TopLevelKind};
    match &m.items[0].kind {
        TopLevelKind::FuncDef { body, .. } => match &body[0].kind {
            StmtKind::VarDecl {
                name,
                init: Some(init),
                ..
            } => {
                assert_eq!(name, "x");
                assert!(matches!(init.kind, ExprKind::IntLit(42)));
            },
            other => panic!("expected VarDecl, got {other:?}"),
        },
        other => panic!("expected FuncDef, got {other:?}"),
    }
}

#[test]
fn parse_if_else() {
    let src = "U0 F() { if (x > 0) { break; } else { continue; } }";
    let m = parse(src);
    use holyc_frontend::ast::{StmtKind, TopLevelKind};
    match &m.items[0].kind {
        TopLevelKind::FuncDef { body, .. } => {
            assert!(matches!(
                body[0].kind,
                StmtKind::If {
                    else_body: Some(_),
                    ..
                }
            ));
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn parse_for_loop() {
    let src = "U0 F() { for (I64 i = 0; i < 10; i++) {} }";
    let m = parse(src);
    use holyc_frontend::ast::{StmtKind, TopLevelKind};
    match &m.items[0].kind {
        TopLevelKind::FuncDef { body, .. } => {
            assert!(matches!(body[0].kind, StmtKind::For { .. }));
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn parse_class_def() {
    let src = "class Point { I64 x; I64 y; };";
    let m = parse(src);
    use holyc_frontend::ast::TopLevelKind;
    match &m.items[0].kind {
        TopLevelKind::ClassDef { name, fields } => {
            assert_eq!(name, "Point");
            assert_eq!(fields.len(), 2);
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn parse_pointer_type() {
    let src = "U0 F(I64* ptr) {}";
    let m = parse(src);
    use holyc_frontend::ast::TopLevelKind;
    use holyc_frontend::types::HolyType;
    match &m.items[0].kind {
        TopLevelKind::FuncDef { params, .. } => {
            assert!(matches!(&params[0].ty, HolyType::Ptr(inner) if **inner == HolyType::I64));
        },
        other => panic!("{other:?}"),
    }
}
