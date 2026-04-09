//! Interactive REPL for HolyC.
//!
//! Uses plain `std::io::stdin` for portability.
//! When rustyline is re-enabled (toolchain >= 1.77), swap the `read_line`
//! implementation below for the rustyline version in `repl_rustyline.rs`.

use std::io::{self, BufRead, Write};
use holyc_frontend::{Lexer, Parser};
use crate::vm::Interpreter;

const BANNER: &str = "\
  _   _       _        _____
 | | | | ___ | |_   _ / ____|
 | |_| |/ _ \\| | | | | |
 |  _  | (_) | | |_| | |___
 |_| |_|\\___/|_|\\__, |\\_____|
                 __/ |
                |___/   HolyComp REPL  (type :help for commands)
";

pub fn start() {
    println!("{BANNER}");

    let stdin  = io::stdin();
    let stdout = io::stdout();

    let mut interp = Interpreter::new();
    let mut buf    = String::new(); // multi-line accumulator

    loop {
        // Prompt
        {
            let mut out = stdout.lock();
            let prompt = if buf.is_empty() { "hc> " } else { "... " };
            let _ = out.write_all(prompt.as_bytes());
            let _ = out.flush();
        }

        // Read a line
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => { println!("\nBye!"); break; } // EOF
            Ok(_) => {}
            Err(e) => { eprintln!("read error: {e}"); break; }
        }

        handle_line(&mut interp, &mut buf, line.trim_end_matches('\n').to_owned());
    }
}

fn handle_line(interp: &mut Interpreter, buf: &mut String, line: String) {
    match line.trim() {
        ":help" | ":h" => {
            println!("Commands:");
            println!("  :help     — this message");
            println!("  :quit     — exit the REPL");
            println!("  :clear    — discard buffered input");
            println!("  :reset    — reset interpreter state");
            println!("  Anything else is executed as HolyC source.");
            return;
        }
        ":quit" | ":q" | "exit" => { println!("Bye!"); std::process::exit(0); }
        ":clear"  => { buf.clear(); return; }
        ":reset"  => { *interp = Interpreter::new(); buf.clear(); println!("Interpreter reset."); return; }
        ""        => return,
        _         => {}
    }

    buf.push_str(&line);
    buf.push('\n');

    let tokens = match Lexer::new(buf).tokenize() {
        Ok(t)  => t,
        Err(e) => { eprintln!("lex error: {e}"); buf.clear(); return; }
    };

    match Parser::new(tokens).parse_module() {
        Ok(module) => {
            buf.clear();
            if let Err(e) = interp.exec_module(&module) {
                eprintln!("runtime error: {e}");
            }
        }
        Err(holyc_frontend::error::ParseError::UnexpectedEof { .. }) => {
            // Keep buffering — user hasn't finished the expression/block.
        }
        Err(e) => {
            eprintln!("parse error: {e}");
            buf.clear();
        }
    }
}
