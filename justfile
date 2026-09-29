# Kotori のタスクランナー(docs/SPEC.md 17.1)

default:
    @just --list

# PR 前に通す全ゲート(17.4-4)。ゴールデン・縮小評価・ベンチは M1 以降で追加する
ci: fmt-check clippy test

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets --locked -- -D warnings

test:
    cargo test --workspace --locked

# 辞書ソース(Mozc dictionary_oss)を取得し、SHA-256 を検証する(5.3)
fetch-dict:
    bash data/dict-src/fetch.sh

# システム辞書を作る(5.3、REQ-5-5)。出力は target/kotori/system.dict
dict: fetch-dict
    cargo run --release --locked -p kotori-dictc -- data/dict-src/mozc target/kotori/system.dict

# 読みを入れて変換結果と候補を見る(M1)。先に just dict で辞書を作る
repl:
    cargo run --release --locked -p kotori-eval -- repl target/kotori/system.dict

# 評価セットを取得して一括評価する(14.1、REQ-14-1)。結果は eval/results/ に JSON と Markdown で出る
eval: dict
    bash eval/fetch.sh
    cargo run --release --locked -p kotori-eval -- run target/kotori/system.dict eval/results ajimee-bench=eval/data/ajimee-bench.json

# 上位候補を zenz-v2.5-small でリランクして評価する(6.2)。先に just zenz でモデルを用意する
eval-lm: dict
    bash eval/fetch.sh
    cargo run --release --locked -p kotori-eval -- run --lm target/kotori/zenz-v2.5-small-f16.gguf target/kotori/system.dict eval/results-lm ajimee-bench=eval/data/ajimee-bench.json

# REQ-6-1 の条件(読み 20 文字、左文脈 64 文字、K=16)でリランクの遅延を測る(13.2)。先に just zenz でモデルを用意する
bench-lm: dict
    bash eval/fetch.sh
    cargo run --release --locked -p kotori-eval -- bench-lm target/kotori/system.dict target/kotori/zenz-v2.5-small-f16.gguf eval/data/ajimee-bench.json

# zenz-v2.5-small を取得して GGUF に変換する(6.1、6.4、docs/adr/0007)。出力は target/kotori/zenz-v2.5-small-f16.gguf。
# 先に third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt を pip で入れておく
zenz python="python3":
    bash training/zenz/fetch.sh
    mkdir -p target/kotori
    {{python}} training/zenz/convert.py training/zenz/models/zenz-v2.5-small target/kotori/zenz-v2.5-small-f16.gguf --outtype f16
