//! End-to-end JIT tests.  Each `.HC` compat file is lexed, parsed, JIT-compiled
//! via LLVM, and its stdout (captured at the OS pipe level) is checked against
//! the `// EXPECT:` annotations embedded in each file.
//!
//! Only compiled when `--features jit` is supplied.

#![cfg(feature = "jit")]

use holyc_compiler::CodegenSession;
use holyc_frontend::{Lexer, Parser, TypeEnv};

// ── OS-level stdout capture ───────────────────────────────────────────────────
//
// The LLVM JIT calls real libc `printf`, which writes to file-descriptor 1.
// `holyc_stdlib::capture` only intercepts the Rust `print!` path, so we have
// to capture at the OS level using `pipe(2)` + `dup2(2)`.

use std::io::Read;
use std::os::unix::io::FromRawFd;

fn capture_jit<F: FnOnce()>(f: F) -> String {
    unsafe {
        // Create a pipe: read_fd ← write_fd
        let mut fds = [0i32; 2];
        libc::pipe(fds.as_mut_ptr());
        let (read_fd, write_fd) = (fds[0], fds[1]);

        // Save the real stdout fd.
        let saved = libc::dup(1);

        // Redirect stdout to the write end of the pipe.
        libc::dup2(write_fd, 1);
        libc::close(write_fd);

        // Run the JIT program.
        f();

        // Flush libc's stdout buffer before we restore.
        // Flush by closing the write end (pipe drain) — no libc::stdout() needed.
        libc::fflush(std::ptr::null_mut()); // fflush(NULL) flushes all streams

        // Restore real stdout.
        libc::dup2(saved, 1);
        libc::close(saved);

        // Read everything from the read end.
        let mut file = std::fs::File::from_raw_fd(read_fd);
        let mut out = String::new();
        file.read_to_string(&mut out).unwrap();
        out
    }
}

// ── Test helpers ──────────────────────────────────────────────────────────────

fn extract_expects(src: &str) -> Vec<String> {
    src.lines()
        .filter_map(|l| l.trim().strip_prefix("// EXPECT: "))
        .map(str::to_owned)
        .collect()
}

fn jit_output(label: &str, src: &str) -> String {
    let src = src.to_owned();
    let label = label.to_owned();
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            let tokens = Lexer::new(&src)
                .tokenize()
                .unwrap_or_else(|e| panic!("{label}: lex error: {e}"));
            let module = Parser::new(tokens)
                .parse_module()
                .unwrap_or_else(|e| panic!("{label}: parse error: {e}"));
            capture_jit(|| {
                CodegenSession::new()
                    .jit_run(&label, &module, TypeEnv::new())
                    .unwrap_or_else(|e| panic!("{label}: JIT error: {e}"));
            })
        })
        .expect("thread spawn")
        .join()
        .expect("JIT thread panicked")
}

fn assert_jit(label: &str, src: &str) {
    let expected = extract_expects(src);
    let output = jit_output(label, src);
    if expected.is_empty() {
        return;
    }
    let got: Vec<&str> = output.lines().collect();
    assert_eq!(
        got.len(),
        expected.len(),
        "{label}: expected {} lines, got {}.\nOutput:\n{output}",
        expected.len(),
        got.len()
    );
    for (i, (exp, got)) in expected.iter().zip(got.iter()).enumerate() {
        assert_eq!(
            exp.as_str(),
            *got,
            "{label}: line {} mismatch\n  expected: {exp:?}\n  got:      {got:?}",
            i + 1
        );
    }
}

// ── JIT compat tests ──────────────────────────────────────────────────────────

#[test]
fn jit_hello() {
    assert_jit("hello.HC", include_str!("../../tests/compat/hello.HC"));
}
#[test]
fn jit_arithmetic() {
    assert_jit(
        "arithmetic.HC",
        include_str!("../../tests/compat/arithmetic.HC"),
    );
}
#[test]
fn jit_fibonacci() {
    assert_jit(
        "fibonacci.HC",
        include_str!("../../tests/compat/fibonacci.HC"),
    );
}
#[test]
fn jit_loops() {
    assert_jit("loops.HC", include_str!("../../tests/compat/loops.HC"));
}
#[test]
fn jit_switch() {
    assert_jit("switch.HC", include_str!("../../tests/compat/switch.HC"));
}
#[test]
fn jit_globals() {
    assert_jit("globals.HC", include_str!("../../tests/compat/globals.HC"));
}
#[test]
fn jit_do_while() {
    assert_jit(
        "do_while.HC",
        include_str!("../../tests/compat/do_while.HC"),
    );
}
#[test]
fn jit_recursion() {
    assert_jit(
        "recursion.HC",
        include_str!("../../tests/compat/recursion.HC"),
    );
}

// ── LLVM IR shape ─────────────────────────────────────────────────────────────

#[test]
fn emit_ir_contains_define() {
    let src = "U0 Main() { Print(\"ok\\n\"); }";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let module = Parser::new(tokens).parse_module().unwrap();
    let ir = CodegenSession::new()
        .emit_ir("t", &module, TypeEnv::new())
        .unwrap();
    assert!(
        ir.contains("define"),
        "IR must contain a function definition:\n{ir}"
    );
    assert!(ir.contains("@printf"), "IR must reference printf:\n{ir}");
}

// ── AOT native executable test ─────────────────────────────────────────────────

#[test]
fn aot_hello_native_executable() {
    use std::process::Command;

    let src = include_str!("../../tests/compat/hello.HC");
    let out = std::env::temp_dir().join("holyc_aot_test_hello");

    let tokens = Lexer::new(src).tokenize().unwrap();
    let module = Parser::new(tokens).parse_module().unwrap();
    CodegenSession::new()
        .emit_executable("hello.HC", &module, TypeEnv::new(), &out, None, 2)
        .expect("AOT link failed");

    assert!(out.exists(), "executable not created");

    let output = Command::new(&out)
        .output()
        .expect("failed to run native executable");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "Hello, World!", "wrong output: {stdout:?}");

    let _ = std::fs::remove_file(&out);
}

#[test]
fn aot_emit_asm_file() {
    let src = "U0 Main() { Print(\"hi\\n\"); }";
    let out = std::env::temp_dir().join("holyc_aot_test.s");

    let tokens = Lexer::new(src).tokenize().unwrap();
    let module = Parser::new(tokens).parse_module().unwrap();
    CodegenSession::new()
        .emit_asm_file("test.HC", &module, TypeEnv::new(), &out, None, 2)
        .expect("emit_asm failed");

    let asm = std::fs::read_to_string(&out).expect("could not read asm");
    assert!(
        asm.contains("call"),
        "asm should contain a call instruction:\n{asm}"
    );
    let _ = std::fs::remove_file(&out);
}
