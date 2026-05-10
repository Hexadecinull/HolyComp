//! TempleOS compatibility shims.
//!
//! TempleOS programs use a large set of system and graphics functions that
//! don't map directly to portable host OS calls.  This module provides
//! best-effort shims so that basic TempleOS `.HC` files run unmodified
//! through the HolyComp interpreter.
//!
//! ## Calling-convention note
//!
//! TempleOS runs in ring-0 and uses a custom calling convention where all
//! arguments and return values are 64-bit (`I64`).  The host interpreter
//! already models this: every `Value::Int` is `i64`, matching the TempleOS
//! register-width assumption.  No special ABI translation is required for
//! the interpreter path; the LLVM backend handles it via the ABI classifier
//! in `holyc_compiler/src/abi.rs`.
//!
//! ## Graphics model
//!
//! TempleOS has a single 640 × 480 framebuffer owned by the OS.  On a host
//! system we have no direct framebuffer access, so graphics calls are either:
//!
//! - **No-ops** (`GrLine`, `GrRect`, `GrFillRect`, `GrCircle`) — silently
//!   dropped; programs that only draw will still run.
//! - **Text-redirected** (`GrPrint`, `DocPrint`, `DocPutS`) — output is sent
//!   to the capture buffer / stdout so text content is preserved.
//! - **Return a sentinel** (`GrWidth`, `GrHeight`) — returns the canonical
//!   TempleOS screen dimensions (640 × 480).

use crate::builtins::{RuntimeError, Value};

// ── Text output ───────────────────────────────────────────────────────────────

/// `GrPrint(x, y, fmt, …)` — draw text at pixel position.
///
/// The x/y coordinates are ignored; text is sent to stdout so programs that
/// use `GrPrint` for all output still work correctly.
pub fn gr_print(args: &[Value]) -> Result<Value, RuntimeError> {
    // args[0] = x, args[1] = y, args[2] = fmt, args[3..] = varargs
    if args.len() < 3 {
        return Ok(Value::Void);
    }
    let fmt_val = &args[2];
    let rest = &args[3..];
    let text = match fmt_val {
        Value::Str(s) => crate::format::format_holyc(s, rest),
        other => other.to_string(),
    };
    crate::capture::output(&text);
    Ok(Value::Void)
}

/// `DocPrint(fmt, …)` — print to the current document.
///
/// Equivalent to `Print` in the host environment.
pub fn doc_print(args: &[Value]) -> Result<Value, RuntimeError> {
    crate::builtins::print(args)
}

/// `DocPutS(str)` — put a raw string to the current document.
pub fn doc_puts(args: &[Value]) -> Result<Value, RuntimeError> {
    let s = match args.first() {
        Some(Value::Str(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => return Ok(Value::Void),
    };
    crate::capture::output(&s);
    Ok(Value::Void)
}

// ── Screen dimensions ─────────────────────────────────────────────────────────

/// `GrWidth()` → 640 (canonical TempleOS screen width in pixels).
pub fn gr_width(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(640))
}

/// `GrHeight()` → 480 (canonical TempleOS screen height in pixels).
pub fn gr_height(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(480))
}

// ── Graphics no-ops ───────────────────────────────────────────────────────────

/// `GrLine(x1, y1, x2, y2, colour)` — no-op on host.
pub fn gr_line(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `GrRect(x, y, w, h, colour)` — no-op on host.
pub fn gr_rect(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `GrFillRect(x, y, w, h, colour)` — no-op on host.
pub fn gr_fill_rect(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `GrCircle(x, y, r, colour)` — no-op on host.
pub fn gr_circle(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `GrFlush()` — no-op on host (double-buffer swap).
pub fn gr_flush(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `GrCls()` — no-op on host (clear screen / framebuffer).
pub fn gr_cls(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

// ── System / timing ───────────────────────────────────────────────────────────

/// `Sleep(ms)` — sleep for the given number of milliseconds.
pub fn sleep(args: &[Value]) -> Result<Value, RuntimeError> {
    let ms = args.first().and_then(|v| v.as_int()).unwrap_or(0).max(0) as u64;
    std::thread::sleep(std::time::Duration::from_millis(ms));
    Ok(Value::Void)
}

/// `SleepUs(us)` — sleep for the given number of microseconds.
pub fn sleep_us(args: &[Value]) -> Result<Value, RuntimeError> {
    let us = args.first().and_then(|v| v.as_int()).unwrap_or(0).max(0) as u64;
    std::thread::sleep(std::time::Duration::from_micros(us));
    Ok(Value::Void)
}

/// `ChkPtr(ptr)` — verify a pointer is non-null and within heap bounds.
///
/// In TempleOS this does a hard fault on invalid access; here we return
/// 1 if the pointer looks valid (non-zero), 0 otherwise.
pub fn chk_ptr(args: &[Value]) -> Result<Value, RuntimeError> {
    let valid = match args.first() {
        Some(Value::Ptr(0)) | None => 0i64,
        Some(Value::Ptr(_)) | Some(Value::Int(_)) => 1,
        _ => 0,
    };
    Ok(Value::Int(valid))
}

/// `GetTicks()` — monotonic millisecond counter (host-relative).
pub fn get_ticks(_args: &[Value]) -> Result<Value, RuntimeError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    Ok(Value::Int(ms))
}

/// `IsPtrValid(ptr)` — 1 if non-null, 0 otherwise.
pub fn is_ptr_valid(args: &[Value]) -> Result<Value, RuntimeError> {
    chk_ptr(args)
}

// ── Keyboard / input stubs ────────────────────────────────────────────────────

/// `GetKey()` → 0 (no key pressed in batch/interpreter mode).
pub fn get_key(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(0))
}

/// `ScanKey()` → 0 (no key in queue).
pub fn scan_key(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(0))
}

// ── Audio stubs ───────────────────────────────────────────────────────────────

/// `Beep(freq, ms)` — no-op on host (plays a PC speaker beep in TempleOS).
pub fn beep(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

/// `MusicNote(note, octave, duration)` — no-op on host.
pub fn music_note(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

// ── Memory / process ─────────────────────────────────────────────────────────

/// `MemUsed()` — return the interpreter heap's used-byte count.
///
/// In TempleOS this returns the system heap usage; here we return 0 as a
/// safe placeholder (the interpreter heap is not accessible from stdlib).
pub fn mem_used(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Int(0))
}

/// `ProgPatchU8(addr, val)` — self-modifying code; no-op on host.
pub fn prog_patch_u8(_args: &[Value]) -> Result<Value, RuntimeError> {
    Ok(Value::Void)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gr_dimensions_are_canonical() {
        assert_eq!(gr_width(&[]).unwrap(), Value::Int(640));
        assert_eq!(gr_height(&[]).unwrap(), Value::Int(480));
    }

    #[test]
    fn gr_nops_return_void() {
        assert_eq!(gr_line(&[]).unwrap(), Value::Void);
        assert_eq!(gr_rect(&[]).unwrap(), Value::Void);
        assert_eq!(gr_fill_rect(&[]).unwrap(), Value::Void);
        assert_eq!(gr_circle(&[]).unwrap(), Value::Void);
        assert_eq!(gr_flush(&[]).unwrap(), Value::Void);
        assert_eq!(gr_cls(&[]).unwrap(), Value::Void);
    }

    #[test]
    fn chk_ptr_null_is_invalid() {
        assert_eq!(chk_ptr(&[Value::Ptr(0)]).unwrap(), Value::Int(0));
        assert_eq!(chk_ptr(&[]).unwrap(), Value::Int(0));
    }

    #[test]
    fn chk_ptr_nonzero_is_valid() {
        assert_eq!(chk_ptr(&[Value::Ptr(0x1000)]).unwrap(), Value::Int(1));
        assert_eq!(chk_ptr(&[Value::Int(42)]).unwrap(), Value::Int(1));
    }

    #[test]
    fn sleep_zero_is_instant() {
        // Just verify it doesn't panic
        sleep(&[Value::Int(0)]).unwrap();
    }

    #[test]
    fn get_ticks_increases() {
        let t1 = get_ticks(&[]).unwrap();
        let t2 = get_ticks(&[]).unwrap();
        if let (Value::Int(a), Value::Int(b)) = (t1, t2) {
            assert!(b >= a, "ticks should be monotonically non-decreasing");
        }
    }

    #[test]
    fn doc_puts_outputs_text() {
        let (out, ()) = crate::capture::with_capture(|| {
            doc_puts(&[Value::Str("hello".into())]).unwrap();
        });
        assert_eq!(out, "hello");
    }

    #[test]
    fn gr_print_outputs_fmt() {
        let (out, ()) = crate::capture::with_capture(|| {
            gr_print(&[
                Value::Int(10),
                Value::Int(20),
                Value::Str("x=%d\n".into()),
                Value::Int(42),
            ])
            .unwrap();
        });
        assert_eq!(out, "x=42\n");
    }
}
