"""Kotori日本語入力のアイコン(.ico)を作る(docs/adr/0019)。マークは kotori_mark.py。

入力モード(あ・カ・A など)はタスクバーの明暗どちらでも読めるよう、
Windows の IME と同じく白い字に濃い縁取り。

使い方: python mozc/tools/gen_icons.py <Mozc の src/data/images/win> [見本の PNG の出力先]
"""
import sys
from pathlib import Path

import skia
from PIL import Image

from kotori_mark import (SANS, SANS_BOLD, SERIF, SHU, U, WHITE, glyph_path, paint, product,
                         render, tile, with_dot)

out = Path(sys.argv[1])
preview = Path(sys.argv[2]) if len(sys.argv) > 2 else None
SIZES = [16, 20, 24, 32, 48, 64, 128, 256]
OUTLINE = skia.Color(0x1A, 0x1A, 0x1A)
GRAY = skia.Color(0x9A, 0x9A, 0x9A)


def dictionary(c, small):
    tile(c, small)
    if small:
        with_dot(c, glyph_path(SANS, "辞", 600), 70, 96, shift=0)
    else:
        with_dot(c, glyph_path(SERIF, "辞", 520), 42, 48, shift=0)


def gear_path(cx, cy, r_out, r_in, r_hole, teeth=8):
    g = skia.Path()
    g.addCircle(cx, cy, r_in)
    tw = r_out * 0.46
    for k in range(teeth):
        t = skia.Path()
        t.addRoundRect(skia.Rect.MakeLTRB(cx - tw / 2, cy - r_out, cx + tw / 2, cy), tw * 0.22, tw * 0.22)
        t.transform(skia.Matrix.RotateDeg(360 / teeth * k, (cx, cy)))
        g = skia.Op(g, t, skia.PathOp.kUnion_PathOp)
    hole = skia.Path()
    hole.addCircle(cx, cy, r_hole)
    return skia.Op(g, hole, skia.PathOp.kDifference_PathOp)


def properties(c, small):
    tile(c, small)
    s = 1.18 if small else 1.0
    c.drawPath(gear_path(U / 2, U / 2, 300 * s, 236 * s, 110 * s), paint(WHITE))
    c.drawCircle(U / 2, U / 2, 58 * s, paint(SHU))


def mode(text, underline=False, disabled=False):
    """タスクバーの入力モード。白い字に濃い縁取り(明るい所でも暗い所でも読める)。"""
    def draw(c, small):
        p = glyph_path(SANS_BOLD, text, 820 if small else 760)
        b = p.computeTightBounds()
        # 下線を引くものは、字を小さめにして下に線の場所を空ける
        box_w = U * (0.92 if small else 0.86)
        box_h = U * (0.66 if underline else (0.92 if small else 0.86))
        scale = min(box_w / b.width(), box_h / b.height())
        p.transform(skia.Matrix.Scale(scale, scale))
        b = p.computeTightBounds()
        dy = -125 if underline else 0
        p.offset(U / 2 - (b.left() + b.right()) / 2, U / 2 - (b.top() + b.bottom()) / 2 + dy)
        edge = 96 if small else 64
        fill = GRAY if disabled else WHITE
        for q, col in ((p, OUTLINE),):
            sp = skia.Paint(AntiAlias=True, Style=skia.Paint.kStroke_Style, StrokeWidth=edge,
                            StrokeJoin=skia.Paint.kRound_Join, Color=col)
            c.drawPath(q, sp)
        if underline:
            b = p.computeTightBounds()
            bar = skia.Rect.MakeLTRB(b.left(), b.bottom() + 100, b.right(), b.bottom() + 190)
            c.drawRoundRect(bar.makeOutset(edge / 2, edge / 2), 40 + edge / 2, 40 + edge / 2, paint(OUTLINE))
            c.drawRoundRect(bar, 40, 40, paint(fill))
        c.drawPath(p, paint(fill))
    return draw


def save(draw, name):
    imgs = [render(draw, n) for n in SIZES]
    # 古い Windows SDK の rc.exe は PNG 入りの .ico を受け付けない(RC2176)ので、BMP で入れる。
    imgs[-1].save(out / name, format="ICO", sizes=[(n, n) for n in SIZES],
                  append_images=imgs[:-1], bitmap_format="bmp")
    return imgs


icons = {
    "product_icon.ico": product,
    "product_icon_langbar.ico": product,
    "tools_icon.ico": product,
    "tools_icon_a.ico": product,
    "tools_properties.ico": properties,
    "tools_properties_a.ico": properties,
    "tools_dictionary.ico": dictionary,
    "tools_dictionary_a.ico": dictionary,
}
modes = {
    "ms_hiragana": mode("あ"),
    "ms_katakana": mode("カ"),
    "ms_katakana_half": mode("ｶ", underline=True),
    "ms_alpha": mode("Ａ"),
    "ms_alpha_half": mode("A", underline=True),
    "ms_direct_input": mode("A"),
    "ms_disabled": mode("×", disabled=True),
}
for name, draw in modes.items():
    for suffix in ("", "_a"):
        icons[f"{name}{suffix}.ico"] = draw

rendered = {name: save(draw, name) for name, draw in icons.items()}

if preview:
    # 見本: 明るい背景と暗い背景に、256・32・24・16 を並べる
    names = list(dict.fromkeys(n.replace("_a.ico", ".ico") for n in rendered))
    cell = 150
    sheet = Image.new("RGBA", (cell * len(names), 2 * cell), (0, 0, 0, 0))
    for j, bg in enumerate([(243, 243, 243, 255), (32, 32, 32, 255)]):
        sheet.paste(Image.new("RGBA", (cell * len(names), cell), bg), (0, j * cell))
        for i, n in enumerate(names):
            imgs = rendered[n]
            sheet.alpha_composite(imgs[-1].resize((88, 88), Image.LANCZOS), (i * cell + 8, j * cell + 8))
            x = i * cell + 8
            for k in (3, 2, 0):
                sheet.alpha_composite(imgs[k], (x, j * cell + 108))
                x += SIZES[k] + 8
    sheet.save(preview)
print("ok", len(icons))
