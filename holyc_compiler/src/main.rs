//! HolyC LLVM compiler (stub).

mod abi;
mod codegen;

use holyc_frontend::{Lexer, Parser as HolyParser};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = match args.first() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("Usage: holyc-compile <file.HC> [--emit-llvm]");
            std::process::exit(1);
        },
    };
    let emit_llvm = args.iter().any(|a| a == "--emit-llvm");

    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        },
    };

    let tokens = match Lexer::new(&src).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("lex error: {e}");
            std::process::exit(1);
        },
    };
    let module = match HolyParser::new(tokens).parse_module() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("parse error: {e}");
            std::process::exit(1);
        },
    };

    eprintln!(
        "note: LLVM backend not yet active — parsed {} top-level items from `{}`.",
        module.items.len(),
        path.display()
    );
    let _ = emit_llvm;
}
