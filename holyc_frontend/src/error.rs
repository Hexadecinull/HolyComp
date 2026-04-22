use thiserror::Error;

/// Byte-offset span in the source string: `[start, end)`.
pub type Span = (usize, usize);

/// A value together with its source span.
pub type Spanned<T> = (T, Span);

// ── Lex errors ────────────────────────────────────────────────────────────────

#[derive(Error, Debug, Clone, PartialEq)]
pub enum LexError {
    #[error("unexpected character '{ch}' at line {line}, col {col}")]
    UnexpectedChar { ch: char, line: u32, col: u32 },

    #[error("unterminated string literal starting at line {line}")]
    UnterminatedString { line: u32 },

    #[error("unterminated character literal at line {line}")]
    UnterminatedChar { line: u32 },

    #[error("empty character literal at line {line}")]
    EmptyChar { line: u32 },

    #[error("invalid escape sequence '\\{ch}' at line {line}")]
    InvalidEscape { ch: char, line: u32 },

    #[error("integer literal overflow at line {line}")]
    IntegerOverflow { line: u32 },

    #[error("unterminated block comment starting at line {line}")]
    UnterminatedBlockComment { line: u32 },
}

// ── Parse errors ──────────────────────────────────────────────────────────────

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParseError {
    #[error("unexpected token `{found}`, expected {expected} at line {line}")]
    UnexpectedToken {
        found: String,
        expected: String,
        line: u32,
    },

    #[error("unexpected end of file; expected {expected}")]
    UnexpectedEof { expected: String },

    #[error("lex error: {0}")]
    Lex(#[from] LexError),
}
