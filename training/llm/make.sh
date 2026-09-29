#!/usr/bin/env bash
# 同梱する LLM(TinySwallow-1.5B、Apache-2.0)を GGUF の Q5_K_M にする(docs/adr/0016)。
# モデルはコミットしない。出力は target/kotori/tinyswallow-1.5b-q5_k_m.gguf。
#
# 使い方: bash training/llm/make.sh <llama-quantize の実行ファイル>
#   llama-quantize は llama.cpp の公式ビルド(Windows なら llama-b11259-bin-win-cpu-x64.zip)か、
#   third_party/llama.cpp を CMake でビルドしたもの。
#
# 注意: convert_hf_to_gguf.py の --outtype q8_0 で直接量子化した GGUF は、この LLM では壊れる
# (対数確率がおかしくなる)。必ず f16 にしてから llama-quantize で量子化する。
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$here/../.."
quantize="${1:?llama-quantize のパスを渡す}"
python="${PYTHON:-python3}"

REPO="SakanaAI/TinySwallow-1.5B"
REVISION="535d1322615c9f8171c3d60b10ef6955cc896fe9"
out="$root/target/kotori"
mkdir -p "$out"

src="$("$python" -c "
from huggingface_hub import snapshot_download
print(snapshot_download('$REPO', revision='$REVISION', allow_patterns=['*.json', '*.safetensors', '*.txt']))
")"
"$python" "$root/third_party/llama.cpp/convert_hf_to_gguf.py" "$src" \
  --outfile "$out/tinyswallow-1.5b-f16.gguf" --outtype f16
"$quantize" "$out/tinyswallow-1.5b-f16.gguf" "$out/tinyswallow-1.5b-q5_k_m.gguf" Q5_K_M
rm -f "$out/tinyswallow-1.5b-f16.gguf"
echo "作った: $out/tinyswallow-1.5b-q5_k_m.gguf" >&2
