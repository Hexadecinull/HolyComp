//! `holyc-fmt` — HolyC source formatter.
//!
//! ## Usage
//!
//! ```text
//! holyc-fmt <file.HC>          # format and print to stdout
//! holyc-fmt --write <file.HC>  # format in-place
//! holyc-fmt --check <file.HC>  # exit 0 if already formatted, 1 if not
//! holyc-fmt -                  # read from stdin, write to stdout
//! ```

use std::io::Read;
use std::path::PathBuf;

use holyc_frontend::{format_module, Lexer, Parser};

fn usage() -> ! {
    eprintln!("Usage: holyc-fmt [--write | --check] <file.HC>\n       holyc-fmt -  (reads stdin)");
    std::process::exit(1);
}

#[derive(PartialEq)]
enum Mode {
    Print,
    Write,
    Check,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
    }

    let mut mode = Mode::Print;
    let mut file: Option<PathBuf> = None;

    for a in &args {
        match a.as_str() {
            "--write" | "-w" => mode = Mode::Write,
            "--check" | "-c" => mode = Mode::Check,
            "-" => {}, // stdin marker — handled below
            s if s.starts_with('-') => {
                eprintln!("unknown flag: {s}");
                usage();
            },
            path => file = Some(PathBuf::from(path)),
        }
    }

    let from_stdin = args.iter().any(|a| a == "-");
    let src = if from_stdin {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .expect("stdin read failed");
        s
    } else {
        let path = file.as_ref().unwrap_or_else(|| usage());
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("error reading `{}`: {e}", path.display());
            std::process::exit(1);
        })
    };

    let tokens = Lexer::new(&src).tokenize().unwrap_or_else(|e| {
        eprintln!("lex error: {e}");
        std::process::exit(1);
    });
    let module = Parser::new_with_src(tokens, &src)
        .parse_module()
        .unwrap_or_else(|e| {
            eprintln!("parse error: {e}");
            std::process::exit(1);
        });

    let formatted = format_module(&module);

    match mode {
        Mode::Print => print!("{formatted}"),
        Mode::Write => {
            let path = file.as_ref().unwrap_or_else(|| usage());
            std::fs::write(path, &formatted).unwrap_or_else(|e| {
                eprintln!("write error: {e}");
                std::process::exit(1);
            });
            eprintln!("formatted: {}", path.display());
        },
        Mode::Check => {
            if src == formatted {
                std::process::exit(0);
            } else {
                eprintln!(
                    "not formatted: {}",
                    file.map(|p| p.display().to_string())
                        .unwrap_or_else(|| "<stdin>".into())
                );
                std::process::exit(1);
            }
        },
    }
}
