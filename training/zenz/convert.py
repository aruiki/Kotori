#!/usr/bin/env python3
"""zenz-v2.5(Hugging Face の safetensors)を Kotori 用の GGUF にする(docs/SPEC.md 6.4、docs/adr/0007)。

使い方:
    python training/zenz/convert.py <fetch.sh の出力ディレクトリ> <出力 .gguf> [--outtype f16]

依存は third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt で固定されている。
llama.cpp 本体は変更せず、次の2点だけをこのスクリプトで補う。

1. zenz の BPE 前処理(ku-nlp/gpt2-small-japanese-char)は上流の llama.cpp に登録がない。
   Zenzai が使うフォークでの実装は GPT-2 と同じ正規表現なので、`gpt-2` として書き出す。
   取り違えを防ぐため、vocab.json が固定した zenz-v2.5 のものと一致するときだけそうする。
2. config.json の特殊トークン ID は元モデルのままで、語彙と合っていない(eos_token_id=2 は
   `<s>`)。学習で使われた終端は `</s>`(3)なので、tokenizer_config.json のトークン名から
   ID を引き直す。
3. 6.4 が必須とするメタデータ `kotori.*` を書き込む。
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LLAMA = ROOT / "third_party" / "llama.cpp"
sys.path.insert(0, str(LLAMA / "gguf-py"))
sys.path.insert(0, str(LLAMA))

import convert_hf_to_gguf  # noqa: E402
import gguf  # noqa: E402
from conversion import base  # noqa: E402

# zenz-v2.5-small のリビジョン 1e408d69 の vocab.json(training/zenz/zenz-v2.5-small.sha256)。
ZENZ_VOCAB_SHA256 = "67fd752abb091e649a9bb08bcb2b52b27cd6e3893ff5ee6e391e5555c0a90e0f"
# fetch.sh が固定したリビジョン。出力の kotori.model_version に書く。
REVISIONS = {
    "zenz-v2.5-xsmall": "9bfb00e795f89284fe9164f0fbde171d046fc546",
    "zenz-v2.5-small": "1e408d69a7e284efa4e4d63e456f50e363a82953",
    "zenz-v2.5-medium": "623bc8edf5417129b8269b628223d08245e9c68e",
}
LICENSE = (
    "CC-BY-SA-4.0; zenz-v2.5 (c) Keita Miwa, "
    "based on ku-nlp/gpt2-small-japanese-char (CC-BY-SA-4.0)"
)
# 6.2 のスコア統合の重み。調整前の既定値(M2 で開発用データにより最適化する)。
SCORE_WEIGHTS = {"lambda_lm": 1.0, "lambda_lattice": 1.0, "temperature": 1000.0, "lambda_user": 0.0}


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def vocab_hash(model_dir: Path) -> str:
    """ID 順のトークン文字列を改行でつないだものの SHA-256。エンジン側と照合する(6.4)。"""
    vocab = json.loads((model_dir / "vocab.json").read_text(encoding="utf-8"))
    tokens = [t for t, _ in sorted(vocab.items(), key=lambda kv: kv[1])]
    return hashlib.sha256("\n".join(tokens).encode("utf-8")).hexdigest()


def special_token_ids(model_dir: Path) -> dict[str, int]:
    """tokenizer_config.json の特殊トークン名を vocab.json で ID にする。"""
    vocab = json.loads((model_dir / "vocab.json").read_text(encoding="utf-8"))
    config = json.loads((model_dir / "tokenizer_config.json").read_text(encoding="utf-8"))
    ids = {}
    for typ in ("bos", "eos", "unk", "pad"):
        token = config[f"{typ}_token"]
        ids[typ] = vocab[token["content"] if isinstance(token, dict) else token]
    return ids


def main() -> None:
    model_dir = Path(sys.argv[1])
    if sha256_file(model_dir / "vocab.json") != ZENZ_VOCAB_SHA256:
        sys.exit("vocab.json が固定した zenz-v2.5 のものと一致しない")
    special = special_token_ids(model_dir)

    original_load = gguf.SpecialVocab._load

    def load_special(self, path: Path) -> None:
        original_load(self, path)
        self.special_token_ids.update(special)

    gguf.SpecialVocab._load = load_special

    original_pre = base.TextModel.get_vocab_base_pre

    def get_vocab_base_pre(self, tokenizer) -> str:
        try:
            return original_pre(self, tokenizer)
        except NotImplementedError:
            return "gpt-2"

    # GPT2Model は set_gguf_parameters を上書きしているので、その呼び出し元に差し込む。
    original_prepare = base.TextModel.prepare_metadata

    def prepare_metadata(self, vocab_only: bool) -> None:
        original_prepare(self, vocab_only)
        w = self.gguf_writer
        name = model_dir.name
        w.add_string("kotori.model_version", f"{name}@{REVISIONS.get(name, 'unknown')}")
        w.add_string("kotori.vocab_hash", vocab_hash(model_dir))
        w.add_string("kotori.score_weights", json.dumps(SCORE_WEIGHTS, sort_keys=True))
        w.add_string("kotori.license", LICENSE)

    base.TextModel.get_vocab_base_pre = get_vocab_base_pre
    base.TextModel.prepare_metadata = prepare_metadata

    sys.argv = ["convert_hf_to_gguf.py", str(model_dir), "--outfile", sys.argv[2], *sys.argv[3:]]
    convert_hf_to_gguf.main()


if __name__ == "__main__":
    main()
