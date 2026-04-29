//! `holyc-lsp` — Language Server Protocol stub for HolyC.
//!
//! Implements a minimal subset of LSP 3.17 over stdin/stdout (JSON-RPC):
//!
//! - `initialize` / `initialized` / `shutdown` / `exit`
//! - `textDocument/didOpen` + `textDocument/didChange` — re-parse and publish diagnostics
//! - `textDocument/hover` — show the HolyC type of the identifier under cursor (stub)
//! - `textDocument/definition` — go-to-definition (stub, returns the cursor position)
//!
//! This is intentionally minimal: it validates the LSP wire protocol and
//! provides real parse-error diagnostics.  Semantic features (type inference,
//! cross-file resolution) are deferred to a later phase.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

use serde::Deserialize;
use serde_json::{json, Value};

use holyc_frontend::{Lexer, Parser};

// ── JSON-RPC plumbing ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RpcRequest {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn send(writer: &mut impl Write, msg: &Value) {
    let body = msg.to_string();
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    writer.flush().unwrap();
}

fn respond(writer: &mut impl Write, id: &Value, result: Value) {
    send(
        writer,
        &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
    );
}

fn notify(writer: &mut impl Write, method: &str, params: Value) {
    send(
        writer,
        &json!({ "jsonrpc": "2.0", "method": method, "params": params }),
    );
}

// ── Server state ──────────────────────────────────────────────────────────────

struct Server {
    documents: HashMap<String, String>,
    shutdown_requested: bool,
}

impl Server {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            shutdown_requested: false,
        }
    }

    fn handle(&mut self, writer: &mut impl Write, req: RpcRequest) {
        match req.method.as_str() {
            "initialize" => {
                let id = req.id.unwrap_or(Value::Null);
                respond(
                    writer,
                    &id,
                    json!({
                        "capabilities": {
                            "textDocumentSync": 1,
                            "hoverProvider": true,
                            "definitionProvider": true
                        },
                        "serverInfo": { "name": "holyc-lsp", "version": env!("CARGO_PKG_VERSION") }
                    }),
                );
            },
            "initialized" => {},
            "shutdown" => {
                self.shutdown_requested = true;
                if let Some(id) = req.id {
                    respond(writer, &id, Value::Null);
                }
            },
            "exit" => {
                std::process::exit(if self.shutdown_requested { 0 } else { 1 });
            },

            "textDocument/didOpen" => {
                if let Some(p) = req.params {
                    if let (Some(uri), Some(text)) = (
                        p["textDocument"]["uri"].as_str(),
                        p["textDocument"]["text"].as_str(),
                    ) {
                        let uri = uri.to_owned();
                        let text = text.to_owned();
                        self.documents.insert(uri.clone(), text.clone());
                        self.publish_diagnostics(writer, &uri, &text);
                    }
                }
            },

            "textDocument/didChange" => {
                if let Some(p) = req.params {
                    if let Some(uri) = p["textDocument"]["uri"].as_str() {
                        let uri = uri.to_owned();
                        // Take the last full-text change.
                        if let Some(changes) = p["contentChanges"].as_array() {
                            if let Some(last) = changes.last() {
                                if let Some(text) = last["text"].as_str() {
                                    let text = text.to_owned();
                                    self.documents.insert(uri.clone(), text.clone());
                                    self.publish_diagnostics(writer, &uri, &text);
                                }
                            }
                        }
                    }
                }
            },

            "textDocument/hover" => {
                // Stub: return the word at the cursor position as the hover content.
                let id = req.id.unwrap_or(Value::Null);
                if let Some(p) = req.params {
                    let uri = p["textDocument"]["uri"].as_str().unwrap_or("");
                    let line = p["position"]["line"].as_u64().unwrap_or(0) as usize;
                    let col = p["position"]["character"].as_u64().unwrap_or(0) as usize;
                    let word = self
                        .documents
                        .get(uri)
                        .and_then(|src| word_at(src, line, col))
                        .unwrap_or_default();
                    if word.is_empty() {
                        respond(writer, &id, Value::Null);
                    } else {
                        respond(
                            writer,
                            &id,
                            json!({
                                "contents": { "kind": "markdown", "value": format!("`{word}` — HolyC identifier") }
                            }),
                        );
                    }
                } else {
                    respond(writer, &id, Value::Null);
                }
            },

            "textDocument/definition" => {
                // Stub: return the cursor position itself (no cross-file indexing yet).
                let id = req.id.unwrap_or(Value::Null);
                if let Some(p) = req.params {
                    let uri = p["textDocument"]["uri"].as_str().unwrap_or("").to_owned();
                    let line = p["position"]["line"].clone();
                    let char = p["position"]["character"].clone();
                    respond(
                        writer,
                        &id,
                        json!([{
                            "uri": uri,
                            "range": { "start": { "line": line, "character": char },
                                       "end":   { "line": line, "character": char } }
                        }]),
                    );
                } else {
                    respond(writer, &id, Value::Null);
                }
            },

            // Ignore unknown notifications silently; return MethodNotFound for requests.
            method => {
                if let Some(id) = req.id {
                    send(
                        writer,
                        &json!({
                            "jsonrpc": "2.0", "id": id,
                            "error": { "code": -32601, "message": format!("method not found: {method}") }
                        }),
                    );
                }
            },
        }
    }

    fn publish_diagnostics(&self, writer: &mut impl Write, uri: &str, text: &str) {
        let diags = parse_diagnostics(text);
        notify(
            writer,
            "textDocument/publishDiagnostics",
            json!({
                "uri": uri,
                "diagnostics": diags
            }),
        );
    }
}

// ── Diagnostic extraction ─────────────────────────────────────────────────────

fn parse_diagnostics(src: &str) -> Vec<Value> {
    let mut diags = Vec::new();

    let tokens = match Lexer::new(src).tokenize() {
        Ok(t) => t,
        Err(e) => {
            let msg = e.to_string();
            // Extract line number from error if available.
            let line = lex_error_line(&e).saturating_sub(1) as u64;
            diags.push(lsp_diag(line, 0, &msg));
            return diags;
        },
    };

    if let Err(e) = Parser::new(tokens).parse_module() {
        let line = parse_error_line(&e).saturating_sub(1) as u64;
        diags.push(lsp_diag(line, 0, &e.to_string()));
    }

    diags
}

fn lsp_diag(line: u64, col: u64, msg: &str) -> Value {
    json!({
        "range": {
            "start": { "line": line, "character": col },
            "end":   { "line": line, "character": col }
        },
        "severity": 1,
        "source": "holyc-lsp",
        "message": msg
    })
}

fn lex_error_line(e: &holyc_frontend::LexError) -> u32 {
    use holyc_frontend::LexError::*;
    match e {
        UnexpectedChar { line, .. }
        | UnterminatedString { line }
        | UnterminatedChar { line }
        | EmptyChar { line }
        | UnterminatedBlockComment { line }
        | IntegerOverflow { line }
        | InvalidEscape { line, .. } => *line,
    }
}

fn parse_error_line(e: &holyc_frontend::ParseError) -> u32 {
    use holyc_frontend::ParseError::*;
    match e {
        UnexpectedToken { line, .. } => *line,
        UnexpectedEof { .. } => 0,
        Lex(le) => lex_error_line(le),
    }
}

// ── Word extraction ───────────────────────────────────────────────────────────

fn word_at(src: &str, line: usize, col: usize) -> Option<String> {
    let text = src.lines().nth(line)?;
    let chars: Vec<char> = text.chars().collect();
    let col = col.min(chars.len());
    if col == chars.len() || !chars[col].is_alphanumeric() {
        return None;
    }
    let start = chars[..col]
        .iter()
        .rposition(|c| !c.is_alphanumeric() && *c != '_')
        .map(|p| p + 1)
        .unwrap_or(0);
    let end = chars[col..]
        .iter()
        .position(|c| !c.is_alphanumeric() && *c != '_')
        .map(|p| col + p)
        .unwrap_or(chars.len());
    Some(chars[start..end].iter().collect())
}

// ── Main loop ─────────────────────────────────────────────────────────────────

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = stdout.lock();
    let mut server = Server::new();

    loop {
        // Read headers
        let mut content_length: usize = 0;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return; // EOF
            }
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(rest) = line.strip_prefix("Content-Length: ") {
                content_length = rest.trim().parse().unwrap_or(0);
            }
        }
        if content_length == 0 {
            continue;
        }

        // Read body
        let mut body = vec![0u8; content_length];
        use std::io::Read;
        if reader.read_exact(&mut body).is_err() {
            return;
        }

        let Ok(req) = serde_json::from_slice::<RpcRequest>(&body) else {
            continue;
        };
        server.handle(&mut writer, req);
    }
}
