# 31: Unreal を見直す(High を上回らない)

- 仕様: docs/adr/0024(Unreal = High + zenz-medium、AJIMEE 92.0% で High 92.5% を上回らない)
- 前提: なし
- 規模の目安: パッチ 30 行、ADR

## 目的
最高設定の Unreal が High より良くならないので、中身を変えるか、選択肢から外して設定を分かりやすくする。

## 手順
1. medium と small の両方で生成し、候補を合わせて採点する案、medium を採点にだけ使う案などを、調整用のセット
   (AJIMEE・日常・慣用句・ニュアンス)で比べる。
2. どれも High を上回らなければ、Unreal を設定画面から外し、既存の設定は High として扱う(設定の互換を保つ)。

## 完了条件
- [ ] Unreal が High を上回る、または Unreal を外す(ADR に数値と理由)
- [ ] CI(Mozc (Windows))が緑
