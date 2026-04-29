const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');
let client;
function activate(context) {
    const lspPath = vscode.workspace.getConfiguration('holyc').get('lspPath','holyc-lsp');
    client = new LanguageClient('holyc','HolyC LSP',
        { run: { command: lspPath, transport: TransportKind.stdio },
          debug: { command: lspPath, transport: TransportKind.stdio } },
        { documentSelector: [{ scheme: 'file', language: 'holyc' }] });
    client.start();
}
function deactivate() { return client?.stop(); }
module.exports = { activate, deactivate };
