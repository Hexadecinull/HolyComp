//! Minimal `printf`-style formatter that understands HolyC format strings.
//!
//! Supported specifiers: `%d`, `%i`, `%u`, `%x`, `%X`, `%f`, `%g`, `%c`, `%s`, `%%`.
//! Width and precision are accepted but ignored in this initial implementation.

/// Format a HolyC `Print`-style string with runtime values.
///
/// `args` is consumed left-to-right as specifiers are encountered.
/// Extra args are silently dropped; missing args render as `<?>`.
use crate::builtins::Value as PrintArg;

pub fn format_holyc(fmt: &str, args: &[crate::builtins::PrintArg]) -> String {
    let mut out = String::with_capacity(fmt.len() + 16);
    let mut chars = fmt.chars().peekable();
    let mut arg_i = 0usize;

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // We have '%'
        // Consume optional flags / width / precision (best-effort, not stored)
        loop {
            match chars.peek() {
                Some('-' | '+' | ' ' | '0' | '#') => {
                    chars.next();
                },
                Some(c) if c.is_ascii_digit() => {
                    chars.next();
                },
                Some('.') => {
                    chars.next();
                    while chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                        chars.next();
                    }
                },
                _ => break,
            }
        }

        let spec = match chars.next() {
            Some(s) => s,
            None => break,
        };

        match spec {
            '%' => {
                out.push('%');
                continue;
            },
            '\n' => {
                out.push('\n');
                continue;
            },
            _ => {},
        }

        let arg = args.get(arg_i);
        arg_i += 1;

        match spec {
            'd' | 'i' => match arg {
                Some(PrintArg::Int(n)) => out.push_str(&n.to_string()),
                Some(PrintArg::UInt(n)) => out.push_str(&(*n as i64).to_string()),
                Some(PrintArg::Float(f)) => out.push_str(&(*f as i64).to_string()),
                _ => out.push_str("<?>"),
            },
            'u' => match arg {
                Some(PrintArg::UInt(n)) => out.push_str(&n.to_string()),
                Some(PrintArg::Int(n)) => out.push_str(&(*n as u64).to_string()),
                _ => out.push_str("<?>"),
            },
            'x' => match arg {
                Some(PrintArg::Int(n)) => out.push_str(&format!("{:x}", *n as u64)),
                Some(PrintArg::UInt(n)) => out.push_str(&format!("{n:x}")),
                _ => out.push_str("<?>"),
            },
            'X' => match arg {
                Some(PrintArg::Int(n)) => out.push_str(&format!("{:X}", *n as u64)),
                Some(PrintArg::UInt(n)) => out.push_str(&format!("{n:X}")),
                _ => out.push_str("<?>"),
            },
            'f' => match arg {
                Some(PrintArg::Float(f)) => out.push_str(&format!("{f:.6}")),
                Some(PrintArg::Int(n)) => out.push_str(&format!("{:.6}", *n as f64)),
                _ => out.push_str("<?>"),
            },
            'g' => match arg {
                Some(PrintArg::Float(f)) => out.push_str(&format!("{f}")),
                Some(PrintArg::Int(n)) => out.push_str(&format!("{}", *n as f64)),
                _ => out.push_str("<?>"),
            },
            'c' => match arg {
                Some(PrintArg::Char(c)) => out.push(*c as char),
                Some(PrintArg::Int(n)) => out.push(*n as u8 as char),
                _ => out.push_str("<?>"),
            },
            's' => match arg {
                Some(PrintArg::Str(s)) => out.push_str(s),
                Some(other) => out.push_str(&other.to_string()),
                None => out.push_str("<?>"),
            },
            other => {
                // Unknown specifier — emit literally
                out.push('%');
                out.push(other);
                arg_i -= 1; // don't consume an arg
            },
        }
    }

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::PrintArg;

    #[test]
    fn test_no_specifiers() {
        let s = format_holyc("Hello, World!\n", &[]);
        assert_eq!(s, "Hello, World!\n");
    }

    #[test]
    fn test_percent_d() {
        let s = format_holyc(
            "%d + %d = %d",
            &[PrintArg::Int(1), PrintArg::Int(2), PrintArg::Int(3)],
        );
        assert_eq!(s, "1 + 2 = 3");
    }

    #[test]
    fn test_percent_s() {
        let s = format_holyc("Hello, %s!", &[PrintArg::Str("HolyC".into())]);
        assert_eq!(s, "Hello, HolyC!");
    }

    #[test]
    fn test_percent_x() {
        let s = format_holyc("0x%x", &[PrintArg::Int(255)]);
        assert_eq!(s, "0xff");
    }

    #[test]
    fn test_percent_percent() {
        let s = format_holyc("100%%", &[]);
        assert_eq!(s, "100%");
    }

    #[test]
    fn test_float() {
        let s = format_holyc("%f", &[PrintArg::Float(3.14)]);
        assert!(s.starts_with("3.14"), "got: {s}");
    }
}
