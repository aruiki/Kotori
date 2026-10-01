<p align="center">
  <img src="docs/images/hero.png" alt="Kotori日本語入力 — Mozc の安定した変換に、文脈を読む AI を。AJIMEE-Bench 91.5%、日常の文 97.5%、変換 1 回 0.07 秒、ネット通信 0" width="880">
</p>

<p align="center">
  <a href="https://github.com/aruiki/KotoriIME-japanese-/releases/latest"><img alt="最新版" src="https://img.shields.io/github/v/release/aruiki/KotoriIME-japanese-?include_prereleases&label=%E6%9C%80%E6%96%B0%E7%89%88&color=242C5C"></a>
  <a href="https://github.com/aruiki/KotoriIME-japanese-/releases"><img alt="ダウンロード数" src="https://img.shields.io/github/downloads/aruiki/KotoriIME-japanese-/total?label=%E3%83%80%E3%82%A6%E3%83%B3%E3%83%AD%E3%83%BC%E3%83%89&color=242C5C"></a>
  <img alt="Windows 10 / 11" src="https://img.shields.io/badge/Windows-10%20%2F%2011-242C5C?logo=windows">
  <img alt="オフラインで動作" src="https://img.shields.io/badge/%E3%82%AA%E3%83%95%E3%83%A9%E3%82%A4%E3%83%B3-%E9%80%9A%E4%BF%A1%E3%81%97%E3%81%AA%E3%81%84-F25C2E">
  <img alt="AJIMEE-Bench 91.5%" src="https://img.shields.io/badge/AJIMEE--Bench-91.5%25-F25C2E">
  <a href="#ライセンス"><img alt="ライセンス" src="https://img.shields.io/badge/license-Apache--2.0%20%2F%20MIT-5B6070"></a>
</p>

<h3 align="center">
  <a href="https://github.com/aruiki/KotoriIME-japanese-/releases/latest">⬇ ダウンロード(Windows 用 MSI)</a>
</h3>

<p align="center">
  <a href="#変換の例">変換の例</a> ・
  <a href="#変換の正確さ">正確さ</a> ・
  <a href="#入力中の-ai-予測">AI 予測</a> ・
  <a href="#軽さ">軽さ</a> ・
  <a href="#しくみ">しくみ</a> ・
  <a href="#必要な環境">必要な環境</a> ・
  <a href="#インストール">インストール</a> ・
  <a href="#よくある質問">よくある質問</a>
</p>

---

**Kotori日本語入力**は、Windows 用の日本語入力(IME)です。Google 日本語入力のオープンソース版である
**Mozc** の変換エンジンと辞書をそのまま土台にし、その上で **AI が前後の文脈を読んで候補を選び直します**。
使い慣れた Mozc の操作はそのままに、同音異義語や言い回しの取り違えを減らします。

> [!TIP]
> MSI をひとつ実行するだけで使えます。AI のモデルと GPU の実行環境も同梱しているので、CUDA などの準備は要りません。
> GPU がない PC(ノート PC など)でも動きます。

<p align="center"><img src="docs/images/features.png" alt="特長: 文脈で選ぶ、先回りの予測、待たせない、外に出さない、GPU に優しい、MSI ひとつ" width="880"></p>

## 変換の例

<p align="center"><img src="docs/images/examples.png" alt="変換の例: はしのはしをはしでつまむ → 箸の箸を箸でつまむ、こうえんをした → 公演をした、せいどがあがった → 精度が上がった、じゅうじから → 10時から、このやく → この訳、きものをぬいで → 履物を脱いで" width="880"></p>

<details>
<summary>表で見る(コピーできる形)</summary>

実際の出力です(Standard、2026-10-01)。「前の文」は、直前に確定した文として AI に渡したものです。

| 読み | 前の文 | Mozc 単体 | Kotori日本語入力 |
| --- | --- | --- | --- |
| はしのはしをはしでつまむ | | 橋の橋を橋でつまむ | **箸の箸を箸**でつまむ |
| かれはこうえんでこうえんをした | | 彼は公園で公園をした | 彼は公園で**公演**をした |
| おんせいにんしきのせいどがあがった | | 音声認識の制度が上がった | 音声認識の**精度**が上がった |
| あしたのかいぎはじゅうじからです | | 明日の会議は従事からです | 明日の会議は**１０時**からです |
| このやくはむずかしい | 英語の文を日本語にしています。 | この薬は難しい | この**訳**は難しい |
| いしをつたえる | 自分の | 医師を伝える | **意思**を伝える |
| ここではきものをぬいでください | 玄関で靴を脱いで。 | ここでは着物を脱いでください | **ここで履物**を脱いでください |

</details>

## 変換の正確さ

<p align="center"><img src="docs/images/accuracy.png" alt="AJIMEE-Bench の第 1 候補の正解率。Kotori日本語入力 Unreal 93.0%、High 92.5%、Standard 91.5%、Low 88.0%、GPU のない PC 83.0%、azooKey + Zenzai 85.0%、Mozc 51.0%。日常の文 81 問は Kotori 97.5%、Mozc 80.2%" width="880"></p>

- [AJIMEE-Bench](https://github.com/azooKey/AJIMEE-Bench) は、かな漢字変換の難しい 200 問(同音異義語、文脈で
  決まる語など)の評価セットです。1 番目の候補が正解と一致した割合を数えています。
- 調整に一度も使っていない **最終評価用のセット 300 問**(ニュース・ビジネス・日常・固有名詞・数字など)では、
  Mozc 単体 82.3% → **96.3%**(Standard)。過学習していないかの確認に使っています。
- 測り方と記録はすべて [eval/README.md](eval/README.md) にあります。Google 日本語入力(製品版)と
  Microsoft IME は自動で測る方法がないため、まだ比べていません。

## 入力中の AI 予測

入力している間、AI が文の続きを裏で考え、次の打鍵のときに候補の先頭に「AI」と付けて出します。
Tab で予測の候補に移って選べます。

| | Mozc 単体 | Kotori日本語入力 |
| --- | ---: | ---: |
| 決まった言い回し 29 問で、正しい続きが上位 3 件に出た割合 | 17.2% | **58.6%** |
| 1 打鍵の応答 | 1〜3 ms | 1〜3 ms(予測は裏で計算し、打鍵を待たせない) |
| 手を止めてから Tab | — | 約 0.04 秒 |

入力の邪魔になるだけの予測(脱線した文、読みを写しただけのカタカナ、1 文字だけの続き)は出しません。

## 軽さ

<p align="center"><img src="docs/images/lightness.png" alt="軽さ: 入力中の GPU の計算 68.6% → 42.6%、使っていないときの VRAM 1.66 GB → 0 GB、GPU のない PC の変換 380 ms → 162 ms" width="880"></p>

- **入力中の GPU の負荷**: 前の打鍵の計算を使い回し、まとめて計算することで、AI が GPU を使う時間を約 4 割減らしました。
- **使っていないとき**: 10 分入力がなければ AI のモデルを外し、VRAM を空けます(ゲームや動画の編集の邪魔をしない)。
  次に入力すると裏で読み込み直します。
- **固まらない**: 変換で AI に使う時間には上限があり、間に合わなければ Mozc の結果をそのまま出します。
  内蔵 GPU には AI を載せません(反応が遅く、入力が止まるため)。

## しくみ

<p align="center"><img src="docs/images/how.png" alt="しくみ: 読み → 候補を集める(Mozc の辞書と変換用 AI)→ AI が選ぶ(zenz と日本語 LLM)→ 結果" width="880"></p>

1. **候補を集める**: Mozc が辞書から文を組み立て、変換用の小さな AI(zenz)も読みから文を書きます。
2. **AI が選ぶ**: zenz と日本語の言語モデル(TinySwallow-1.5B)が、「前の文に続けて自然な日本語か」を
   それぞれ点数にします。点数の合計がいちばん高い文を選び、文節の区切りもそれに合わせます。
3. **辞書で確かめる**: AI が書いた文は、読みと合っているかを Mozc の辞書で確かめ、合わないものは捨てます。
   AI が読みにない語を足したり、読みを落としたりすることはありません。

AI が働くのは変換(Space)と予測のときだけで、1 文字ずつの入力はこれまでどおり Mozc が処理します。
ユーザー辞書に登録した語は、AI が書き換えません。詳しい設計は [docs/adr/](docs/adr/) にあります(0012 以降)。

```mermaid
flowchart LR
  K[キー入力] --> M[Mozc<br/>辞書・ラティス]
  M -->|文の候補| R{AI が選ぶ}
  Z[zenz<br/>変換用 AI] -->|生成した文| R
  R -->|採点| Z
  R -->|採点| L[TinySwallow-1.5B<br/>日本語 LLM]
  R --> O[候補ウィンドウ]
  K -. 裏で .-> P[AI 予測] -.-> O
```

## 設定

<p align="center"><img src="docs/images/settings.png" alt="設定画面の「AI 変換」タブ: 品質、AI の状態、入力中の AI 予測、AI モデル" width="880"></p>

## 必要な環境

| | 最低限 | おすすめ |
| --- | --- | --- |
| OS | Windows 10 1809 以降(64 bit) | Windows 11 |
| GPU | なくても動く(自動で Low) | 専用 GPU、Vulkan 対応、VRAM 3 GB 以上(NVIDIA / AMD / Intel Arc) |
| メモリ | 8 GB | 16 GB |
| ディスク | 1.7 GB | |

GPU は自動で見つけて使います。ドライバー以外の準備は要りません。内蔵 GPU(CPU に付いている GPU)は使わず、
その場合は CPU で動きます。動作を確かめた GPU は RTX 3060 です。

### 品質の段階

設定の「AI 変換」タブで選べます。数値は RTX 3060(GPU のない PC の行はデスクトップの CPU)で測りました。

| 品質 | AJIMEE-Bench | 変換 1 回(中央値) | VRAM | 向いている環境 |
| --- | ---: | ---: | ---: | --- |
| Low | 88.0% | 0.06 秒 | 約 0.5 GB | GPU が小さい |
| **Standard(既定)** | **91.5%** | **0.06 秒** | 約 1.5 GB | GPU あり |
| High | 92.5% | 0.08 秒 | 約 1.5 GB | 速い GPU |
| Unreal | 93.0% | 0.26 秒 | High より多い | とても速い GPU |
| (GPU のない PC) | 83.0% | 0.12〜0.16 秒 | — | ノート PC。どの品質を選んでも自動でこれ |

## インストール

1. [Releases](https://github.com/aruiki/KotoriIME-japanese-/releases/latest) から `Kotori64.msi` をダウンロードして実行します
   (管理者の確認で「はい」)。「Windows によって PC が保護されました」と出たら「詳細情報」→「実行」。
2. 入力方式に **Kotori日本語入力** が加わります。Windows キー + Space で切り替え、半角/全角キーでオン/オフ。
3. 設定はデスクトップの「Kotori日本語入力の設定」から開けます。

新しい版は、前の版の上にそのまま入れられます(v0.3.0-beta.6 以降)。それより前の版が入っているときは、
一度アンインストールしてから入れてください。アンインストールは「設定」→「アプリ」から行えます。

## 最近の更新

| 版 | 主な変更 |
| --- | --- |
| [v0.3.0-beta.8](https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v0.3.0-beta.8) | GPU のない PC で、長い文の変換が速く正確に(打ってすぐ Space の正解 36 → 46 / 60 文) |
| [v0.3.0-beta.7](https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v0.3.0-beta.7) | 10 分使わなければ VRAM を空ける。入力中の GPU の負荷をさらに約 1 割減 |
| [v0.3.0-beta.6](https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v0.3.0-beta.6) | 上書きインストール、予測が邪魔になりにくく、Unreal 93.0%、VRAM 約 170 MB 減 |
| [v0.3.0-beta.5](https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v0.3.0-beta.5) | ノート PC で固まらない、GPU のない PC 向けの Low、設定画面の不具合の修正 |

すべての版は [Releases](https://github.com/aruiki/KotoriIME-japanese-/releases) にあります。

## よくある質問

<details>
<summary><b>インターネットにつながりますか?</b></summary>

つながりません。辞書も AI も PC の中にあり、入力した文字を外に送りません。AI が覚えておく予測は
シークレットモード(プライベートな入力)では作らず、使わないときは捨てます。
</details>

<details>
<summary><b>Google 日本語入力や Mozc と何が違いますか?</b></summary>

操作と辞書は Mozc と同じです。その上で、変換の最後に AI が文脈を見て候補を選び直し、入力中に文の続きを
予測します。Kotori日本語入力は Google が提供・推奨するものではありません。
</details>

<details>
<summary><b>GPU がないと使えませんか?</b></summary>

使えます。GPU がない PC(内蔵 GPU だけの PC を含む)では、どの品質を選んでも自動で CPU 向けの設定になります
(AJIMEE-Bench 83.0%、日常の文 97.5%)。AI が時間内に終わらないときは Mozc の結果を出すので、入力が固まりません。
</details>

<details>
<summary><b>ゲーム中に VRAM を取られませんか?</b></summary>

日本語を 10 分入力しなければ、AI のモデルを外して VRAM を空けます。次に入力したときに裏で読み込み直します
(その最初の 1 回だけ、AI なしの変換になることがあります)。
</details>

<details>
<summary><b>AI の状態はどこで見られますか?</b></summary>

設定の「AI 変換」タブに、使っている GPU と、実際に動いている AI(読み込み中・休んでいる・動いていない理由)が出ます。
</details>

<details>
<summary><b>学習やユーザー辞書は使えますか?</b></summary>

Mozc の学習とユーザー辞書がそのまま使えます。ユーザー辞書に登録した語は、AI が書き換えません。
</details>

## 開発者向け

- 現状と次の作業: [docs/HANDOFF.md](docs/HANDOFF.md)、作業カード: [docs/tasks/](docs/tasks/)
- Mozc への変更とビルド方法: [mozc/README.md](mozc/README.md)、[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)(Bazel、パッチ 3 本)
- 精度の評価: [eval/README.md](eval/README.md)、性能: [docs/PERFORMANCE.md](docs/PERFORMANCE.md)
- 設計の記録: [docs/adr/](docs/adr/)、仕様: [docs/SPEC.md](docs/SPEC.md)、開発規約: [AGENTS.md](AGENTS.md)
- README の画像は `python docs/images/gen_readme_images.py` で作り直せます。
- Rust 版(以前の実装、参考として残す)は `cargo install just` のあと `just ci` でテストできます。

## ライセンス

このリポジトリのコードは Apache License 2.0 または MIT License のいずれかを選べます([LICENSE-APACHE](LICENSE-APACHE)、
[LICENSE-MIT](LICENSE-MIT))。Mozc への変更は Mozc と同じ BSD-3-Clause です。同梱するものは次のとおりです
(詳しくは [docs/licenses.md](docs/licenses.md))。

- Mozc: BSD-3-Clause(Google Inc.)
- zenz-v2.5-small・medium: CC BY-SA 4.0(Keita Miwa)
- TinySwallow-1.5B: Apache-2.0(Sakana AI)
- llama.cpp / ggml: MIT、Qt 6: LGPL-3.0

アイコンと README の画像の文字には Noto Serif JP / Noto Sans JP(SIL Open Font License)を使っています。
