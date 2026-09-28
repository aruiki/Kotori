# 0006: llama.cpp の取り込みと FFI の形

- 状態: 採用
- 日付: 2026-09-28

## 背景

6.4 は llama.cpp をコミット固定の git submodule として静的リンクし、Rust から FFI で呼ぶよう
求める。17.2 は `unsafe` を `kotori-lm` の FFI 境界に限る。llama.cpp の API は頻繁に変わり、
`llama_model_params` などの構造体を値で受け渡す。

## 決定

- `third_party/llama.cpp` に submodule として置き、タグ v0.5.0(`7fe450e1`)に固定する。
  更新は精度・速度の回帰(14 章)を確かめる PR で行う。
- `kotori-lm` の build.rs で `cmake` クレートを使い、Release・静的ライブラリ・OpenMP なし・
  `GGML_NATIVE=OFF`(実行機の命令セットに依存させない)で `llama` と `ggml*` を作る。
  ツール・テスト・サーバー・OpenSSL は作らない。
- Rust から llama.cpp の構造体の配置を写すと、上流の変更で黙って壊れる。そこで C のシム
  (`csrc/shim.c`)を挟み、ポインタと整数だけを受け渡す関数に包む。バインディング生成
  (bindgen)は libclang が要るので使わない。
- `unsafe` は `kotori-lm` の `ffi` モジュールだけに置き、各ブロックに `// SAFETY:` を書く。
- テストは llama.cpp 同梱の語彙だけの GGUF(`models/ggml-vocab-*.gguf`)と期待値で、
  トークン化が本家と一致することを確かめる。重みを含むモデルは CI に置かない。
- `KOTORI_LM_NO_NATIVE=1` のとき build.rs は何も作らない。Linux 上で windows-gnu 向けの
  clippy を回すときなど、リンクしない検査のためだけに使う。

## 影響

- CI の checkout で submodule を取得する。初回のビルドが数分延びる(以後は rust-cache)。
- クラウドセッションの起動フックで submodule を初期化する。
