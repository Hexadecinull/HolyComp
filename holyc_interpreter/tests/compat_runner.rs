//! End-to-end compatibility test runner.
//!
//! Each test lexes, parses, and executes a `.HC` file from `tests/compat/`
//! through the tree-walk interpreter and asserts the captured stdout matches
//! the `// EXPECT: <line>` annotations embedded at the top of each file.
//!
//! A file with no `// EXPECT:` lines just asserts clean (non-panicking) execution.

use holyc_frontend::{Lexer, Parser};
use holyc_interpreter::vm::Interpreter;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn run_source(src: &str, label: &str) -> String {
    let (output, ()) = holyc_stdlib::capture::with_capture(|| {
        let tokens = Lexer::new(src)
            .tokenize()
            .unwrap_or_else(|e| panic!("{label}: lex error: {e}"));
        let module = Parser::new(tokens)
            .parse_module()
            .unwrap_or_else(|e| panic!("{label}: parse error: {e}"));
        let mut interp = Interpreter::new();
        interp
            .exec_module(&module)
            .unwrap_or_else(|e| panic!("{label}: runtime error: {e}"));
    });
    output
}

fn extract_expected(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("// EXPECT:")
                .map(|rest| rest.trim().to_owned())
        })
        .collect()
}

fn assert_output(label: &str, src: &str) {
    let expected = extract_expected(src);
    let output = run_source(src, label);

    if expected.is_empty() {
        // No EXPECT annotations — just verify clean execution (already done above).
        return;
    }

    let got_lines: Vec<&str> = output.lines().collect();

    assert_eq!(
        expected.len(),
        got_lines.len(),
        "{label}: expected {} line(s) of output, got {}\n--- expected ---\n{}\n--- got ---\n{}",
        expected.len(),
        got_lines.len(),
        expected.join("\n"),
        output.trim_end(),
    );

    for (i, (exp, got)) in expected.iter().zip(got_lines.iter()).enumerate() {
        assert_eq!(
            exp.as_str(),
            *got,
            "{label}: output line {} mismatch\n  expected: {exp:?}\n  got:      {got:?}",
            i + 1,
        );
    }
}

// ── Test cases ────────────────────────────────────────────────────────────────

#[test]
fn compat_hello() {
    assert_output("hello.HC", include_str!("../../tests/compat/hello.HC"));
}

#[test]
fn compat_arithmetic() {
    assert_output(
        "arithmetic.HC",
        include_str!("../../tests/compat/arithmetic.HC"),
    );
}

#[test]
fn compat_loops() {
    assert_output("loops.HC", include_str!("../../tests/compat/loops.HC"));
}

#[test]
fn compat_switch() {
    assert_output("switch.HC", include_str!("../../tests/compat/switch.HC"));
}

#[test]
fn compat_fibonacci() {
    assert_output(
        "fibonacci.HC",
        include_str!("../../tests/compat/fibonacci.HC"),
    );
}

#[test]
fn compat_pointers() {
    assert_output(
        "pointers.HC",
        include_str!("../../tests/compat/pointers.HC"),
    );
}

#[test]
fn compat_classes() {
    assert_output("classes.HC", include_str!("../../tests/compat/classes.HC"));
}

#[test]
fn compat_strings() {
    assert_output("strings.HC", include_str!("../../tests/compat/strings.HC"));
}

#[test]
fn compat_do_while() {
    assert_output(
        "do_while.HC",
        include_str!("../../tests/compat/do_while.HC"),
    );
}

#[test]
fn compat_globals() {
    assert_output("globals.HC", include_str!("../../tests/compat/globals.HC"));
}

#[test]
fn compat_recursion() {
    assert_output(
        "recursion.HC",
        include_str!("../../tests/compat/recursion.HC"),
    );
}
