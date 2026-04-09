//! Thread-local stdout capture.
//!
//! The [`Print`](crate::builtins::print) builtin writes via [`output`] instead
//! of directly to `std::io::stdout`.  In normal execution the sink is real
//! stdout; inside [`with_capture`] all output is collected into a `String` so
//! tests can assert on what HolyC programs printed.
//!
//! ## Usage in tests
//!
//! ```rust
//! use holyc_stdlib::capture;
//!
//! let (out, _) = capture::with_capture(|| {
//!     capture::output("hello\n");
//! });
//! assert_eq!(out, "hello\n");
//! ```
//!
//! ## Nesting
//!
//! `with_capture` is re-entrant: each call saves and restores the previous
//! buffer, so nested invocations each capture only their own output.

use std::cell::RefCell;

// ── Thread-local sink ─────────────────────────────────────────────────────────

thread_local! {
    /// `None`       → write to real stdout.
    /// `Some(text)` → append to the capture buffer.
    static SINK: RefCell<Option<String>> = const { RefCell::new(None) };
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Write `text` to the active output sink.
///
/// During normal execution this calls `print!`; inside [`with_capture`] it
/// appends to the capture buffer.
pub fn output(text: &str) {
    SINK.with(|cell| {
        match cell.borrow_mut().as_mut() {
            Some(buf) => buf.push_str(text),
            None      => print!("{text}"),
        }
    });
}

/// Run `f` with all [`output`] calls redirected into a fresh `String`.
///
/// Saves and restores any enclosing capture context, so nesting is safe.
/// Returns `(captured_text, return_value_of_f)`.
pub fn with_capture<F, T>(f: F) -> (String, T)
where
    F: FnOnce() -> T,
{
    // Save whatever was previously in the slot (may be None or Some outer buf).
    let prev = SINK.with(|cell| cell.borrow_mut().replace(String::new()));

    let result = f();

    // Extract our buffer and restore the previous slot.
    let captured = SINK.with(|cell| {
        let our_buf = cell.borrow_mut().take().unwrap_or_default();
        // Restore previous context (None if we were at the outermost level).
        *cell.borrow_mut() = prev;
        our_buf
    });

    (captured, result)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_basic() {
        let (out, _) = with_capture(|| {
            output("hello ");
            output("world\n");
        });
        assert_eq!(out, "hello world\n");
    }

    #[test]
    fn capture_empty() {
        let (out, _) = with_capture(|| {});
        assert_eq!(out, "");
    }

    #[test]
    fn capture_returns_value() {
        let (out, val) = with_capture(|| {
            output("x");
            42u32
        });
        assert_eq!(out, "x");
        assert_eq!(val, 42);
    }

    #[test]
    fn capture_nested_independent() {
        let (outer, _) = with_capture(|| {
            output("a\n");

            let (inner, _) = with_capture(|| {
                output("b\n");
            });
            assert_eq!(inner, "b\n", "inner capture should only contain inner output");

            output("c\n");
        });
        assert_eq!(outer, "a\nc\n", "outer capture must not contain inner output");
    }

    #[test]
    fn no_capture_doesnt_panic() {
        // Verify no active capture buffer at the top level.
        SINK.with(|cell| assert!(cell.borrow().is_none()));
        // `output` falls through to `print!` — just ensure no panic.
    }
}
