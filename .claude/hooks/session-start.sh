#!/bin/bash
# Claude Code on the web のセッション開始時に、`just ci` を走らせる道具をそろえる。
# 何度実行しても同じ結果になる。終わった後のコンテナはキャッシュされる。
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(dirname "$0")/../..}"

export PATH="$HOME/.cargo/bin:$PATH"
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$CLAUDE_ENV_FILE"
fi

# rust-toolchain.toml の stable と rustfmt・clippy。
rustup toolchain install

if ! command -v just >/dev/null 2>&1; then
  cargo install just --locked
fi

# 依存を取得し、テストまでビルドしておく。最初の `just ci` が速くなる。
cargo test --workspace --locked --no-run
