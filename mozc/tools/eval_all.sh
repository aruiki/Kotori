#!/usr/bin/env bash
# 品質 3 段階 × 評価セット 4 つの Acc@1 を出す(前の文あり)。Mozc 単体より悪くなった問題も出す。
#
# 使い方: mozc/tools/eval_all.sh <converter_main.exe> [インストール先(モデルのある所)]
# Mozc 単体の結果は、モデルのない空のフォルダで動かして作る(最初に 1 回)。
set -u
cd "$(dirname "$0")/../.."
M="$1"
I="${2:-C:\\Program Files (x86)\\Kotori}"
OUT="${TMPDIR:-/tmp}/kotori-eval"
mkdir -p "$OUT/empty"
export PYTHONIOENCODING=utf-8 KOTORI_LM_PRELOAD=0
SETS="eval/data/ajimee-bench.json eval/sets/kotori-idioms.json eval/sets/kotori-nuance.json eval/sets/kotori-daily.json"
for d in $SETS; do
  n=$(basename "$d" .json)
  echo -n "Mozc $n: "
  KOTORI_RUNTIME_DIR="$OUT/empty" KOTORI_MODEL_DIR="$OUT/empty" \
    python mozc/eval_baseline.py --context --data "$d" --out "$OUT/m_$n.json" "$M" | tail -1
done
# 品質ごとの設定は rewriter/lm_rewriter.cc の QualityOf と同じ(生成の数、LLM で採点する数)。
for q in "Low KOTORI_LM_BEAMS=2 KOTORI_LM_LLM_TOP=0" "Standard KOTORI_LM_BEAMS=4 KOTORI_LM_LLM_TOP=4" \
         "High KOTORI_LM_BEAMS=8 KOTORI_LM_LLM_TOP=32"; do
  set -- $q; name=$1; shift
  for d in $SETS; do
    n=$(basename "$d" .json)
    r=$(env KOTORI_RUNTIME_DIR="$I" KOTORI_MODEL_DIR="$I" "$@" \
          python mozc/eval_baseline.py --context --data "$d" --out "$OUT/k_$n.json" "$M" | tail -1)
    echo "$name $n: $r"
    python - "$OUT/m_$n.json" "$OUT/k_$n.json" <<'EOF'
import json, sys
m = json.load(open(sys.argv[1], encoding="utf-8"))
k = json.load(open(sys.argv[2], encoding="utf-8"))
for a, b in zip(m, k):
    if a["ok"] and not b["ok"]:
        print("    悪化:", a["top1"][:36], "->", b["top1"][:36])
EOF
  done
done
