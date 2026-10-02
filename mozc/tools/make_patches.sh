#!/usr/bin/env bash
# Mozc の作業ツリーの変更から mozc/patches/0002・0003 を作り直す(0001 はそのまま)。
#
# 使い方(WSL などの bash): mozc/tools/make_patches.sh <Mozc の作業ツリー> <素の Mozc の clone>
#   作業ツリー: google/mozc a069a88d に 0001〜0003 を当てて手を入れたもの(Windows なら /mnt/c/... で指す)
#   素の clone: google/mozc を clone したもの(a069a88d を含む)。作業用に ~/kotori-patch-work を作る。
# モデル(*.gguf)、third_party、bazel の出力は含めない。インストーラと同梱物(src/win32/installer、
# src/data/kotori)は 0003、それ以外は 0002 に入れる。最後に素のコミットに 0001〜0003 が順に当たるか確かめる。
set -euo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
MZ="$1"
BASE_REPO="$2"
BASE=a069a88d4cb5c011de0f9aebb6c149a1c808d904
WORK="$HOME/kotori-patch-work"
cd "$MZ"
FILES=$( (git -c core.quotepath=off diff --name-only $BASE; git -c core.quotepath=off ls-files --others --exclude-standard) |
  grep -v '\.gguf$' | grep -v '^src/third_party' | grep -v '^src/bazel-' | sort -u)
echo "$(echo "$FILES" | wc -l) files"
# 作り直す clone は 1 行ずつ確かめる(&& でつなぐと set -e が効かず、失敗すると Mozc の作業ツリーのまま
# 続けて、ファイルを自分自身に写して空にしてしまう。2026-10-02 に起きた)。
rm -rf "$WORK"
if [ -e "$WORK" ]; then
  echo "$WORK を消せない(Bazel のサーバーなどが開いている。cd $WORK/src && bazelisk shutdown)" >&2
  exit 1
fi
git clone -q "$BASE_REPO" "$WORK"
cd "$WORK"
if [ "$(pwd -P)" = "$(cd "$MZ" && pwd -P)" ]; then
  echo "作業用の clone に移れていない" >&2
  exit 1
fi
git checkout -q $BASE
git -c user.email=k@k -c user.name=k commit -q --allow-empty -m base
git apply "$R/mozc/patches/0001-kotori-branding.patch" && git add -A && git -c user.email=k@k -c user.name=k commit -q -m 0001
for f in $FILES; do
  mkdir -p "$(dirname "$f")"
  case "$f" in
    # バイナリはそのまま写す(改行を消すと壊れる)
    *.bmp|*.ico|*.png|*.jpg|*.dll|*.exe) cp "$MZ/$f" "$f" ;;
    *) tr -d '\r' < "$MZ/$f" > "$f" ;;
  esac
done
git add -A
git diff --cached --binary -- . ':!src/win32/installer' ':!src/data/kotori' > "$R/mozc/patches/0002-kotori-lm-rerank.patch"
git diff --cached --binary -- src/win32/installer src/data/kotori > "$R/mozc/patches/0003-kotori-bundle-model.patch"
git diff --cached --stat | tail -1
git reset -q --hard $BASE && git clean -fdq
for p in "$R"/mozc/patches/*.patch; do git apply --check "$p" && git apply "$p" && echo "ok $(basename "$p")"; done
