//! `stem-lsp` — Language Server for the Stem markup language.
//!
//! Wire this into any LSP-aware editor:
//!
//! ```jsonc
//! // VS Code settings.json (with a generic-language-client extension):
//! "stem.server.path": "stem-lsp"
//! ```

mod backend;
mod conv;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(backend::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
