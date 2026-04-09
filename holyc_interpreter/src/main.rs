//! `holyc` binary — interpreter CLI entry point.

use holyc_interpreter::vm::Interpreter;
use holyc_frontend::{Lexer, Parser as HolyParser};
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!("Usage: holyc [repl | run <file.HC> | <file.HC>]");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        [] => {
            holyc_interpreter::repl::start();
        }
        [a] if a == "repl" => {
            holyc_interpreter::repl::start();
        }
        [file] => {
            run_file(PathBuf::from(file));
        }
        [sub, file] if sub == "run" => {
            run_file(PathBuf::from(file));
        }
        _ => usage(),
    }
}

fn run_file(path: PathBuf) {
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{}`: {e}", path.display());
            std::process::exit(1);
        }
    };
    let filename = path.display().to_string();
    let tokens = match Lexer::new(&src).tokenize() {
        Ok(t)  => t,
        Err(e) => { eprintln!("{filename}: lex error: {e}"); std::process::exit(1); }
    };
    let module = match HolyParser::new(tokens).parse_module() {
        Ok(m)  => m,
        Err(e) => { eprintln!("{filename}: parse error: {e}"); std::process::exit(1); }
    };
    let mut interp = Interpreter::new();
    if let Err(e) = interp.exec_module(&module) {
        eprintln!("{filename}: runtime error: {e}");
        std::process::exit(1);
    }
}
