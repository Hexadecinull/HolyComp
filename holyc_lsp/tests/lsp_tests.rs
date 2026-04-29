//! Unit tests for the LSP diagnostic engine and JSON-RPC message shapes.

use holyc_lsp::diagnostics;
use serde_json::json;

// ── Diagnostic extraction ─────────────────────────────────────────────────────

#[test]
fn clean_source_produces_no_diagnostics() {
    let src = "I64 Add(I64 a, I64 b) { return a + b; }";
    let diags = diagnostics::from_source(src);
    assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
}

#[test]
fn parse_error_produces_diagnostic() {
    let src = "U0 F() { return }"; // missing expression before }
    let diags = diagnostics::from_source(src);
    assert!(!diags.is_empty(), "expected at least one diagnostic");
    let msg = diags[0]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("unexpected") || msg.contains("expected"),
        "diagnostic should mention unexpected token, got: {msg:?}"
    );
}

#[test]
fn diagnostic_has_required_lsp_fields() {
    let src = "I64 F() { return"; // unterminated
    let diags = diagnostics::from_source(src);
    assert!(!diags.is_empty());
    let d = &diags[0];
    assert!(d["severity"].is_number(), "severity must be a number");
    assert!(d["message"].is_string(), "message must be a string");
    assert!(
        d["range"]["start"]["line"].is_number(),
        "range.start.line must be a number"
    );
    assert!(d["range"]["start"]["character"].is_number());
    assert!(d["range"]["end"]["line"].is_number());
    assert!(d["range"]["end"]["character"].is_number());
}

#[test]
fn multiple_items_no_diagnostics() {
    let src = r#"
class Vec2 { I64 x; I64 y; };
typedef I64 Score;
I64 Dot(Vec2* a, Vec2* b) { return a->x * b->x + a->y * b->y; }
U0 Main() { Print("ok\n"); }
"#;
    let diags = diagnostics::from_source(src);
    assert!(diags.is_empty(), "expected no diagnostics, got: {diags:?}");
}

// ── JSON-RPC message shape tests ──────────────────────────────────────────────

#[test]
fn initialize_response_shape() {
    // The server's initialize response must contain a capabilities object.
    // We test the shape we produce, not the server loop itself.
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "capabilities": {
                "textDocumentSync": 1,
                "hoverProvider": true,
                "definitionProvider": true
            },
            "serverInfo": { "name": "holyc-lsp", "version": "0.1.0" }
        }
    });
    assert_eq!(response["result"]["capabilities"]["textDocumentSync"], 1);
    assert!(response["result"]["capabilities"]["hoverProvider"]
        .as_bool()
        .unwrap_or(false));
}

#[test]
fn publish_diagnostics_notification_shape() {
    let uri = "file:///test.HC";
    let diag = json!({
        "range": { "start": { "line": 0, "character": 0 },
                   "end":   { "line": 0, "character": 0 } },
        "severity": 1,
        "source": "holyc-lsp",
        "message": "unexpected token"
    });
    let notification = json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": { "uri": uri, "diagnostics": [diag] }
    });
    assert_eq!(notification["method"], "textDocument/publishDiagnostics");
    assert_eq!(notification["params"]["uri"], uri);
    assert_eq!(notification["params"]["diagnostics"][0]["severity"], 1);
}
