/// Every token that the HolyC lexer can produce.
///
/// Variants that carry data are in *lower-case* (Rust convention);
/// the remaining keywords / punctuation are *PascalCase* matching
/// how they appear in source for readability.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Primitive type keywords ───────────────────────────────────────────────
    /// `U0` – void / unit
    U0,
    I8, U8, I16, U16, I32, U32, I64, U64,
    F32, F64,
    Bool,

    // ── Control-flow keywords ─────────────────────────────────────────────────
    If, Else,
    While, For, Do,
    Switch, Case, Default,
    Break, Continue,
    Return,

    // ── Storage-class / visibility keywords ──────────────────────────────────
    Extern, Static, Public, Private, Local, Reg,

    // ── Aggregate / type keywords ─────────────────────────────────────────────
    Class, Union, Typedef,

    // ── Misc keywords ─────────────────────────────────────────────────────────
    Sizeof, Asm,
    True, False,
    Null,

    // ── Preprocessor directives (kept as distinct tokens) ────────────────────
    Define, Include,
    Ifdef, Ifndef, Endif, Elif, Else2, // #else handled separately from keyword else

    // ── Literals ─────────────────────────────────────────────────────────────
    IntLit(u64),
    FloatLit(f64),
    StringLit(String),
    CharLit(u8),

    // ── Identifier ───────────────────────────────────────────────────────────
    Ident(String),

    // ── Compound assignment ───────────────────────────────────────────────────
    PlusEq, MinusEq, StarEq, SlashEq, PercentEq,
    AmpEq, PipeEq, CaretEq,
    ShlEq, ShrEq,

    // ── Comparison ───────────────────────────────────────────────────────────
    EqEq, BangEq,
    LtEq, GtEq,
    AmpAmp, PipePipe,

    // ── Shift ────────────────────────────────────────────────────────────────
    Shl, Shr,

    // ── Increment / decrement ─────────────────────────────────────────────────
    PlusPlus, MinusMinus,

    // ── Pointer access ────────────────────────────────────────────────────────
    Arrow, // ->

    // ── Single-char operators ─────────────────────────────────────────────────
    Plus, Minus, Star, Slash, Percent,
    Amp, Pipe, Caret, Tilde, Bang,
    Lt, Gt, Eq,
    Dot, Question, Colon,

    // ── Punctuation ───────────────────────────────────────────────────────────
    LParen, RParen,
    LBrace, RBrace,
    LBracket, RBracket,
    Semi, Comma, Hash,

    // ── End-of-file ───────────────────────────────────────────────────────────
    Eof,
}

impl Token {
    /// Human-readable description used in error messages.
    pub fn describe(&self) -> String {
        match self {
            Token::IntLit(n)    => format!("integer literal `{n}`"),
            Token::FloatLit(f)  => format!("float literal `{f}`"),
            Token::StringLit(s) => format!("string literal `\"{s}\"`"),
            Token::CharLit(c)   => format!("char literal `'{c}'`"),
            Token::Ident(name)  => format!("identifier `{name}`"),
            Token::Eof          => "end of file".into(),
            other               => format!("`{other:?}`"),
        }
    }

    /// Returns `true` if this token is a primitive type keyword.
    pub fn is_type_keyword(&self) -> bool {
        matches!(
            self,
            Token::U0
                | Token::I8  | Token::U8
                | Token::I16 | Token::U16
                | Token::I32 | Token::U32
                | Token::I64 | Token::U64
                | Token::F32 | Token::F64
                | Token::Bool
        )
    }
}
