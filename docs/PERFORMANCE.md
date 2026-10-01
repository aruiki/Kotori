# 性能表(機器 × 品質)

docs/IMPROVEMENT_PROPOSALS.md の案 09。`mozc/tools/perf_table.py` で測る。変換器(`converter_main`)に日常の文
40 問を 1 問ずつ変換させ、打ってから待たずに変換したときの時間を測る(実際に打つときは、GPU のない PC では
先回りの変換で短くなる。docs/adr/0024 の 4 章)。VRAM は nvidia-smi の全体の使用量の増分。

おすすめの品質は VRAM の量だけでなく、この表の実測で決める。ほかの機器(内蔵 GPU、VRAM の少ない GPU、AMD の
GPU、遅い CPU)の数値は、測れたら行を足す(測る人は機器・ドライバー・メモリを書く)。

## 2026-09-30: AMD Ryzen 7 5700X(8 コア 16 スレッド)、メモリ 32 GB、RTX 3060 12 GB(ドライバー 616.92)

AI の計算は 4 スレッド。docs/adr/0024〜0027 の後。

| 機器 | 品質 | 最初の変換(読み込みを含む) | 変換 中央値 / p95 / 最大 | メモリ(最大) | VRAM の増分 |
| --- | --- | ---: | --- | ---: | ---: |
| GPU | Low | 395 ms | 65 / 88 / 89 ms | 147 MB | 548 MB |
| GPU | Standard | 816 ms | 65 / 87 / 96 ms | 488 MB | 1655 MB |
| GPU | High | 809 ms | 79 / 104 / 105 ms | 566 MB | 1641 MB |
| CPU | Low | 481 ms | 287 / 428 / 439 ms | 596 MB | - |

- GPU の Low は zenz-v2.5-medium だけを GPU に載せる(VRAM 548 MB)。Standard 以上は TinySwallow-1.5B も
  載せる(約 1.6 GB)。
- CPU の Low の Space は、先回りの変換が間に合うと中央値 170 ms、打ってすぐ Space で中央値 397 ms・最大 613 ms
  (`mozc/tools/space_latency.py`、docs/adr/0024)。

## 2026-10-01: 同じ PC、1 回の計算のまとまりを 256 に(docs/adr/0032)

| 機器 | 品質 | 最初の変換(読み込みを含む) | 変換 中央値 / p95 / 最大 | メモリ(最大) | VRAM の増分 |
| --- | --- | ---: | --- | ---: | ---: |
| GPU | Low | 385 ms | 62 / 83 / 91 ms | 133 MB | 524 MB |
| GPU | Standard | 730 ms | 62 / 84 / 87 ms | 474 MB | 1490 MB |
| GPU | High | 738 ms | 76 / 102 / 106 ms | 559 MB | 1483 MB |

## 2026-10-01: 同じ PC、生成の前置きの共有(docs/adr/0033)

入力中の AI の稼働率(`mozc/tools/cost_bench.py`、Standard): 47.1% → 42.6%。予測の当たりと変換の精度は同じ。

## 2026-10-01: 同じ PC の CPU、長い読みに zenz-v2.5-small(docs/adr/0035)

`KOTORI_LM_DEVICE=cpu`。AJIMEE(前の文あり)67.0% → 83.0%、日常 97.5%(同じ)。打ってすぐ Space(AJIMEE 60 文)
正解 36 → 46、中央値 380 → 162 ms、最大 717 → 415 ms。日常 15 文は中央値 286 → 118 ms。メモリ(プライベート)282 → 391 MB。
