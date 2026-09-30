"""Kotori のアイコン(.ico)を作る(docs/adr/0018)。Fluent 2 の配色で、ブランド色の角丸の四角に白い絵柄。
使い方: python mozc/tools/gen_icons.py <Mozc の src/data/images/win>
Mozc の同じ名前のアイコンを置き換える。タスクバーが明るくても暗くても読めるよう、入力モードも青地に白。
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

out = Path(sys.argv[1])
FONTS = Path("C:/Windows/Fonts")
BRAND = (15, 108, 189, 255)
BRAND_DARK = (12, 59, 94, 255)
WHITE = (255, 255, 255, 255)
SIZES = [16, 20, 24, 32, 48, 64, 128, 256]
S = 256  # 大きく描いて縮める


def base(color_top=BRAND, color_bottom=BRAND_DARK, radius=0.22):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    grad = Image.new("RGBA", (S, S))
    for y in range(S):
        t = y / S
        c = tuple(int(color_top[i] + (color_bottom[i] - color_top[i]) * t * 0.8) for i in range(3)) + (255,)
        ImageDraw.Draw(grad).line([(0, y), (S, y)], fill=c)
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([8, 8, S - 8, S - 8], radius=int(S * radius), fill=255)
    img.paste(grad, (0, 0), mask)
    return img


def bird(d, cx, cy, s, fill=WHITE):
    d.ellipse([cx - 1.0 * s, cy - 0.62 * s, cx + 0.62 * s, cy + 0.72 * s], fill=fill)
    d.ellipse([cx + 0.05 * s, cy - 1.05 * s, cx + 0.95 * s, cy - 0.15 * s], fill=fill)
    d.polygon([(cx + 0.88 * s, cy - 0.72 * s), (cx + 1.32 * s, cy - 0.56 * s), (cx + 0.88 * s, cy - 0.42 * s)],
              fill=(255, 196, 64, 255))
    d.polygon([(cx - 0.85 * s, cy - 0.05 * s), (cx - 1.55 * s, cy - 0.55 * s), (cx - 1.35 * s, cy + 0.15 * s)], fill=fill)
    d.ellipse([cx + 0.52 * s, cy - 0.78 * s, cx + 0.66 * s, cy - 0.64 * s], fill=BRAND_DARK)


def badge(img, kind):
    """右下の小さな印(設定は歯車、辞書は本)。"""
    d = ImageDraw.Draw(img)
    cx, cy, r = 192, 192, 54
    d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=WHITE)
    if kind == "gear":
        import math
        for k in range(8):
            a = k * math.pi / 4
            x, y = cx + math.cos(a) * 34, cy + math.sin(a) * 34
            d.ellipse([x - 10, y - 10, x + 10, y + 10], fill=BRAND)
        d.ellipse([cx - 30, cy - 30, cx + 30, cy + 30], fill=BRAND)
        d.ellipse([cx - 13, cy - 13, cx + 13, cy + 13], fill=WHITE)
    elif kind == "book":
        d.rounded_rectangle([cx - 32, cy - 26, cx - 2, cy + 28], radius=4, fill=BRAND)
        d.rounded_rectangle([cx + 2, cy - 26, cx + 32, cy + 28], radius=4, fill=BRAND)
        for y in (cy - 12, cy, cy + 12):
            d.line([(cx - 26, y), (cx - 8, y)], fill=WHITE, width=4)
            d.line([(cx + 8, y), (cx + 26, y)], fill=WHITE, width=4)
    return img


def product():
    img = base()
    bird(ImageDraw.Draw(img), 120, 146, 62)
    return img


def glyph(text, font_name, size, underline=False, disabled=False):
    img = base(radius=0.26) if not disabled else base((120, 120, 120, 255), (80, 80, 80, 255), 0.26)
    d = ImageDraw.Draw(img)
    f = ImageFont.truetype(str(FONTS / font_name), size)
    y = S / 2 - (14 if underline else 0)
    d.text((S / 2, y), text, font=f, fill=WHITE, anchor="mm")
    if underline:
        d.rounded_rectangle([56, 200, S - 56, 216], radius=6, fill=WHITE)
    return img


def save(img, name):
    img.save(out / name, format="ICO", sizes=[(n, n) for n in SIZES])


icons = {
    "product_icon.ico": product(),
    "product_icon_langbar.ico": product(),
    "tools_icon.ico": product(),
    "tools_icon_a.ico": product(),
    "tools_properties.ico": badge(product(), "gear"),
    "tools_properties_a.ico": badge(product(), "gear"),
    "tools_dictionary.ico": badge(product(), "book"),
    "tools_dictionary_a.ico": badge(product(), "book"),
}
modes = {
    "ms_hiragana": ("あ", "YuGothB.ttc", 176, False),
    "ms_katakana": ("カ", "YuGothB.ttc", 176, False),
    "ms_katakana_half": ("ｶ", "YuGothB.ttc", 176, True),
    "ms_alpha": ("Ａ", "YuGothB.ttc", 176, False),
    "ms_alpha_half": ("A", "SegUIVar.ttf", 176, True),
    "ms_direct_input": ("A", "SegUIVar.ttf", 176, False),
}
for name, (t, f, sz, ul) in modes.items():
    for suffix in ("", "_a"):
        icons[f"{name}{suffix}.ico"] = glyph(t, f, sz, ul)
for suffix in ("", "_a"):
    icons[f"ms_disabled{suffix}.ico"] = glyph("×", "SegUIVar.ttf", 170, disabled=True)

for name, img in icons.items():
    save(img, name)
# 見本(確認用)
sheet = Image.new("RGBA", (64 * len(icons), 64), (243, 243, 243, 255))
for i, img in enumerate(icons.values()):
    sheet.paste(img.resize((56, 56), Image.LANCZOS), (i * 64 + 4, 4), img.resize((56, 56), Image.LANCZOS))
sheet.save(out / "_kotori_icons_preview.png")
print("ok", len(icons))
