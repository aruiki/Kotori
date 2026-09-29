# 16: パイプの ACL を AppContainer と低整合性に広げ、接続元を検証する

- 仕様: REQ-10-4、15 章(IPC 乗っ取りへの対策)
- 前提: なし
- 規模の目安: 250 行

## 目的
今のサーバーのパイプは、保護付き DACL で同じユーザーだけを許している(`kotori-client/src/windows.rs` の
`user_only_descriptor`)。ストアアプリ(AppContainer)と低整合性のプロセスからはつながらない。
REQ-10-4 のとおりにする。renderer のパイプ(カード 13)でも同じ規則を使う。

## 読むもの
- `crates/kotori-client/src/windows.rs`(`user_only_descriptor`、パイプの作り方)
- `crates/kotori-server/src/lib.rs` の `listen_pipe`
- SPEC REQ-10-4 と 15 章の表

## 手順
1. セキュリティ記述子を `D:P(A;;GA;;;<ユーザーSID>)(A;;GRGW;;;AC)S:(ML;;NW;;;LW)` にする
   (「すべてのアプリケーションパッケージ」に読み書き、低整合性のラベル)。SDDL を組み立てる関数を
   純粋関数にして、Linux でも文字列をテストする。
2. サーバーは接続を受けたら `GetNamedPipeClientProcessId` → `OpenProcess` → `OpenProcessToken` で
   接続元のユーザー SID を取り、自分と違えば切る。同じ処理を関数にして renderer でも使えるようにする。
3. 変えた理由と SDDL の意味を `docs/adr/0011-pipe-acl.md` に書く。

## テスト
- SDDL の文字列を組み立てる関数の単体テスト。
- Windows の CI: 同じユーザーのクライアントはつながる(既存のテスト)。低整合性のトークンで
  つなぐテストは、書ければ足す(難しければ PR に手で確かめた方法を書く)。

## 完了条件
- [ ] `just ci` と `just check` が通る
- [ ] CI の windows ジョブが緑
- [ ] 実機のストアアプリ(メモ帳の新しい版など)で入力できる(PR に結果を書く)

## 注意
- ACL を広げるのは読み書きだけにする。パイプのインスタンスを作る権限(FILE_CREATE_PIPE_INSTANCE)は
  与えない(乗っ取り対策)。
