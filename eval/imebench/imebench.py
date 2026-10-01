"""実際の IME で変換の正確さを測る(eval/imebench/README.md)。

ImeBench.exe(ImeBench.cs を csc で作る)に読みのローマ字を渡し、IME で変換・確定した文字を集めて、
評価セットの正解と比べる。前の文は渡さない(どの IME にも同じ条件)。全角と半角の違い(数字・記号)は
NFKC で揃えてから比べる(IME の設定の違いで差が出ないように)。

使い方: python eval/imebench/imebench.py <IME の名前> [--data セット.json ...] [--n 問の数] [--wait ms]
  IME の名前: google / msime / kotori(下の IMES)。結果は eval/imebench/out/<名前>_<セット>.tsv。
計測の間、この PC のキーボード入力を使う(ほかの操作をしない)。
"""
import argparse
import json
import subprocess
import sys
import unicodedata
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
IMES = {
    # テキストサービスの CLSID、プロファイルの GUID
    "google": ("{D5A86FD5-5308-47EA-AD16-9C4EB160EC3C}", "{773EB24E-CA1D-4B1B-B420-FA985BB0B80D}"),
    "msime": ("{03B5835F-F03C-411B-9CE2-AA23E1171E36}", "{A76C93D9-5523-4E90-AAFA-4DB112F9AC76}"),
    "kotori": ("{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}", "{455A87E7-275F-4582-92CB-6600917A436D}"),
}

BASE = {
    "あ": "a", "い": "i", "う": "u", "え": "e", "お": "o",
    "か": "ka", "き": "ki", "く": "ku", "け": "ke", "こ": "ko",
    "さ": "sa", "し": "si", "す": "su", "せ": "se", "そ": "so",
    "た": "ta", "ち": "ti", "つ": "tu", "て": "te", "と": "to",
    "な": "na", "に": "ni", "ぬ": "nu", "ね": "ne", "の": "no",
    "は": "ha", "ひ": "hi", "ふ": "hu", "へ": "he", "ほ": "ho",
    "ま": "ma", "み": "mi", "む": "mu", "め": "me", "も": "mo",
    "や": "ya", "ゆ": "yu", "よ": "yo",
    "ら": "ra", "り": "ri", "る": "ru", "れ": "re", "ろ": "ro",
    "わ": "wa", "を": "wo", "ん": "nn", "ゐ": "wyi", "ゑ": "wye",
    "が": "ga", "ぎ": "gi", "ぐ": "gu", "げ": "ge", "ご": "go",
    "ざ": "za", "じ": "zi", "ず": "zu", "ぜ": "ze", "ぞ": "zo",
    "だ": "da", "ぢ": "di", "づ": "du", "で": "de", "ど": "do",
    "ば": "ba", "び": "bi", "ぶ": "bu", "べ": "be", "ぼ": "bo",
    "ぱ": "pa", "ぴ": "pi", "ぷ": "pu", "ぺ": "pe", "ぽ": "po",
    "ぁ": "xa", "ぃ": "xi", "ぅ": "xu", "ぇ": "xe", "ぉ": "xo",
    "ゃ": "xya", "ゅ": "xyu", "ょ": "xyo", "ゎ": "xwa", "っ": "xtu", "ゔ": "vu",
    "ー": "-", "、": ",", "。": ".", "・": "/", "「": "[", "」": "]", "！": "!", "？": "?",
    "〜": "~", "～": "~",
}


def kata_to_hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def to_romaji(reading):
    """読みをローマ字のキーにする。打てない文字(英字など)があれば None。"""
    out = []
    for ch in unicodedata.normalize("NFKC", kata_to_hira(reading)):
        ch = kata_to_hira(ch)
        if ch in BASE:
            out.append(BASE[ch])
        elif ch.isdigit():
            out.append(ch)
        elif ch == ",":
            out.append(",")
        elif ch == ".":
            out.append(".")
        elif ch in "!?()":
            out.append(ch)
        else:
            return None
    return "".join(out)


def norm(s):
    return unicodedata.normalize("NFKC", s).replace(" ", "")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ime", choices=sorted(IMES))
    ap.add_argument("--data", nargs="+", default=["eval/data/ajimee-bench.json", "eval/sets/kotori-daily.json"])
    ap.add_argument("--n", type=int, default=0)
    ap.add_argument("--wait", type=int, default=1200, help="Space を押してから Enter までの待ち(ms)")
    args = ap.parse_args()
    exe = HERE / "ImeBench.exe"
    (HERE / "out").mkdir(exist_ok=True)
    clsid, profile = IMES[args.ime]
    for data in args.data:
        items = json.load(open(REPO / data, encoding="utf-8"))
        if args.n:
            items = items[: args.n]
        name = Path(data).stem
        tsv = HERE / "out" / f"{args.ime}_{name}.in.tsv"
        res = HERE / "out" / f"{args.ime}_{name}.tsv"
        rows, skipped = [], 0
        with open(tsv, "w", encoding="utf-8", newline="\n") as f:
            # 最初の 2 問は慣らし(採点しない)。IME を有効にした直後の変換は、読み込みなどで結果が揺れるため。
            for w, r in enumerate(["kyouhaiitennkidesune", "yoroshikuonegaishimasu"]):
                f.write(f"w{w}\t{r}\n")
            for i, it in enumerate(items):
                r = to_romaji(it["input"])
                if r is None:
                    skipped += 1
                    continue
                rows.append((str(i), it))
                f.write(f"{i}\t{r}\n")
        subprocess.run([str(exe), clsid, profile, str(tsv), str(res), str(args.wait)], check=False)
        err = Path(str(res) + ".error.txt")
        if err.exists():
            print(err.read_text(encoding="utf-8"))
            sys.exit(1)
        got = dict(line.rstrip("\n").split("\t", 1) for line in open(res, encoding="utf-8") if "\t" in line)
        # 確定した文字が空の問は、1 回だけやり直す(キーの取りこぼし)。
        empty = [i for i, _ in rows if not got.get(i)]
        if empty:
            romaji = dict(line.rstrip("\n").split("\t", 1) for line in open(tsv, encoding="utf-8"))
            retry = Path(str(tsv) + ".retry.tsv")
            with open(retry, "w", encoding="utf-8", newline="\n") as f:
                f.write("w0\tkyouhaiitennkidesune\n")
                for i in empty:
                    f.write(f"{i}\t{romaji[i]}\n")
            out2 = Path(str(res) + ".retry.tsv")
            subprocess.run([str(exe), clsid, profile, str(retry), str(out2), str(args.wait)], check=False)
            for line in open(out2, encoding="utf-8"):
                if "\t" in line:
                    k, v = line.rstrip("\n").split("\t", 1)
                    if k in empty:
                        got[k] = v
            with open(res, "w", encoding="utf-8", newline="\n") as f:
                for k, v in got.items():
                    f.write(f"{k}\t{v}\n")
        still = sum(1 for i, _ in rows if not got.get(i))
        ok = sum(1 for i, it in rows if norm(got.get(i, "")) in {norm(e) for e in it["expected_output"]})
        print(f"{args.ime} {name}: {ok}/{len(rows)} = {100 * ok / max(1, len(rows)):.1f}%"
              f"(打てない文字で除いた問 {skipped}、やり直した問 {len(empty)}、空のまま {still})")


if __name__ == "__main__":
    main()
