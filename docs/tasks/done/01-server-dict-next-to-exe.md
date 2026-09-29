# 01: サーバーの辞書を実行ファイルの隣から探す

- 仕様: 12.1、13.2(起動)
- 前提: なし
- 規模の目安: 60 行

## 目的
`kotori-server` が `--dict` なしで起動されたとき、まず自分の実行ファイルと同じフォルダの
`data\system.dict` を探す。今は `%ProgramFiles%` の環境変数から場所を決めているため、32 bit の
アプリから起動されると `Program Files (x86)` を見てしまう。

## 読むもの
- `crates/kotori-server/src/lib.rs` の `default_dict_path`
- `crates/kotori-server/src/main.rs` の `load_engine_in_background`

## 手順
1. `lib.rs` に `pub fn dict_candidates(exe: Option<&Path>) -> Vec<PathBuf>` を足す。
   順に「`exe` の親フォルダ/`data`/`system.dict`」、今の `default_dict_path()` の結果を返す。
2. `main.rs` で `--dict` がないとき、`std::env::current_exe().ok()` を渡して候補を作り、
   最初に存在するものを使う。どれもなければ今と同じく警告を出してキーを渡す。
3. `frontends/windows/README.md` の配置の説明を、`data` フォルダを exe の隣に置く形に直す
   (今の配置と同じ場所なので、文だけ整える)。

## テスト
`crates/kotori-server/tests/engine.rs` か `lib.rs` のテストに `dict_candidates_prefers_exe_dir` を
足し、`Some(Path::new("/opt/kotori/kotori-server"))` のとき先頭が
`/opt/kotori/data/system.dict` になることを確かめる(OS ごとの区切りは `Path::join` で作って比べる)。

## 完了条件
- [ ] `just ci` と `just check` が通る
- [ ] 実行ファイルの隣に `data/system.dict` を置くと `--dict` なしで読み込む(手で確かめ、PR に書く)
