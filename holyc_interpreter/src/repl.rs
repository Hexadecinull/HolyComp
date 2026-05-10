//! Interactive REPL for the HolyC interpreter.
//!
//! Reads source line-by-line, accumulates multi-line input until the parser
//! signals completion, then executes the parsed module.  Error diagnostics
//! are rendered using [`holyc_frontend::diag`] for rustc-style output.

use std::io::{self, BufRead, Write};

use holyc_frontend::{
    diag::{from_lex_error, from_parse_error},
    error::ParseError,
    Lexer, Parser,
};

use crate::vm::Interpreter;

const BANNER: &str = "\
  _   _       _        _____
 | | | | ___ | |_   _ / ____|
 | |_| |/ _ \\| | | | | |
 |  _  | (_) | | |_| | |___
 |_| |_|\\___/|_|\\__, |\\____|  HolyC REPL
                |___/  type :help for commands
";

pub fn start() {
    println!("{BANNER}");

    let stdin = io::stdin();
    let stdout = io::stdout();

    let mut interp = Interpreter::new();
    let mut buf = String::new(); // multi-line accumulator

    loop {
        // Prompt
        {
            let mut out = stdout.lock();
            let prompt = if buf.is_empty() { "hc> " } else { "... " };
            write!(out, "{prompt}").ok();
            out.flush().ok();
        }

        // Read a line
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => {
                println!("\nBye!");
                break;
            }, // EOF
            Ok(_) => {},
            Err(e) => {
                eprintln!("read error: {e}");
                break;
            },
        }

        handle_line(
            &mut interp,
            &mut buf,
            line.trim_end_matches('\n').to_owned(),
        );
    }
}

fn handle_line(interp: &mut Interpreter, buf: &mut String, line: String) {
    match line.trim() {
        ":help" | ":h" => {
            println!("  :help     — this help");
            println!("  :reset    — reset interpreter state");
            println!("  :clear    — clear the input buffer");
            println!("  :quit     — exit");
            println!("  :heap     — show heap statistics");
            println!("  Anything else is executed as HolyC source.");
            return;
        },
        ":quit" | ":q" | "exit" => {
            println!("Bye!");
            std::process::exit(0);
        },
        ":clear" => {
            buf.clear();
            return;
        },
        ":reset" => {
            *interp = Interpreter::new();
            buf.clear();
            println!("Interpreter reset.");
            return;
        },
        ":heap" => {
            println!(
                "heap: {}/{} bytes used, {} live allocations",
                interp.heap.used_bytes(),
                interp.heap.capacity(),
                interp.heap.live_allocs()
            );
            return;
        },
        "" => return,
        _ => {},
    }

    buf.push_str(&line);
    buf.push('\n');

    // Try to lex and parse; keep buffering on unexpected EOF.
    let tokens = match Lexer::new(buf).tokenize() {
        Ok(t) => t,
        Err(e) => {
            let diag = from_lex_error(&e, buf, Some("<repl>"));
            eprintln!("{diag}");
            buf.clear();
            return;
        },
    };

    match Parser::new_with_src(tokens, buf.as_str()).parse_module() {
        Ok(module) => {
            if let Err(e) = interp.exec_module(&module) {
                eprintln!("runtime error: {e}");
            }
            buf.clear();
        },
        Err(ParseError::UnexpectedEof { .. }) => {
            // Keep buffering — user hasn't finished the block.
        },
        Err(e) => {
            let diag = from_parse_error(&e, buf, None, Some("<repl>"));
            eprintln!("{diag}");
            buf.clear();
        },
    }
}
