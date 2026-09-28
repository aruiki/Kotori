# 辞書ソース

システム辞書(docs/SPEC.md 5.3)の元データを取得する。生データはコミットせず、
`fetch.sh` が固定したコミットから取得して SHA-256 を検証する(17.2)。

```sh
just fetch-dict   # data/dict-src/mozc/ に取得する
```

| ファイル | 中身 |
| --- | --- |
| `fetch.sh` | 取得と検証。正しいハッシュのファイルが既にあれば取得しない |
| `mozc.sha256` | 取得するファイルと SHA-256 の一覧 |

## Mozc dictionary_oss

- 取得元: `google/mozc` の `src/data/dictionary_oss`(コミットは `fetch.sh` の `MOZC_COMMIT`)
- `dictionaryNN.txt`: `読み \t 左文脈ID \t 右文脈ID \t コスト \t 表記`
- `id.def`: 品詞 ID と品詞名
- `connection_single_column.txt`: 1 行目が品詞数 N、以降 N×N 個の連接コスト(左の右文脈 ID が行)
- `suffix.txt`: 接尾辞の語彙
- ライセンス: IPAdic 由来(NAIST)と沖縄辞書を含む。条文は `README.txt` にあり、
  生成した辞書を配布するときはこれを同梱する(16 章)

補助語彙(SudachiDict)、固有名詞、絵文字、郵便番号は M1 の範囲外で、後から加える。
