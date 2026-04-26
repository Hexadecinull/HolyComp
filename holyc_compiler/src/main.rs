//! `holyc-compile` — parse, validate, and optionally JIT-run HolyC source.
//!
//! Usage:
//!   holyc-compile <file.HC>             # parse only (no jit feature)
//!   holyc-compile <file.HC> --emit-llvm # print LLVM IR  (requires jit feature)
//!   holyc-compile <file.HC> --run       # JIT-execute Main() (requires jit feature)

mod abi;
mod codegen;

use std::path::PathBuf;

use holyc_frontend::{Lexer, Parser as HolyParser, TypeEnv};

use crate::codegen::CodegenSession;

fn usage() -> ! {
    eprintln!("Usage: holyc-compile <file.HC> [--emit-llvm | --run]");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = match args.first() {
        Some(p) => PathBuf::from(p),
        None => usage(),
    };
    let emit_llvm = args.iter().any(|a| a == "--emit-llvm");
    let run = args.iter().any(|a| a == "--run");

    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: cannot read `{}`: {e}", path.display());
        std::process::exit(1);
    });

    let tokens = Lexer::new(&src).tokenize().unwrap_or_else(|e| {
        eprintln!("lex error: {e}");
        std::process::exit(1);
    });

    let module = HolyParser::new(tokens).parse_module().unwrap_or_else(|e| {
        eprintln!("parse error: {e}");
        std::process::exit(1);
    });

    let env = TypeEnv::new();
    let session = CodegenSession::new();

    if emit_llvm {
        match session.emit_ir(&path.display().to_string(), &module, env) {
            Ok(ir) => print!("{ir}"),
            Err(e) => {
                eprintln!("codegen error: {e}");
                std::process::exit(1);
            },
        }
    } else if run {
        if let Err(e) = session.jit_run(&path.display().to_string(), &module, env) {
            eprintln!("JIT error: {e}");
            std::process::exit(1);
        }
    } else {
        eprintln!(
            "note: parsed {} top-level items from `{}` (use --emit-llvm or --run)",
            module.items.len(),
            path.display()
        );
    }
}
