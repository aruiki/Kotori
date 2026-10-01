# 40: リリースの段階(beta / rc / 正式)を選んで出せるようにする

- 仕様: SPEC 16.3(版と更新)、docs/adr/0037 の「段階」
- 前提: なし
- 規模の目安: ワークフロー 40 行、`check_release.py` 30 行、文書 30 行

## 目的
今のワークフローは常に `<mozc/VERSION>-beta.N` を付けて Latest にする。`1.0.0-rc.N`(プレリリース)と
`1.0.0`(正式版)も同じ手順で出せるようにし、MSI が GitHub の上限(1 ファイル 2 GiB)に近づいたら気づけるようにする。

## 読むもの
- `.github/workflows/mozc-windows.yml` の「版とリリースの名前を決める」と `softprops/action-gh-release`
- `mozc/tools/check_release.py`、`docs/DEVELOPMENT.md` の 1.5

## 手順
1. `workflow_dispatch` に入力 `channel`(`beta` / `rc` / `stable`、既定 `beta`)を足す。
   - `beta`・`rc`: 既存のタグ `v<VERSION>-<channel>.N` の次の番号。`stable`: `v<VERSION>`(既にあれば失敗させる)。
   - `rc` は `prerelease: true`、`make_latest: false`。`beta` と `stable` は今と同じ(Latest)。
   - 正式版を出した後に同じ `VERSION` で beta・rc を出そうとしたら失敗させる(版が戻るため)。
2. 本文は `mozc/release-notes.md` のまま。正式版で「ベータ版、未署名」の文が残っていたら失敗させる。
3. `check_release.py` に、MSI の大きさが 1.8 GiB を超えたら NG を足す(上限 2 GiB の手前で気づく)。
4. `docs/DEVELOPMENT.md` の 1.5 に、段階の選び方(`gh workflow run "Mozc (Windows)" --ref main -f channel=rc`)を書く。
5. `mozc/VERSION` を `1.0.0` に上げるのは RC を出す PR で行う(このカードでは上げない)。

## テスト
- ワークフローの版の決め方を `mozc/tools/release_tag.py`(関数 1 つ)に移し、タグの一覧から次の版を出す単体テスト
  (`python -m unittest mozc/tools/test_release_tag.py`)で、beta・rc・stable、既存の正式版がある場合を確かめる。

## 完了条件
- [ ] 単体テストが通り、PR の CI が緑
- [ ] 次のベータのリリースで、版が `0.3.0-beta.9` になる(今の付け方を壊していない)

## 注意
- 積み重ねた PR とマージの注意は `AGENTS.md`。ワークフローの変更は PR では MSI のビルドが走る(45 分)。
- MSI の ProductVersion(`version.bzl` の BUILD)は今のまま実行番号で上げる。段階とは関係ない。
