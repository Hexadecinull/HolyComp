//! `holyc-compile` — parse, validate, and compile HolyC source.
//!
//! ## Usage
//!
//! ```text
//! holyc-compile <file.HC>                              # parse only, report item count
//! holyc-compile <file.HC> --check                     # parse + validate, no output
//! holyc-compile <file.HC> --run                       # JIT-execute Main() [jit feature]
//! holyc-compile <file.HC> --emit-llvm                 # print LLVM IR   [jit feature]
//! holyc-compile <file.HC> --emit-asm   -o out.s       # write assembly  [jit feature]
//! holyc-compile <file.HC> -o out                      # compile + link  [jit feature]
//! holyc-compile <file.HC> --target x86_64-linux-gnu -o out.o  # cross-compile
//! ```

mod abi;
mod codegen;

use std::path::{Path, PathBuf};

use holyc_frontend::{Lexer, Parser as HolyParser, TypeEnv};

use crate::codegen::CodegenSession;

fn usage() -> ! {
    eprintln!(
        "Usage: holyc-compile <file.HC> [options]\n\
         Options:\n  \
           --check           parse only\n  \
           --run             JIT-execute Main()\n  \
           --emit-llvm       print LLVM IR\n  \
           --emit-asm        write assembly\n  \
           -o <path>         output path (default: a.out)\n  \
           --target <triple> cross-compile target\n  \
           --opt <0|1|2|3>   optimisation level (default 2)\n  \
           --help            show this help"
    );
    std::process::exit(1);
}

struct Args {
    input: PathBuf,
    check: bool,
    run: bool,
    emit_llvm: bool,
    emit_asm: bool,
    output: Option<PathBuf>,
    target: Option<String>,
    #[allow(dead_code)]
    opt: u8,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() || raw.iter().any(|a| a == "--help" || a == "-h") {
        usage();
    }
    let mut input = None;
    let mut check = false;
    let mut run = false;
    let mut emit_llvm = false;
    let mut emit_asm = false;
    let mut output: Option<PathBuf> = None;
    let mut target: Option<String> = None;
    let mut opt: u8 = 2;
    let mut i = 0usize;
    while i < raw.len() {
        match raw[i].as_str() {
            "--check" => check = true,
            "--run" => run = true,
            "--emit-llvm" => emit_llvm = true,
            "--emit-asm" => emit_asm = true,
            "-o" => {
                i += 1;
                output = raw.get(i).map(PathBuf::from);
            },
            "--target" => {
                i += 1;
                target = raw.get(i).cloned();
            },
            "--opt" => {
                i += 1;
                opt = raw.get(i).and_then(|v| v.parse().ok()).unwrap_or(2);
            },
            s if s.starts_with('-') => {
                eprintln!("error: unknown flag `{s}`");
                usage();
            },
            path => {
                if input.is_some() {
                    eprintln!("error: multiple input files not supported");
                    usage();
                }
                input = Some(PathBuf::from(path));
            },
        }
        i += 1;
    }
    let input = input.unwrap_or_else(|| usage());
    Args {
        input,
        check,
        run,
        emit_llvm,
        emit_asm,
        output,
        target,
        opt,
    }
}

fn main() {
    let args = parse_args();

    let src = std::fs::read_to_string(&args.input).unwrap_or_else(|e| {
        eprintln!("error: cannot read `{}`: {e}", args.input.display());
        std::process::exit(1);
    });

    let tokens = Lexer::new(&src).tokenize().unwrap_or_else(|e| {
        eprintln!("{}:lex: {e}", args.input.display());
        std::process::exit(1);
    });

    let module = HolyParser::new_with_src(tokens, &src)
        .parse_module()
        .unwrap_or_else(|e| {
            eprintln!("{}:parse: {e}", args.input.display());
            std::process::exit(1);
        });

    if args.check {
        eprintln!(
            "ok: {} items parsed from `{}`",
            module.items.len(),
            args.input.display()
        );
        return;
    }

    let name = args.input.display().to_string();
    let session = CodegenSession::new();

    if args.emit_llvm {
        let ir = session
            .emit_ir(&name, &module, TypeEnv::new())
            .unwrap_or_else(|e| {
                eprintln!("codegen: {e}");
                std::process::exit(1);
            });
        match &args.output {
            Some(p) => std::fs::write(p, &ir).unwrap_or_else(|e| {
                eprintln!("write: {e}");
                std::process::exit(1);
            }),
            None => print!("{ir}"),
        }
        return;
    }

    if args.run {
        session
            .jit_run(&name, &module, TypeEnv::new())
            .unwrap_or_else(|e| {
                eprintln!("JIT: {e}");
                std::process::exit(1);
            });
        return;
    }

    if args.emit_asm {
        let out = args
            .output
            .clone()
            .unwrap_or_else(|| args.input.with_extension("s"));
        aot_op(
            || {
                session.emit_asm_file(
                    &name,
                    &module,
                    TypeEnv::new(),
                    &out,
                    args.target.as_deref(),
                    args.opt,
                )
            },
            &out,
        );
        return;
    }

    // Default: compile to executable (or .o if output ends with .o)
    let out = args
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from("a.out"));
    if out.extension().is_some_and(|e| e == "o") {
        aot_op(
            || {
                session.emit_object(
                    &name,
                    &module,
                    TypeEnv::new(),
                    &out,
                    args.target.as_deref(),
                    args.opt,
                )
            },
            &out,
        );
    } else {
        aot_op(
            || {
                session.emit_executable(
                    &name,
                    &module,
                    TypeEnv::new(),
                    &out,
                    args.target.as_deref(),
                    args.opt,
                )
            },
            &out,
        );
    }
}

fn aot_op<F: FnOnce() -> Result<(), crate::codegen::CodegenError>>(f: F, out: &Path) {
    f().unwrap_or_else(|e| {
        eprintln!("compile: {e}");
        let _ = std::fs::remove_file(out);
        std::process::exit(1);
    });
}
