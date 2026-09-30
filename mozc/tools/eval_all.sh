#!/usr/bin/env bash
# 品質 3 段階 × 評価セット 4 つの Acc@1 を出す(前の文あり)。Mozc 単体より悪くなった問題も出す。
#
# 使い方: mozc/tools/eval_all.sh <converter_main.exe> [インストール先(モデルのある所)]
# Mozc 単体の結果は、モデルのない空のフォルダで動かして作る(最初に 1 回)。
# 結果は $OUT(既定 ${TMPDIR:-/tmp}/kotori-eval)に、品質とセットごとに <品質>_<セット>.json と
# 実行条件(.manifest.json)を残す。評価が 1 つでも失敗したら、最後に終了コード 1 を返す。
set -u
cd "$(dirname "$0")/../.."
M="$1"
I="${2:-C:\Program Files (x86)\Kotori}"
OUT="${OUT:-${TMPDIR:-/tmp}/kotori-eval}"
mkdir -p "$OUT/empty"
export PYTHONIOENCODING=utf-8 KOTORI_LM_PRELOAD=0
SETS="eval/data/ajimee-bench.json eval/sets/kotori-idioms.json eval/sets/kotori-nuance.json eval/sets/kotori-daily.json"
failed=0
# 1 回の評価。失敗したら覚えておき、残りは続ける。
run() {
  local label=$1 out=$2
  shift 2
  local r
  if r=$("$@" python mozc/eval_baseline.py --context --data "$d" --out "$out" "$M"); then
    echo "$label: $(echo "$r" | head -1)"
  else
    echo "$label: 失敗"
    failed=1
    return 1
  fi
}
for d in $SETS; do
  n=$(basename "$d" .json)
  run "Mozc $n" "$OUT/Mozc_$n.json" env KOTORI_RUNTIME_DIR="$OUT/empty" KOTORI_MODEL_DIR="$OUT/empty"
done
# 品質ごとの設定は rewriter/lm_rewriter.cc の QualityOf と同じ(生成の数、LLM で採点する数、zenz)。
# Low の zenz(medium)は、パスに空白があってもよいように別に渡す(区切りは / にする)。
LOW_ZENZ="$(printf '%s' "$I" | tr '\\' /)/zenz-v2.5-medium-q8_0.gguf"
for q in "Low KOTORI_LM_BEAMS=2 KOTORI_LM_LLM_TOP=0" "Standard KOTORI_LM_BEAMS=4 KOTORI_LM_LLM_TOP=4" \
         "High KOTORI_LM_BEAMS=8 KOTORI_LM_LLM_TOP=32"; do
  set -- $q; name=$1; shift
  zenz=()
  [ "$name" = Low ] && zenz=("KOTORI_ZENZ_MODEL=$LOW_ZENZ")
  for d in $SETS; do
    n=$(basename "$d" .json)
    run "$name $n" "$OUT/${name}_$n.json" env KOTORI_RUNTIME_DIR="$I" KOTORI_MODEL_DIR="$I" "$@" "${zenz[@]}" \
      || continue
    python - "$OUT/Mozc_$n.json" "$OUT/${name}_$n.json" <<'PY'
import json, sys
m = json.load(open(sys.argv[1], encoding="utf-8"))
k = json.load(open(sys.argv[2], encoding="utf-8"))
for a, b in zip(m, k):
    if a["ok"] and not b["ok"]:
        print("    悪化:", a["top1"][:36], "->", b["top1"][:36])
PY
  done
done
exit $failed
