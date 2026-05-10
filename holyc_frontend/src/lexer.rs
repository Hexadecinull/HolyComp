//! Character-by-character lexer for HolyC source files.
//!
//! Produces a `Vec<Spanned<Token>>` where every [`Token`] carries
//! the `[start, end)` byte-offset span within the source string so
//! that diagnostics can point back to the original text.

use crate::{
    error::{LexError, Spanned},
    token::Token,
};

// ── Public interface ──────────────────────────────────────────────────────────

pub struct Lexer<'src> {
    src: &'src str,
    pos: usize, // current byte offset
    line: u32,
    col: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Lexer {
            src,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Tokenise the entire source, returning all tokens including a trailing
    /// [`Token::Eof`].
    pub fn tokenize(&mut self) -> Result<Vec<Spanned<Token>>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.0 == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    /// Lex the next single token from the stream.
    pub fn next_token(&mut self) -> Result<Spanned<Token>, LexError> {
        self.skip_whitespace_and_comments()?;

        let start = self.pos;

        match self.peek() {
            None => Ok((Token::Eof, (start, start))),

            Some('#') => {
                self.advance();
                // eat optional whitespace between '#' and directive name
                while self.peek() == Some(' ') || self.peek() == Some('\t') {
                    self.advance();
                }
                let directive = self.read_ident_raw();
                let tok = match directive.as_str() {
                    "define" => Token::Define,
                    "include" => Token::Include,
                    "ifdef" => Token::Ifdef,
                    "ifndef" => Token::Ifndef,
                    "endif" => Token::Endif,
                    "elif" => Token::Elif,
                    "else" => Token::Else2,
                    _ => Token::Hash,
                };
                Ok((tok, (start, self.pos)))
            },

            Some('"') => self.lex_string(start),
            Some('\'') => self.lex_char(start),

            Some(c) if c.is_ascii_digit() => self.lex_number(start),

            Some(c) if c.is_alphabetic() || c == '_' => {
                let name = self.read_ident_raw();
                let tok = keyword_or_ident(name);
                Ok((tok, (start, self.pos)))
            },

            Some(_) => self.lex_punctuation(start),
        }
    }

    // ── Skipping ─────────────────────────────────────────────────────────────

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            // whitespace
            while matches!(self.peek(), Some(' ' | '\t' | '\r' | '\n')) {
                self.advance();
            }
            // line comment
            if self.peek() == Some('/') && self.peek2() == Some('/') {
                while !matches!(self.peek(), None | Some('\n')) {
                    self.advance();
                }
                continue;
            }
            // block comment
            if self.peek() == Some('/') && self.peek2() == Some('*') {
                let comment_line = self.line;
                self.advance();
                self.advance(); // consume /*
                loop {
                    match self.peek() {
                        None => {
                            return Err(LexError::UnterminatedBlockComment { line: comment_line })
                        },
                        Some('*') if self.peek2() == Some('/') => {
                            self.advance();
                            self.advance();
                            break;
                        },
                        _ => {
                            self.advance();
                        },
                    }
                }
                continue;
            }
            break;
        }
        Ok(())
    }

    // ── Literals ─────────────────────────────────────────────────────────────

    fn lex_string(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        let line = self.line;
        self.advance(); // consume opening "
        let mut s = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return Err(LexError::UnterminatedString { line }),
                Some('"') => {
                    self.advance();
                    break;
                },
                Some('\\') => {
                    self.advance();
                    s.push(self.lex_escape()?);
                },
                Some(c) => {
                    s.push(c);
                    self.advance();
                },
            }
        }
        Ok((Token::StringLit(s), (start, self.pos)))
    }

    fn lex_char(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        let line = self.line;
        self.advance(); // consume opening '
        let ch = match self.peek() {
            None | Some('\n') => return Err(LexError::UnterminatedChar { line }),
            Some('\'') => return Err(LexError::EmptyChar { line }),
            Some('\\') => {
                self.advance();
                self.lex_escape()?
            },
            Some(c) => {
                self.advance();
                c
            },
        };
        match self.peek() {
            Some('\'') => {
                self.advance();
            },
            _ => return Err(LexError::UnterminatedChar { line }),
        }
        Ok((Token::CharLit(ch as u8), (start, self.pos)))
    }

    fn lex_escape(&mut self) -> Result<char, LexError> {
        let line = self.line;
        match self.peek() {
            Some('n') => {
                self.advance();
                Ok('\n')
            },
            Some('t') => {
                self.advance();
                Ok('\t')
            },
            Some('r') => {
                self.advance();
                Ok('\r')
            },
            Some('0') => {
                self.advance();
                Ok('\0')
            },
            Some('\\') => {
                self.advance();
                Ok('\\')
            },
            Some('"') => {
                self.advance();
                Ok('"')
            },
            Some('\'') => {
                self.advance();
                Ok('\'')
            },
            Some(c) => Err(LexError::InvalidEscape { ch: c, line }),
            None => Err(LexError::UnterminatedString { line }),
        }
    }

    fn lex_number(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        let line = self.line;

        // Hex: 0x / 0X
        if self.peek() == Some('0') && matches!(self.peek2(), Some('x') | Some('X')) {
            self.advance();
            self.advance(); // consume 0x
            let hex_start = self.pos;
            while self.peek().map(|c| c.is_ascii_hexdigit()).unwrap_or(false) {
                self.advance();
            }
            let digits = &self.src[hex_start..self.pos];
            let val =
                u64::from_str_radix(digits, 16).map_err(|_| LexError::IntegerOverflow { line })?;
            return Ok((Token::IntLit(val), (start, self.pos)));
        }

        // Binary: 0b / 0B
        if self.peek() == Some('0') && matches!(self.peek2(), Some('b') | Some('B')) {
            self.advance();
            self.advance(); // consume 0b
            let bin_start = self.pos;
            while matches!(self.peek(), Some('0') | Some('1')) {
                self.advance();
            }
            let digits = &self.src[bin_start..self.pos];
            let val =
                u64::from_str_radix(digits, 2).map_err(|_| LexError::IntegerOverflow { line })?;
            return Ok((Token::IntLit(val), (start, self.pos)));
        }

        // Consume leading digits
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance();
        }

        // Float: if we see '.' followed by a digit
        if self.peek() == Some('.') && self.peek2().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.advance(); // consume '.'
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.advance();
            }
            // Optional exponent
            if matches!(self.peek(), Some('e') | Some('E')) {
                self.advance();
                if matches!(self.peek(), Some('+') | Some('-')) {
                    self.advance();
                }
                while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    self.advance();
                }
            }
            let text = &self.src[start..self.pos];
            let val: f64 = text
                .parse()
                .map_err(|_| LexError::IntegerOverflow { line })?;
            return Ok((Token::FloatLit(val), (start, self.pos)));
        }

        // Plain integer
        let text = &self.src[start..self.pos];
        let val: u64 = text
            .parse()
            .map_err(|_| LexError::IntegerOverflow { line })?;
        Ok((Token::IntLit(val), (start, self.pos)))
    }

    // ── Punctuation / operators ───────────────────────────────────────────────

    fn lex_punctuation(&mut self, start: usize) -> Result<Spanned<Token>, LexError> {
        macro_rules! tok {
            ($t:expr) => {{
                self.advance();
                Ok(($t, (start, self.pos)))
            }};
        }
        macro_rules! tok2 {
            ($t:expr) => {{
                self.advance();
                self.advance();
                Ok(($t, (start, self.pos)))
            }};
        }
        macro_rules! tok2_or {
            ($two_ch:expr, $two_tok:expr, $one_tok:expr) => {
                if self.peek2() == Some($two_ch) {
                    tok2!($two_tok)
                } else {
                    tok!($one_tok)
                }
            };
        }
        macro_rules! tok2_eq_or {
            ($with_eq:expr, $without:expr) => {
                tok2_or!('=', $with_eq, $without)
            };
        }

        let c = self.peek().unwrap();
        match c {
            '(' => tok!(Token::LParen),
            ')' => tok!(Token::RParen),
            '{' => tok!(Token::LBrace),
            '}' => tok!(Token::RBrace),
            '[' => tok!(Token::LBracket),
            ']' => tok!(Token::RBracket),
            ';' => tok!(Token::Semi),
            ',' => tok!(Token::Comma),
            '?' => tok!(Token::Question),
            ':' => tok!(Token::Colon),
            '~' => tok!(Token::Tilde),
            '.' => tok!(Token::Dot),
            '^' => tok2_eq_or!(Token::CaretEq, Token::Caret),
            '%' => tok2_eq_or!(Token::PercentEq, Token::Percent),
            '!' => tok2_or!('=', Token::BangEq, Token::Bang),
            '=' => tok2_or!('=', Token::EqEq, Token::Eq),
            '+' => {
                if self.peek2() == Some('+') {
                    tok2!(Token::PlusPlus)
                } else if self.peek2() == Some('=') {
                    tok2!(Token::PlusEq)
                } else {
                    tok!(Token::Plus)
                }
            },
            '-' => {
                if self.peek2() == Some('-') {
                    tok2!(Token::MinusMinus)
                } else if self.peek2() == Some('=') {
                    tok2!(Token::MinusEq)
                } else if self.peek2() == Some('>') {
                    tok2!(Token::Arrow)
                } else {
                    tok!(Token::Minus)
                }
            },
            '*' => tok2_eq_or!(Token::StarEq, Token::Star),
            '/' => tok2_eq_or!(Token::SlashEq, Token::Slash),
            '&' => {
                if self.peek2() == Some('&') {
                    tok2!(Token::AmpAmp)
                } else if self.peek2() == Some('=') {
                    tok2!(Token::AmpEq)
                } else {
                    tok!(Token::Amp)
                }
            },
            '|' => {
                if self.peek2() == Some('|') {
                    tok2!(Token::PipePipe)
                } else if self.peek2() == Some('=') {
                    tok2!(Token::PipeEq)
                } else {
                    tok!(Token::Pipe)
                }
            },
            '<' => {
                if self.peek2() == Some('<') {
                    self.advance();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Ok((Token::ShlEq, (start, self.pos)))
                    } else {
                        Ok((Token::Shl, (start, self.pos)))
                    }
                } else if self.peek2() == Some('=') {
                    tok2!(Token::LtEq)
                } else {
                    tok!(Token::Lt)
                }
            },
            '>' => {
                if self.peek2() == Some('>') {
                    self.advance();
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Ok((Token::ShrEq, (start, self.pos)))
                    } else {
                        Ok((Token::Shr, (start, self.pos)))
                    }
                } else if self.peek2() == Some('=') {
                    tok2!(Token::GtEq)
                } else {
                    tok!(Token::Gt)
                }
            },
            _ => {
                let line = self.line;
                let col = self.col;
                self.advance();
                Err(LexError::UnexpectedChar { ch: c, line, col })
            },
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    /// Read an identifier/keyword name into a `String` without wrapping in Token.
    fn read_ident_raw(&mut self) -> String {
        let start = self.pos;
        while self
            .peek()
            .map(|c| c.is_alphanumeric() || c == '_')
            .unwrap_or(false)
        {
            self.advance();
        }
        self.src[start..self.pos].to_owned()
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut chars = self.src[self.pos..].chars();
        chars.next();
        chars.next()
    }

    fn advance(&mut self) {
        if let Some(c) = self.src[self.pos..].chars().next() {
            self.pos += c.len_utf8();
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
    }
}

// ── Keyword lookup table ──────────────────────────────────────────────────────

fn keyword_or_ident(name: String) -> Token {
    match name.as_str() {
        // types
        "U0" => Token::U0,
        "I8" => Token::I8,
        "U8" => Token::U8,
        "I16" => Token::I16,
        "U16" => Token::U16,
        "I32" => Token::I32,
        "U32" => Token::U32,
        "I64" => Token::I64,
        "U64" => Token::U64,
        "F32" => Token::F32,
        "F64" => Token::F64,
        "Bool" => Token::Bool,
        // control
        "if" => Token::If,
        "else" => Token::Else,
        "while" => Token::While,
        "for" => Token::For,
        "do" => Token::Do,
        "switch" => Token::Switch,
        "case" => Token::Case,
        "default" => Token::Default,
        "break" => Token::Break,
        "continue" => Token::Continue,
        "return" => Token::Return,
        // storage
        "extern" => Token::Extern,
        "static" => Token::Static,
        "public" => Token::Public,
        "private" => Token::Private,
        "local" => Token::Local,
        "reg" => Token::Reg,
        // aggregate
        "class" => Token::Class,
        "union" => Token::Union,
        "typedef" => Token::Typedef,
        // misc
        "sizeof" => Token::Sizeof,
        "asm" => Token::Asm,
        "TRUE" | "true" => Token::True,
        "FALSE" | "false" => Token::False,
        "NULL" | "null" => Token::Null,
        _ => Token::Ident(name),
    }
}
