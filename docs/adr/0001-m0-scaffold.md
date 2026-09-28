# 0001: M0 の土台を2つのPRに分ける

- 状態: 採用
- 日付: 2026-09-28

## 背景

SPEC 18.1 の M0 は、ワークスペースと空クレート、`AGENTS.md`、`docs/SPEC.md`、`justfile`、CI、IPC の `.proto` と疎通テストを含む。1PRあたり原則600行以下(17.4-3)に収めるには分割が要る。

## 決定

1. 1本目: ワークスペース、空クレート16個、`AGENTS.md`、`docs/SPEC.md`、`justfile`、GitHub Actions(ubuntu、windows、MSRV)。
2. 2本目: `kotori-proto` の `.proto` 定義と、サーバー・クライアントの疎通テスト(4.2)。

`docs/SPEC.md` とライセンス本文は文書のため600行の対象外とする。

## その他の判断

- `unsafe_code` はワークスペースで deny とし、`kotori-lm` と `kotori-client` だけが後で allow する(17.2)。
- `kotori-config` は Tauri 2 を使う予定だが、M0 では依存を入れず空のバイナリにする。
