#!/usr/bin/env bash
# Build stem-wasm and serve the playground at http://localhost:8080
#
# Auto-installs any missing prerequisites (wasm32 rust target,
# wasm-pack). After the first run, subsequent runs are fast.

set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$(pwd)"

say() { printf '\033[1;36m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33mwarning: %s\033[0m\n' "$*"; }

# 1. wasm32 target
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
  say "installing rust target: wasm32-unknown-unknown"
  rustup target add wasm32-unknown-unknown
fi

# 2. wasm-pack
if ! command -v wasm-pack >/dev/null 2>&1; then
  say "installing wasm-pack"
  cargo install wasm-pack
fi

# 3. python3 (for the dev server)
if ! command -v python3 >/dev/null 2>&1; then
  warn "python3 not found; install it or change this script to use another static server"
  exit 1
fi

# 4. build the wasm package
say "building stem-wasm (release) -> web/pkg/"
wasm-pack build crates/stem-wasm \
  --target web \
  --out-dir "$REPO_ROOT/web/pkg" \
  --release

# 5. serve
PORT="${PORT:-8080}"
say "serving web/ at http://localhost:${PORT}"
say "press Ctrl-C to stop"
cd web
exec python3 -m http.server "$PORT"
