#!/usr/bin/env bash
# 配布物に同梱する THIRD_PARTY_NOTICES.txt を作る(docs/SPEC.md 16.1、REQ-16-1)。
# Rust のクレートは cargo-about で集め、llama.cpp と辞書(Mozc dictionary_oss)の
# ライセンス全文を後ろにつなげる。先に `just fetch-dict` で辞書ソースを取得しておく。
#
# 使い方: frontends/windows/notices.sh <出力ファイル>
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="${1:?出力ファイルを渡す}"
mozc="$root/data/dict-src/mozc"
llama="$root/third_party/llama.cpp"
for f in "$mozc/README.txt" "$mozc/LICENSE" "$llama/LICENSE"; do
  [ -f "$f" ] || { echo "$f がない(just fetch-dict と git submodule update --init)" >&2; exit 1; }
done

rule() { printf '\n========================================================================\n%s\n========================================================================\n\n' "$1"; }

{
  (cd "$root" && cargo about generate --workspace --locked about.hbs)
  rule "llama.cpp / ggml (MIT) — third_party/llama.cpp @ $(git -C "$llama" rev-parse HEAD)"
  cat "$llama/LICENSE"
  rule "Mozc (BSD-3-Clause) — 辞書ソースの取得元 google/mozc"
  cat "$mozc/LICENSE"
  rule "Mozc dictionary_oss(システム辞書の元データ。IPAdic と沖縄辞書を含む)"
  cat "$mozc/README.txt"
} > "$out"
echo "作った: $out" >&2
