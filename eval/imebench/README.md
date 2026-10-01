# 実際の IME で測る(Google 日本語入力・Microsoft IME・Kotori)

Google 日本語入力(製品版)と Microsoft IME には変換だけを呼ぶ方法がないので、実際に IME で入力して測る。
Kotori も同じ方法で測り、同じ条件で比べる。

## しくみ

`ImeBench.exe`(`ImeBench.cs`)がテキストボックスの窓を開き、指定の IME をこのプロセスだけで有効にして
(`ITfInputProcessorProfileMgr::ActivateProfile`)、読みをローマ字のキーとして送る。Space で変換し、
Enter で確定した文字を読む。

- 前の文は渡さない(どの IME にも同じ条件。Kotori の「前の文あり」の数値とは比べない)。
- Space 1 回の第 1 候補(文節の区切りも IME に任せる)。
- 全角と半角の違い(数字・記号)は NFKC で揃えてから正解と比べる(IME の設定の違いで差が出ないように)。
- 読みに打てない文字(『』、英字など)がある問は除く(AJIMEE は 200 問中 2 問)。
- 最初の 2 問は慣らし(採点しない)。確定した文字が空の問は 1 回だけやり直す(キーの取りこぼし)。
- 問ごとにフォーカスを移して戻し、前の問の確定を文脈として持ち越さないようにする。
- IME の学習の影響を減らすため、各 IME の学習は既定のまま、1 問ごとにテキストボックスを空にする。

## 使い方

```bash
# 1 回だけ: C# のコンパイラ(.NET Framework に付属)で作る
/c/Windows/Microsoft.NET/Framework64/v4.0.30319/csc.exe -nologo -target:winexe -out:eval/imebench/ImeBench.exe eval/imebench/ImeBench.cs
python eval/imebench/imebench.py google     # Google 日本語入力
python eval/imebench/imebench.py msime      # Microsoft IME
python eval/imebench/imebench.py kotori     # Kotori(インストール済みのもの)
```

既定は AJIMEE-Bench と日常の文(`eval/sets/kotori-daily.json`)。1 つの IME で約 12 分かかる。**計測の間は
キーボードとマウスを使わない**(キー入力を送るため)。結果は `eval/imebench/out/` に問ごとに残る。

## 結果(2026-10-01、RTX 3060、前の文なし)

| | AJIMEE-Bench(198) | 日常(81) | 同音語(40) | 人名・新語(40) | 打ち間違い(40) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Kotori日本語入力 beta.8(Unreal) | 88.4% | 97.5% | 97.5% | 97.5% | 5.0% |
| Microsoft IME(Windows 11) | 59.6% | 86.4% | 82.5% | 82.5% | 5.0% |
| Google 日本語入力 3.34.6260 | 58.1% | 81.5% | 65.0% | 95.0% | 10.0% |

参考: 変換器を直接呼んだ Standard(前の文なし)は AJIMEE 86.0%、日常 97.5%。Mozc 単体は 51.0%、80.2%。
