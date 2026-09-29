# 0009: TSF TIP の構成

- 状態: 採用
- 日付: 2026-09-29

## 背景

10.1 は Windows のフロントエンドを、C++20 のインプロセス COM DLL `kotori_tip.dll` として
実装するよう求める。フロントエンドは変換ロジックを持たず、`kotori-client` の C ABI を呼ぶ
(10章、docs/adr/0008)。REQ-10-1 は x64 と x86 の両方の DLL を登録するよう求める。
Linux では TSF を動かせないので、確認は Windows の CI で行う(AGENTS.md 9)。

## 決定

- ソースは `frontends/windows/` に置き、CMake でビルドする。`kotori-client` は Rust の
  静的ライブラリ(`staticlib`)として DLL にリンクする。ビルドする側が `cargo build` で
  対象のターゲット(`x86_64-pc-windows-msvc`、`i686-pc-windows-msvc`)の静的ライブラリを作り、
  CMake に `KOTORI_CLIENT_LIB` で場所を渡す。DLL は VC ランタイムを静的にリンクする(`/MT`)ので、
  Rust 側も `-C target-feature=+crt-static` で作ってそろえる。
- 段階的に入れる。最初は COM DLL の骨格(クラスファクトリ、登録・解除、TSF のプロファイルと
  カテゴリの登録、`ITfTextInputProcessorEx` の起動・終了)だけにする。キー処理と
  プリエディット、表示属性、候補ウィンドウ、フォーカスの追跡は後続の変更で足す。
- 識別子は次のとおり固定する。
  - テキストサービスの CLSID: `{797AD563-3368-4DCB-B3E7-8B0306B42B67}`
  - 言語プロファイル: `{D8E58E42-4B5C-4E34-88D5-355F38FE91A4}`(日本語、LANGID 0x0411)
  - 表示属性(入力中・変換済み・注目文節): `{7A1ECE80-81F1-4304-845D-B78E92E7D75D}`、
    `{26898B1A-2ACF-4C7D-B666-11ED66C55053}`、`{6786DF7D-0858-4447-87D1-ED6710C2E21B}`
- CI の windows ランナーで x64 と x86 をビルドし、`regsvr32` で登録・解除できることと、
  登録でレジストリに CLSID と TSF のプロファイルができることを確かめる。
- 手元では mingw-w64 の g++ で構文と型を確かめられるようにする(MSVC と完全には同じでないので、
  正は CI の MSVC とする)。

## 影響

- CI に Windows の C++ ビルドのジョブが加わる。
- 実際のアプリでの入力の確認(10.4)は利用者の PC で行う。
