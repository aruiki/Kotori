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
