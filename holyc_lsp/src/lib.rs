//! Re-export internals for testing.

pub mod diagnostics {
    use holyc_frontend::{Lexer, Parser};
    use serde_json::Value;

    pub fn from_source(src: &str) -> Vec<Value> {
        crate_diagnostics(src)
    }

    fn crate_diagnostics(src: &str) -> Vec<Value> {
        use serde_json::json;
        let mut diags = Vec::new();

        match Lexer::new(src).tokenize() {
            Err(e) => {
                let line = lex_line(&e).saturating_sub(1) as u64;
                diags.push(json!({ "severity": 1, "message": e.to_string(),
                    "range": { "start": { "line": line, "character": 0 },
                               "end":   { "line": line, "character": 0 } } }));
            },
            Ok(tokens) => {
                if let Err(e) = Parser::new(tokens).parse_module() {
                    let line = parse_line(&e).saturating_sub(1) as u64;
                    diags.push(json!({ "severity": 1, "message": e.to_string(),
                        "range": { "start": { "line": line, "character": 0 },
                                   "end":   { "line": line, "character": 0 } } }));
                }
            },
        }
        diags
    }

    fn lex_line(e: &holyc_frontend::LexError) -> u32 {
        use holyc_frontend::LexError::*;
        match e {
            UnexpectedChar { line, .. }
            | UnterminatedString { line }
            | UnterminatedChar { line }
            | EmptyChar { line }
            | UnterminatedBlockComment { line }
            | IntegerOverflow { line }
            | InvalidEscape { line, .. } => *line,
        }
    }

    fn parse_line(e: &holyc_frontend::ParseError) -> u32 {
        use holyc_frontend::ParseError::*;
        match e {
            UnexpectedToken { line, .. } => *line,
            UnexpectedEof { .. } => 0,
            Lex(le) => lex_line(le),
        }
    }
}
