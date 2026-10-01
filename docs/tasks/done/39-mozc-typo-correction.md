# 39. 打ち間違いを補って変換する(ATOK に近づける 1)

## 目的
ATOK が得意とされる「打ち間違いの補正」を、ほかの精度を下げずに入れる。

## 読むもの
- docs/adr/0016、0024、0036
- `eval/imebench/README.md`(ほかの IME との比較)、`eval/sets/make_atok_sets.py`

## やること
- 読みを 1 か所書き換えた候補を作り、辞書のコストで絞り、zenz で生成、zenz + LLM で比べる。
- 選んだ文が不自然なときだけ試す(Space を遅くしない)。

## 完了条件
- [x] 打ち間違い 40 問: 2 → 14(Google 4、MS-IME 2)
- [x] 調整用のセット 8 つと heldout が下がらない
- [x] 負荷試験・単体テスト、Space の時間
- [x] docs/adr/0036

## 注意
- 文字の削除は文が短くなるだけで点が上がる。重複と余分な っ・ん に限る。
- `eval_baseline.py --stderr` で変換器の stderr(`[typo]` の行)を残せる。
