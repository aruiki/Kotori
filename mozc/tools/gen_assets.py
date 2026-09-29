"""Kotori のインストーラ(WiX UI)の画像とライセンス文を作る。Fluent 2 の配色(ブランド色 #0F6CBD)。
使い方: python gen_assets.py <出力フォルダ>
"""
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
FONTS = Path("C:/Windows/Fonts")
BRAND = (15, 108, 189)       # Fluent 2 brand 80
BRAND_DARK = (17, 94, 163)   # brand 70
BRAND_DEEP = (12, 59, 94)    # brand 40


def font(name, size, index=0):
    return ImageFont.truetype(str(FONTS / name), size, index=index)


def bird(draw, cx, cy, s, fill):
    """小鳥(ことり)のシンプルなマーク。丸い体、頭、くちばし、尾。"""
    draw.ellipse([cx - 1.0 * s, cy - 0.62 * s, cx + 0.62 * s, cy + 0.72 * s], fill=fill)          # 体
    draw.ellipse([cx + 0.05 * s, cy - 1.05 * s, cx + 0.95 * s, cy - 0.15 * s], fill=fill)          # 頭
    draw.polygon([(cx + 0.88 * s, cy - 0.72 * s), (cx + 1.32 * s, cy - 0.56 * s),
                  (cx + 0.88 * s, cy - 0.42 * s)], fill=(255, 196, 64))                           # くちばし
    draw.polygon([(cx - 0.85 * s, cy - 0.05 * s), (cx - 1.55 * s, cy - 0.55 * s),
                  (cx - 1.35 * s, cy + 0.15 * s)], fill=fill)                                     # 尾
    draw.ellipse([cx + 0.52 * s, cy - 0.78 * s, cx + 0.66 * s, cy - 0.64 * s], fill=BRAND_DEEP)  # 目


def gradient(w, h, top, bottom):
    img = Image.new("RGB", (w, h))
    px = img.load()
    for y in range(h):
        for x in range(w):
            t = (y / h) * 0.75 + (x / w) * 0.25
            px[x, y] = tuple(int(top[i] + (bottom[i] - top[i]) * t) for i in range(3))
    return img


# ようこそ・完了の画面(493x312)。左の 164px に絵、右は白(WiX が文字を置く)。
dlg = Image.new("RGB", (493, 312), (255, 255, 255))
panel = gradient(164, 312, BRAND, BRAND_DEEP)
glow = Image.new("L", (164, 312), 0)
ImageDraw.Draw(glow).ellipse([-80, -60, 200, 200], fill=90)
glow = glow.filter(ImageFilter.GaussianBlur(40))
panel = Image.composite(Image.new("RGB", (164, 312), (98, 171, 245)), panel, glow)
d = ImageDraw.Draw(panel)
# 柔らかい曲線(Fluent の有機的な形)
for k, a in enumerate((40, 28)):
    d.arc([-120 + k * 30, 170 + k * 20, 260 + k * 30, 470 + k * 20], 180, 360,
          fill=tuple(min(255, c + a) for c in BRAND), width=2)
bird(d, 84, 118, 30, (255, 255, 255))
d.text((82, 188), "Kotori", font=font("SegUIVar.ttf", 30), fill=(255, 255, 255), anchor="mm")
d.text((82, 220), "AI 日本語入力", font=font("YuGothM.ttc", 13), fill=(222, 236, 249), anchor="mm")
dlg.paste(panel, (0, 0))
dlg.save(out / "dialog.bmp")

# 上部の帯(493x58)。左は白(WiX がタイトルを置く)、右端に小さなマーク。
ban = Image.new("RGB", (493, 58), (255, 255, 255))
d = ImageDraw.Draw(ban)
d.rectangle([0, 56, 493, 58], fill=BRAND)
d.ellipse([437, 7, 481, 51], fill=BRAND)
bird(d, 458, 31, 10, (255, 255, 255))
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


text = """Kotori IME 使用許諾と第三者ソフトウェアの表示

Kotori は Mozc(Copyright 2010-2026 Google Inc.、BSD-3-Clause)を改変した日本語入力です。Kotori は Google が提供・推奨するものではありません。Kotori 独自の改変部分も BSD-3-Clause で提供します。

本ソフトウェアは「現状のまま」提供され、明示または黙示を問わず、いかなる保証もありません。

同梱している部品とライセンス:
・Mozc: BSD-3-Clause(Google Inc.)
・Qt 6: LGPL-3.0(動的リンク)
・llama.cpp / ggml: MIT(Copyright The ggml authors)
・zenz-v2.5-small: CC BY-SA 4.0(© Keita Miwa。元は ku-nlp/gpt2-small-japanese-char)。8 bit に量子化して同梱
・TinySwallow-1.5B: Apache-2.0(Sakana AI)。5 bit に量子化して同梱
・Microsoft Visual C++ ランタイム: Microsoft のライセンス

各ライセンスの全文と帰属表示は、インストール先(C:\\Program Files\\Kotori)の NOTICE-*.txt と documents フォルダにあります。

Kotori はインターネットに接続しません。変換と AI の処理は、すべてこの PC の中で行います。
"""
rtf = ("{\\rtf1\\ansi\\ansicpg932\\deff0{\\fonttbl{\\f0\\fnil\\fcharset128 Yu Gothic UI;}}"
       "\\viewkind4\\uc1\\pard\\f0\\fs18 " + rtf_escape(text) + "}")
(out / "license.rtf").write_text(rtf, encoding="ascii")

(out / "NOTICE-tinyswallow.txt").write_text(
    "Kotori IME に同梱している言語モデル(LLM)\n\n"
    "TinySwallow-1.5B (c) Sakana AI\n"
    "  https://huggingface.co/SakanaAI/TinySwallow-1.5B\n"
    "  ライセンス: Apache License 2.0 https://www.apache.org/licenses/LICENSE-2.0\n\n"
    "変更点: 公開されている重みを llama.cpp で GGUF(f16)にし、Q5_K_M に量子化した。\n"
    "学習した内容は変えていない。\n\n"
    "llama.cpp / ggml (MIT): https://github.com/ggml-org/llama.cpp\n"
    "  インストール先の LICENSE-llama.cpp.txt を参照。\n", encoding="utf-8")
print("ok", sorted(p.name for p in out.iterdir()))
