#!/usr/bin/env bash
# zenz-v2.5(Hugging Face、CC-BY-SA 4.0)を取得して SHA-256 を検証する(docs/SPEC.md 6.1)。
# モデルはコミットしない。取得先は training/zenz/models/(.gitignore 済み)。
#
# 使い方: fetch.sh [xsmall|small|medium] [出力ディレクトリ]   (既定は small)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
size="${1:-small}"
out="${2:-$here/models/zenz-v2.5-$size}"

case "$size" in
  xsmall) REVISION="9bfb00e795f89284fe9164f0fbde171d046fc546" ;;
  small) REVISION="1e408d69a7e284efa4e4d63e456f50e363a82953" ;;
  medium) REVISION="623bc8edf5417129b8269b628223d08245e9c68e" ;;
  *) echo "サイズは xsmall / small / medium のどれか: $size" >&2; exit 2 ;;
esac
REPO="Miwa-Keita/zenz-v2.5-$size"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

mkdir -p "$out"
while read -r expected name; do
  [ -z "$name" ] && continue
  dest="$out/$name"
  if [ -f "$dest" ] && [ "$(sha256 "$dest")" = "$expected" ]; then
    continue
  fi
  echo "取得: $name" >&2
  curl -fsSL --retry 3 -o "$dest.part" "https://huggingface.co/$REPO/resolve/$REVISION/$name"
  actual="$(sha256 "$dest.part")"
  if [ "$actual" != "$expected" ]; then
    rm -f "$dest.part"
    echo "SHA-256 が一致しない: $name (期待 $expected, 実際 $actual)" >&2
    exit 1
  fi
  mv "$dest.part" "$dest"
done < "$here/zenz-v2.5-$size.sha256"
echo "zenz-v2.5-$size を $out に用意した" >&2
