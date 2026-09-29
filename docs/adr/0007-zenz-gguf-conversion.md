# 0007: zenz-v2.5 の GGUF への変換

- 状態: 採用
- 日付: 2026-09-29

## 背景

6.1 はリランクの基準モデルを zenz-v2.5 とし、6.4 は GGUF のメタデータに `kotori.model_version`・
`kotori.vocab_hash`・`kotori.score_weights` を入れるよう求める。zenz-v2.5 は Hugging Face に
safetensors だけで公開されていて、GGUF は配られていない。またその BPE 前処理
(ku-nlp/gpt2-small-japanese-char)は上流の llama.cpp に登録がなく、同梱の
`convert_hf_to_gguf.py` はそのままでは変換を拒む。

## 決定

- モデルは `training/zenz/fetch.sh` がリビジョン `1e408d69` に固定して取得し、
  `training/zenz/zenz-v2.5-small.sha256` で照合する。モデルはリポジトリに置かない(17.2)。
- 変換は `training/zenz/convert.py` が llama.cpp 同梱の変換器(submodule と同じコミット)を
  呼んで行う。llama.cpp 本体は変更しない。
- 前処理は、Zenzai が使う llama.cpp のフォークでの実装が GPT-2 と同じ正規表現なので、
  `gpt-2` として書き出す。取り違えを防ぐため、vocab.json の SHA-256 が固定した zenz-v2.5 の
  ものと一致するときだけそうする。
- config.json の特殊トークン ID は元モデル(ku-nlp)のままで、zenz の語彙と合っていない
  (`eos_token_id=2` は `<s>`)。transformers でモデルが終端に出すのは `</s>`(3)なので、
  tokenizer_config.json のトークン名から ID を引き直す(bos 2、eos 3、pad 1、unk 0)。
  BOS はプロンプトに付けない(tokenizer_config.json の `add_bos_token=false`)。
- `kotori.vocab_hash` は、ID 順のトークン文字列を改行でつないだ UTF-8 の SHA-256 とする。
  エンジンは GGUF の語彙から同じ値を計算して照合する。
- `kotori.score_weights` は 6.2 の重みの JSON。調整前の既定値を入れ、M2 で開発用データにより
  最適化したら変換し直す。
- `kotori.license` にライセンス(CC-BY-SA-4.0)と帰属を書く。

## 影響

- 変換結果は `KOTORI_ZENZ_GGUF=<gguf> cargo test -p kotori-lm -- --ignored` で確かめる。
  トークン列と候補の対数確率が transformers と一致すること(f16 の誤差 0.1 以内)を見る。
- 変換には Python と `third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt`
  の依存(PyTorch を含む)が要る。CI では変換しない。
- zenz-v2.5 は CC-BY-SA-4.0 なので、変換した GGUF を配布するときは同じライセンスと帰属表示が
  要る。配布の扱いはメンテナの判断を待ち、それまでは利用者が手元で取得・変換する。
