"""Kotori日本語入力のインストーラ(WiX UI)の画像とライセンス文を作る。マークは kotori_mark.py(docs/adr/0019)。
使い方: python gen_assets.py <出力フォルダ>
"""
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

from kotori_mark import INK_BOT_RGB, INK_TOP_RGB, SHU_RGB, glyph, product, render

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
FONTS = Path("C:/Windows/Fonts")


def font(name, size, index=0):
    return ImageFont.truetype(str(FONTS / name), size, index=index)


def gradient(w, h, top, bottom):
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            t = (y / h) * 0.75 + (x / w) * 0.25
            px[x, y] = tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
    return img


# ようこそ・完了の画面(493x312)。左の 164px に藍の帯とマーク、右は白(WiX が文字を置く)。
dlg = Image.new("RGB", (493, 312), (255, 255, 255))
panel = gradient(164, 312, INK_TOP_RGB, INK_BOT_RGB)
glow = Image.new("L", (164, 312), 0)
ImageDraw.Draw(glow).ellipse([-90, -80, 190, 170], fill=46)
glow = glow.filter(ImageFilter.GaussianBlur(44))
panel = Image.composite(Image.new("RGB", (164, 312), (90, 104, 180)), panel, glow)
mark = render(glyph, 132)
panel.paste(mark, (16, 44), mark)
d = ImageDraw.Draw(panel)
d.text((82, 206), "Kotori", font=font("SegUIVar.ttf", 28), fill=(255, 255, 255), anchor="mm")
d.text((82, 236), "日本語入力", font=font("YuGothM.ttc", 14), fill=(206, 212, 236), anchor="mm")
d.rectangle([74, 262, 90, 264], fill=SHU_RGB)
dlg.paste(panel, (0, 0))
dlg.save(out / "dialog.bmp")

# 上部の帯(493x58)。左は白(WiX がタイトルを置く)、右端にマーク。
ban = Image.new("RGB", (493, 58), (255, 255, 255))
d = ImageDraw.Draw(ban)
d.rectangle([0, 57, 493, 58], fill=(224, 224, 224))
icon = render(product, 48)
ban.paste(icon, (437, 5), icon)
ban.save(out / "banner.bmp")


# ライセンス(WixUI の同意画面)。RTF は日本語を \uN? で書く。
def rtf_escape(s):
    o = []
    for ch in s:
        if ch in "\\{}":
            o.append("\\" + ch)
        elif ch == "\n":
            o.append("\\par\n")
        elif ord(ch) < 128:
            o.append(ch)
        else:
            n = ord(ch)
            o.append(f"\\u{n if n < 32768 else n - 65536}?")
    return "".join(o)


text = """Kotori日本語入力 使用許諾と第三者ソフトウェアの表示

Kotori日本語入力(以下 Kotori)は Mozc(Copyright 2010-2026 Google Inc.、BSD-3-Clause)を改変した日本語入力です。Kotori は Google が提供・推奨するものではありません。Kotori 独自の改変部分も BSD-3-Clause で提供します。

本ソフトウェアは「現状のまま」提供され、明示または黙示を問わず、いかなる保証もありません。

同梱している部品とライセンス:
・Mozc: BSD-3-Clause(Google Inc.)
・Qt 6: LGPL-3.0(動的リンク)
・llama.cpp / ggml: MIT(Copyright The ggml authors)
・zenz-v2.5-small: CC BY-SA 4.0(© Keita Miwa。元は ku-nlp/gpt2-small-japanese-char)。8 bit に量子化して同梱
・TinySwallow-1.5B: Apache-2.0(Sakana AI)。5 bit に量子化して同梱
・Microsoft Visual C++ ランタイム: Microsoft のライセンス

各ライセンスの全文と帰属表示は、インストール先(C:\\Program Files (x86)\\Kotori)の NOTICE-*.txt と documents フォルダにあります。

Kotori はインターネットに接続しません。変換と AI の処理は、すべてこの PC の中で行います。
"""
rtf = ("{\\rtf1\\ansi\\ansicpg932\\deff0{\\fonttbl{\\f0\\fnil\\fcharset128 Yu Gothic UI;}}"
       "\\viewkind4\\uc1\\pard\\f0\\fs18 " + rtf_escape(text) + "}")
(out / "license.rtf").write_text(rtf, encoding="ascii")

(out / "NOTICE-tinyswallow.txt").write_text(
    "Kotori日本語入力に同梱している言語モデル(LLM)\n\n"
    "TinySwallow-1.5B (c) Sakana AI\n"
    "  https://huggingface.co/SakanaAI/TinySwallow-1.5B\n"
    "  ライセンス: Apache License 2.0 https://www.apache.org/licenses/LICENSE-2.0\n\n"
    "変更点: 公開されている重みを llama.cpp で GGUF(f16)にし、Q5_K_M に量子化した。\n"
    "学習した内容は変えていない。\n\n"
    "llama.cpp / ggml (MIT): https://github.com/ggml-org/llama.cpp\n"
    "  インストール先の LICENSE-llama.cpp.txt を参照。\n", encoding="utf-8")
print("ok", sorted(p.name for p in out.iterdir()))
