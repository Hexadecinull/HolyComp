//! Builtin functions exposed to HolyC programs.
//!
//! Each builtin has the signature `fn(&[Value]) -> Result<Value, RuntimeError>`
//! where `Value` is re-exported below as a thin wrapper so the stdlib crate
//! doesn't depend on the interpreter crate.
//!
//! The interpreter registers these in `vm::Interpreter::register_builtins`.

use thiserror::Error;

// ── Shared value type (mirrors interpreter Value but owned by stdlib) ─────────

/// A runtime value passed to or returned from a builtin.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    UInt(u64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(u8),
    Ptr(usize),
    Void,
}

impl Value {
    /// Truthy test: mirrors C semantics (0 / null / false == false).
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b)  => *b,
            Value::Int(n)   => *n != 0,
            Value::UInt(n)  => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Ptr(p)   => *p != 0,
            Value::Char(c)  => *c != 0,
            Value::Str(_)   => true,
            Value::Void     => false,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Int(_)   => "I64",
            Value::UInt(_)  => "U64",
            Value::Float(_) => "F64",
            Value::Bool(_)  => "Bool",
            Value::Str(_)   => "U8*",
            Value::Char(_)  => "U8",
            Value::Ptr(_)   => "pointer",
            Value::Void     => "U0",
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n)   => Some(*n),
            Value::UInt(n)  => Some(*n as i64),
            Value::Bool(b)  => Some(*b as i64),
            Value::Char(c)  => Some(*c as i64),
            Value::Ptr(p)   => Some(*p as i64),
            _               => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n)   => Some(*n as f64),
            Value::UInt(n)  => Some(*n as f64),
            _               => None,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n)   => write!(f, "{n}"),
            Value::UInt(n)  => write!(f, "{n}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Bool(b)  => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Value::Str(s)   => write!(f, "{s}"),
            Value::Char(c)  => write!(f, "{}", *c as char),
            Value::Ptr(p)   => write!(f, "0x{p:x}"),
            Value::Void     => write!(f, ""),
        }
    }
}

// ── PrintArg: Value variant used by the formatter ────────────────────────────

/// Alias so `format.rs` can refer to typed args.
pub type PrintArg = Value;

// ── Runtime error ─────────────────────────────────────────────────────────────

#[derive(Error, Debug, Clone)]
pub enum RuntimeError {
    #[error("type error: expected {expected}, found {found}")]
    TypeError { expected: String, found: String },

    #[error("wrong number of arguments: expected {expected}, found {got}")]
    ArgCount { expected: usize, got: usize },

    #[error("{0}")]
    Custom(String),
}

fn type_err(expected: &str, found: &str) -> RuntimeError {
    RuntimeError::TypeError { expected: expected.into(), found: found.into() }
}

fn argc(expected: usize, got: usize) -> RuntimeError {
    RuntimeError::ArgCount { expected, got }
}

// ── I/O ───────────────────────────────────────────────────────────────────────

/// `Print(fmt, ...)` / `printf(fmt, ...)` — write to current output sink.
///
/// The first argument must be a string (format string).
/// Subsequent arguments are substituted into printf-style specifiers.
/// Output is routed through [`crate::capture::output`] so tests can intercept it.
///
/// Note: `Value::Ptr` format strings are resolved to `Value::Str` by the
/// interpreter's call-site before reaching here (see `vm::Interpreter::call_func`
/// heap-string resolution).  This function handles `Str` and falls back to
/// `Display` for other types.
pub fn print(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.is_empty() {
        return Err(argc(1, 0));
    }
    let output = match &args[0] {
        Value::Str(fmt) => crate::format::format_holyc(fmt, &args[1..]),
        other           => other.to_string(),
    };
    crate::capture::output(&output);
    Ok(Value::Void)
}

/// `Exit(code)` — terminate the process.
pub fn exit(args: &[Value]) -> Result<Value, RuntimeError> {
    let code = args.first().and_then(|v| v.as_int()).unwrap_or(0);
    std::process::exit(code as i32);
}

// ── Math ──────────────────────────────────────────────────────────────────────

/// `Abs(x)` — absolute value (integer or float).
pub fn abs(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first().ok_or_else(|| argc(1, 0))? {
        Value::Int(n)   => Ok(Value::Int(n.wrapping_abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        other           => Err(type_err("number", other.type_name())),
    }
}

/// `Sin(x)` — sine (radians, returns F64).
pub fn sin(args: &[Value]) -> Result<Value, RuntimeError> {
    let f = require_float(args, 0, "Sin")?;
    Ok(Value::Float(f.sin()))
}

/// `Cos(x)` — cosine (radians, returns F64).
pub fn cos(args: &[Value]) -> Result<Value, RuntimeError> {
    let f = require_float(args, 0, "Cos")?;
    Ok(Value::Float(f.cos()))
}

/// `Sqrt(x)` — square root.
pub fn sqrt(args: &[Value]) -> Result<Value, RuntimeError> {
    let f = require_float(args, 0, "Sqrt")?;
    Ok(Value::Float(f.sqrt()))
}

/// `Pow(base, exp)` — power.
pub fn pow(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(argc(2, args.len())); }
    let base = require_float(args, 0, "Pow")?;
    let exp  = require_float(args, 1, "Pow")?;
    Ok(Value::Float(base.powf(exp)))
}

// ── String ────────────────────────────────────────────────────────────────────

/// `StrLen(s)` — length of string in bytes.
pub fn strlen(args: &[Value]) -> Result<Value, RuntimeError> {
    match args.first().ok_or_else(|| argc(1, 0))? {
        Value::Str(s) => Ok(Value::Int(s.len() as i64)),
        // Ptr case is resolved by the interpreter's heap before reaching here.
        other         => Err(type_err("string", other.type_name())),
    }
}

/// `StrCmp(a, b)` — compare two strings (C strcmp semantics: <0, 0, >0).
pub fn strcmp(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(argc(2, args.len())); }
    let a = require_str(&args[0], "StrCmp")?;
    let b = require_str(&args[1], "StrCmp")?;
    Ok(Value::Int(a.cmp(b) as i64))
}

/// `StrCpy(dst, src)` — copy `src` into `dst`; returns `dst` (as a new String).
/// In the interpreter this returns the copied string (no raw pointer mutation).
pub fn strcpy(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(argc(2, args.len())); }
    let src = require_str(&args[1], "StrCpy")?.to_owned();
    Ok(Value::Str(src))
}

/// `StrCat(a, b)` — concatenate two strings; returns the result.
pub fn strcat(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(argc(2, args.len())); }
    let a = require_str(&args[0], "StrCat")?;
    let b = require_str(&args[1], "StrCat")?;
    Ok(Value::Str(format!("{a}{b}")))
}

/// `StrStr(haystack, needle)` — find first occurrence; returns index or -1.
pub fn strstr(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(argc(2, args.len())); }
    let hay    = require_str(&args[0], "StrStr")?;
    let needle = require_str(&args[1], "StrStr")?;
    match hay.find(needle) {
        Some(idx) => Ok(Value::Int(idx as i64)),
        None      => Ok(Value::Int(-1)),
    }
}

/// `StrToI64(s)` — parse a decimal string to I64; 0 on failure.
pub fn str_to_i64(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = require_str(args.first().ok_or_else(|| argc(1, 0))?, "StrToI64")?;
    Ok(Value::Int(s.trim().parse::<i64>().unwrap_or(0)))
}

// ── Random numbers ────────────────────────────────────────────────────────────

use std::cell::Cell;
thread_local! {
    static RNG_STATE: Cell<u64> = const { Cell::new(12345) };
}

/// LCG pseudo-random number generator (same constants as glibc rand).
fn lcg_next() -> u64 {
    RNG_STATE.with(|s| {
        let next = s.get().wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s.set(next);
        next
    })
}

/// `SRand(seed)` — seed the random number generator.
pub fn srand(args: &[Value]) -> Result<Value, RuntimeError> {
    let seed = args.first().and_then(|v| v.as_int()).unwrap_or(1) as u64;
    RNG_STATE.with(|s| s.set(seed));
    Ok(Value::Void)
}

/// `Rand()` — return a pseudo-random non-negative I64.
pub fn rand(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int((lcg_next() >> 1) as i64))
}

/// `RandI64(lo, hi)` — random integer in `[lo, hi]` inclusive.
pub fn rand_range(args: &[Value]) -> Result<Value, RuntimeError> {
    if args.len() < 2 { return Err(argc(2, args.len())); }
    let lo = args[0].as_int().unwrap_or(0);
    let hi = args[1].as_int().unwrap_or(0);
    if lo >= hi { return Ok(Value::Int(lo)); }
    let range = (hi - lo + 1) as u64;
    let r = (lcg_next() % range) as i64 + lo;
    Ok(Value::Int(r))
}

// ── Time ──────────────────────────────────────────────────────────────────────

/// `Time()` — seconds since Unix epoch as I64.
pub fn time_now(_args: &[Value]) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(Value::Int(secs))
}

// ── Memory (real impls in vm.rs via heap; these are fallback stubs) ───────────

/// `MemSet(ptr, val, len)` — stub (interpreter handles via heap).
pub fn memset_stub(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `MAlloc(size)` — stub returning null (interpreter overrides via heap).
pub fn malloc_stub(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Ptr(0))
}

/// `Free(ptr)` — stub (interpreter overrides via heap).
pub fn free_stub(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn require_float(args: &[Value], idx: usize, name: &str) -> Result<f64, RuntimeError> {
    args.get(idx)
        .and_then(|v| v.as_float())
        .ok_or_else(|| RuntimeError::Custom(
            format!("{name}: argument {idx} must be a number")
        ))
}

fn require_str<'a>(val: &'a Value, name: &str) -> Result<&'a str, RuntimeError> {
    match val {
        Value::Str(s) => Ok(s.as_str()),
        other => Err(type_err("string", other.type_name())).map_err(|e| RuntimeError::Custom(
            format!("{name}: {e}")
        )),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abs_int() {
        let v = abs(&[Value::Int(-42)]).unwrap();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn test_abs_float() {
        let v = abs(&[Value::Float(-3.14)]).unwrap();
        assert_eq!(v, Value::Float(3.14));
    }

    #[test]
    fn test_strcmp_equal() {
        let v = strcmp(&[Value::Str("abc".into()), Value::Str("abc".into())]).unwrap();
        assert_eq!(v, Value::Int(0));
    }

    #[test]
    fn test_strcmp_less() {
        let v = strcmp(&[Value::Str("abc".into()), Value::Str("abd".into())]).unwrap();
        assert!(matches!(v, Value::Int(n) if n < 0));
    }

    #[test]
    fn test_strcmp_greater() {
        let v = strcmp(&[Value::Str("b".into()), Value::Str("a".into())]).unwrap();
        assert!(matches!(v, Value::Int(n) if n > 0));
    }

    #[test]
    fn test_strcat() {
        let v = strcat(&[Value::Str("foo".into()), Value::Str("bar".into())]).unwrap();
        assert_eq!(v, Value::Str("foobar".into()));
    }

    #[test]
    fn test_strstr_found() {
        let v = strstr(&[Value::Str("hello world".into()), Value::Str("world".into())]).unwrap();
        assert_eq!(v, Value::Int(6));
    }

    #[test]
    fn test_strstr_not_found() {
        let v = strstr(&[Value::Str("hello".into()), Value::Str("xyz".into())]).unwrap();
        assert_eq!(v, Value::Int(-1));
    }

    #[test]
    fn test_str_to_i64() {
        let v = str_to_i64(&[Value::Str("  -42 ".into())]).unwrap();
        assert_eq!(v, Value::Int(-42));
    }

    #[test]
    fn test_str_to_i64_invalid() {
        let v = str_to_i64(&[Value::Str("abc".into())]).unwrap();
        assert_eq!(v, Value::Int(0));
    }

    #[test]
    fn test_rand_deterministic() {
        srand(&[Value::Int(42)]).unwrap();
        let a = rand(&[]).unwrap();
        srand(&[Value::Int(42)]).unwrap();
        let b = rand(&[]).unwrap();
        assert_eq!(a, b, "same seed must produce same sequence");
    }

    #[test]
    fn test_rand_range_in_bounds() {
        srand(&[Value::Int(1)]).unwrap();
        for _ in 0..100 {
            let v = rand_range(&[Value::Int(1), Value::Int(6)]).unwrap();
            assert!(matches!(v, Value::Int(n) if (1..=6).contains(&n)));
        }
    }

    #[test]
    fn test_ptr_as_int() {
        // Value::Ptr must be usable as integer (for comparisons like ptr == NULL)
        assert_eq!(Value::Ptr(0).as_int(), Some(0));
        assert_eq!(Value::Ptr(42).as_int(), Some(42));
    }

    #[test]
    fn test_sin_zero() {
        let v = sin(&[Value::Float(0.0)]).unwrap();
        assert_eq!(v, Value::Float(0.0));
    }

    #[test]
    fn test_sqrt() {
        let v = sqrt(&[Value::Float(9.0)]).unwrap();
        assert_eq!(v, Value::Float(3.0));
    }

    #[test]
    fn test_pow() {
        let v = pow(&[Value::Float(2.0), Value::Float(10.0)]).unwrap();
        assert_eq!(v, Value::Float(1024.0));
    }

    #[test]
    fn test_strlen() {
        let v = strlen(&[Value::Str("hello".into())]).unwrap();
        assert_eq!(v, Value::Int(5));
    }

    #[test]
    fn test_argc_error() {
        let err = abs(&[]).unwrap_err();
        assert!(matches!(err, RuntimeError::ArgCount { expected: 1, got: 0 }));
    }
}
