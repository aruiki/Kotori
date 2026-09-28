#!/usr/bin/env bash
# 評価セットを取得し、SHA-256 を検証する(docs/SPEC.md 14.1)。
# 生データはコミットしない。取得先は eval/data/(.gitignore 済み)。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="${1:-$here/data}"

# AJIMEE-Bench(CC-BY-SA 3.0)。コミットを固定し、更新時はハッシュも変える。
AJIMEE_COMMIT="401666cd56d1a570c2021798b64b6da4396bfd45"
AJIMEE_URL="https://raw.githubusercontent.com/azooKey/AJIMEE-Bench/${AJIMEE_COMMIT}/JWTD_v2/v1/evaluation_items.json"
AJIMEE_SHA256="e9eb668fd6aa14b1e26436f429b5550108af0a1dfd443b8cea0bcb3ab3028fca"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

mkdir -p "$out"
dest="$out/ajimee-bench.json"
if [ -f "$dest" ] && [ "$(sha256 "$dest")" = "$AJIMEE_SHA256" ]; then
  exit 0
fi
echo "取得: AJIMEE-Bench" >&2
curl -fsSL --retry 3 -o "$dest.part" "$AJIMEE_URL"
actual="$(sha256 "$dest.part")"
if [ "$actual" != "$AJIMEE_SHA256" ]; then
  rm -f "$dest.part"
  echo "SHA-256 が一致しない: 期待 $AJIMEE_SHA256, 実際 $actual" >&2
  exit 1
fi
mv "$dest.part" "$dest"
