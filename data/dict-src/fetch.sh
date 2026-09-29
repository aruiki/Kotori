#!/usr/bin/env bash
# 辞書ソースを取得し、SHA-256 を検証する(docs/SPEC.md 5.3、17.1)。
# 生データはコミットしない。取得先は data/dict-src/mozc/(.gitignore 済み)。
#
# 使い方: data/dict-src/fetch.sh [出力ディレクトリ]
# 既に正しいハッシュのファイルがあれば取得し直さない。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="${1:-$here/mozc}"

# Mozc の dictionary_oss。コミットを固定し、更新時は mozc.sha256 と一緒に変える。
MOZC_COMMIT="a069a88d4cb5c011de0f9aebb6c149a1c808d904"
MOZC_BASE="https://raw.githubusercontent.com/google/mozc/${MOZC_COMMIT}/src/data/dictionary_oss"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

mkdir -p "$out"
while read -r expected name; do
  name="${name%$'\r'}"
  [ -z "$name" ] && continue
  dest="$out/$name"
  if [ -f "$dest" ] && [ "$(sha256 "$dest")" = "$expected" ]; then
    continue
  fi
  echo "取得: $name" >&2
  curl -fsSL --retry 3 -o "$dest.part" "$MOZC_BASE/$name"
  actual="$(sha256 "$dest.part")"
  if [ "$actual" != "$expected" ]; then
    rm -f "$dest.part"
    echo "SHA-256 が一致しない: $name (期待 $expected, 実際 $actual)" >&2
    exit 1
  fi
  mv "$dest.part" "$dest"
done < "$here/mozc.sha256"
# Mozc 本体のライセンス(BSD-3-Clause)。辞書を配布するときに同梱する(16.1)。
MOZC_LICENSE_SHA256="44cdd923b91ea9199293abecc2762c70c87dbf1e581c027a94c416368d1a648c"
dest="$out/LICENSE"
if ! { [ -f "$dest" ] && [ "$(sha256 "$dest")" = "$MOZC_LICENSE_SHA256" ]; }; then
  echo "取得: LICENSE" >&2
  curl -fsSL --retry 3 -o "$dest.part" "https://raw.githubusercontent.com/google/mozc/${MOZC_COMMIT}/LICENSE"
  actual="$(sha256 "$dest.part")"
  if [ "$actual" != "$MOZC_LICENSE_SHA256" ]; then
    rm -f "$dest.part"
    echo "SHA-256 が一致しない: LICENSE (期待 $MOZC_LICENSE_SHA256, 実際 $actual)" >&2
    exit 1
  fi
  mv "$dest.part" "$dest"
fi
echo "辞書ソースを $out に用意した" >&2
