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
