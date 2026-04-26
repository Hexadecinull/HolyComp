//! Structured diagnostic rendering for HolyC.
//!
//! Converts a parse or lex error together with the original source text into a
//! human-readable, rustc-style diagnostic with a caret underline:
//!
//! ```text
//! error[parse]: unexpected token `}`, expected `;` after expression at line 12
//!   --> src/foo.HC:12:5
//!    |
//! 12 |   return }
//!    |          ^ unexpected token
//! ```

use crate::error::{LexError, ParseError, Span};

// ── Diagnostic severity ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Note => write!(f, "note"),
        }
    }
}

// ── Diagnostic ────────────────────────────────────────────────────────────────

/// A fully rendered diagnostic ready to print to stderr.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Short category tag, e.g. `"lex"` or `"parse"`.
    pub category: &'static str,
    pub message: String,
    /// Optional file name shown in the `-->` line.
    pub file: Option<String>,
    /// Optional source snippet with caret underline.
    pub snippet: Option<Snippet>,
}

#[derive(Debug, Clone)]
pub struct Snippet {
    pub line_no: u32,
    pub col_no: u32,
    pub line_text: String,
    pub label: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Header: `error[parse]: message`
        write!(f, "{}[{}]: {}", self.severity, self.category, self.message)?;

        let Some(ref snip) = self.snippet else {
            return Ok(());
        };

        // Location line
        let file = self.file.as_deref().unwrap_or("<unknown>");
        write!(f, "\n  --> {file}:{}:{}", snip.line_no, snip.col_no)?;

        // Snippet + caret
        let line_prefix = format!("{:>4} | ", snip.line_no);
        let blank_prefix = " ".repeat(line_prefix.len() - 2);
        write!(
            f,
            "\n{blank_prefix}|\n{line_prefix}{}",
            snip.line_text.trim_end()
        )?;
        let caret_indent = " ".repeat(snip.col_no.saturating_sub(1) as usize);
        write!(f, "\n{blank_prefix}| {caret_indent}^ {}", snip.label)?;

        Ok(())
    }
}

// ── Constructors ──────────────────────────────────────────────────────────────

/// Render a [`LexError`] as a `Diagnostic` given the original source text.
pub fn from_lex_error(err: &LexError, src: &str, file: Option<&str>) -> Diagnostic {
    let (line_no, col_no) = match err {
        LexError::UnexpectedChar { line, col, .. } => (*line, *col),
        LexError::UnterminatedString { line }
        | LexError::UnterminatedChar { line }
        | LexError::EmptyChar { line }
        | LexError::UnterminatedBlockComment { line }
        | LexError::IntegerOverflow { line } => (*line, 1),
        LexError::InvalidEscape { line, .. } => (*line, 1),
    };
    let snippet = make_snippet(src, line_no, col_no, "here");
    Diagnostic {
        severity: Severity::Error,
        category: "lex",
        message: err.to_string(),
        file: file.map(str::to_owned),
        snippet,
    }
}

/// Render a [`ParseError`] as a `Diagnostic` given the original source text and
/// optional byte-level span (from the token that caused the error).
pub fn from_parse_error(
    err: &ParseError,
    src: &str,
    span: Option<Span>,
    file: Option<&str>,
) -> Diagnostic {
    let (line_no, col_no) = match err {
        ParseError::UnexpectedToken { line, .. } => (*line, 1u32),
        ParseError::UnexpectedEof { .. } => (src.lines().count() as u32, 1),
        ParseError::Lex(lex_err) => return from_lex_error(lex_err, src, file),
    };
    // If we have a byte span, compute a more precise column from it.
    let col_no = span.map_or(col_no, |(start, _)| byte_to_col(src, start));
    let snippet = make_snippet(src, line_no, col_no, "here");
    Diagnostic {
        severity: Severity::Error,
        category: "parse",
        message: err.to_string(),
        file: file.map(str::to_owned),
        snippet,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_snippet(src: &str, line_no: u32, col_no: u32, label: &str) -> Option<Snippet> {
    let line_text = src.lines().nth(line_no.saturating_sub(1) as usize)?;
    Some(Snippet {
        line_no,
        col_no,
        line_text: line_text.to_owned(),
        label: label.to_owned(),
    })
}

fn byte_to_col(src: &str, byte_offset: usize) -> u32 {
    let line_start = src[..byte_offset].rfind('\n').map(|p| p + 1).unwrap_or(0);
    (byte_offset - line_start + 1) as u32
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LexError;

    #[test]
    fn lex_diagnostic_renders() {
        let src = "I64 x = ;\n";
        let err = LexError::UnexpectedChar {
            ch: ';',
            line: 1,
            col: 9,
        };
        let diag = from_lex_error(&err, src, Some("test.HC"));
        let s = diag.to_string();
        assert!(s.contains("error[lex]"), "got: {s}");
        assert!(s.contains("test.HC:1:9"), "got: {s}");
        assert!(s.contains("I64 x = ;"), "got: {s}");
    }

    #[test]
    fn parse_diagnostic_renders() {
        let src = "U0 F() { return }\n";
        let err = ParseError::UnexpectedToken {
            found: "`}`".into(),
            expected: "expression".into(),
            line: 1,
        };
        let diag = from_parse_error(&err, src, None, Some("test.HC"));
        let s = diag.to_string();
        assert!(s.contains("error[parse]"), "got: {s}");
        assert!(s.contains("unexpected token"), "got: {s}");
    }

    #[test]
    fn col_computation() {
        let src = "hello\nworld";
        assert_eq!(byte_to_col(src, 0), 1); // 'h'
        assert_eq!(byte_to_col(src, 6), 1); // 'w' (first col of line 2)
        assert_eq!(byte_to_col(src, 8), 3); // 'r' (col 3 of line 2)
    }
}
