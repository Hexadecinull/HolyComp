//! `holyc` binary — tree-walk interpreter CLI.
//!
//! ## Usage
//!
//! ```text
//! holyc                            # start REPL
//! holyc repl                       # start REPL explicitly
//! holyc <file.HC>                  # run a file
//! holyc run <file.HC>              # run a file (explicit subcommand)
//! holyc <file.HC> --check          # parse only, report errors
//! holyc <file.HC> --heap-size 128m # run with 128 MiB heap (default: 64 MiB)
//! ```

use holyc_frontend::{Lexer, Parser as HolyParser};
use holyc_interpreter::vm::Interpreter;
use std::path::PathBuf;

fn usage() -> ! {
    eprintln!(
        "Usage: holyc [repl | run] [<file.HC>] [options]\n\
         Options:\n  \
           --check              parse only, do not execute\n  \
           --heap-size <size>   interpreter heap size (default: 64m)\n                       \
               suffixes: b, k, m, g (bytes/KiB/MiB/GiB)\n  \
           --help               show this help"
    );
    std::process::exit(1);
}

struct Args {
    file: Option<PathBuf>,
    check: bool,
    heap_bytes: usize,
}

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();

    if raw.iter().any(|a| a == "--help" || a == "-h") {
        usage();
    }

    let mut file: Option<PathBuf> = None;
    let mut check = false;
    let mut heap_bytes: usize = 64 * 1024 * 1024; // 64 MiB default
    let mut i = 0usize;

    while i < raw.len() {
        match raw[i].as_str() {
            "--check" => check = true,
            "--heap-size" => {
                i += 1;
                heap_bytes = raw.get(i).and_then(|v| parse_size(v)).unwrap_or_else(|| {
                    eprintln!("error: invalid --heap-size value");
                    std::process::exit(1);
                });
            },
            "repl" | "run" => {}, // subcommands — handled by file presence
            s if s.starts_with('-') => {
                eprintln!("error: unknown flag `{s}`");
                usage();
            },
            path => {
                if file.is_none() {
                    file = Some(PathBuf::from(path));
                }
            },
        }
        i += 1;
    }

    // If no file but "repl" was in args, leave file as None → REPL.
    Args {
        file,
        check,
        heap_bytes,
    }
}

/// Parse a human-readable size string: `64m`, `128k`, `1g`, plain number = bytes.
fn parse_size(s: &str) -> Option<usize> {
    let s = s.to_ascii_lowercase();
    let (digits, mult) = if let Some(d) = s.strip_suffix('g') {
        (d, 1024 * 1024 * 1024usize)
    } else if let Some(d) = s.strip_suffix('m') {
        (d, 1024 * 1024)
    } else if let Some(d) = s.strip_suffix('k') {
        (d, 1024)
    } else if let Some(d) = s.strip_suffix('b') {
        (d, 1)
    } else {
        (s.as_str(), 1)
    };
    digits.parse::<usize>().ok().map(|n| n * mult)
}

fn main() {
    let args = parse_args();

    match args.file {
        None => holyc_interpreter::repl::start(),
        Some(path) => run_file(path, args.check, args.heap_bytes),
    }
}

fn run_file(path: PathBuf, check_only: bool, heap_bytes: usize) {
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error: cannot read `{}`: {e}", path.display());
        std::process::exit(1);
    });
    let filename = path.display().to_string();

    let tokens = Lexer::new(&src).tokenize().unwrap_or_else(|e| {
        eprintln!("{filename}: lex error: {e}");
        std::process::exit(1);
    });

    let module = HolyParser::new_with_src(tokens, &src)
        .parse_module()
        .unwrap_or_else(|e| {
            eprintln!("{filename}: parse error: {e}");
            std::process::exit(1);
        });

    if check_only {
        eprintln!("ok: {} items in `{filename}`", module.items.len());
        return;
    }

    let mut interp = Interpreter::with_heap_size(heap_bytes);
    if let Err(e) = interp.exec_module(&module) {
        eprintln!("{filename}: runtime error: {e}");
        std::process::exit(1);
    }
}
