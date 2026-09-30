"""Kotori日本語入力のマーク(docs/adr/0019)。gen_icons.py と gen_assets.py が使う。

マークは明朝の「こ」に朱の点(句点・印)。藍のタイルに白で置く。32px 以下は線の細い明朝が潰れるので、
ゴシックの太字で描き直す。必要: skia-python、Pillow、Noto Serif JP / Noto Sans JP(可変、OFL)。
"""
from pathlib import Path

import skia
from PIL import Image

FONTS = Path("C:/Windows/Fonts")
U = 1024  # 1024 単位で描いて縮める
SMALL = 32  # これ以下は小さい用の描き方

INK_TOP = skia.Color(0x24, 0x2C, 0x5C)  # 藍
INK_BOT = skia.Color(0x0C, 0x10, 0x2A)
WHITE = skia.ColorWHITE
SHU = skia.Color(0xF2, 0x5C, 0x2E)  # 朱
INK_TOP_RGB = (0x24, 0x2C, 0x5C)
INK_BOT_RGB = (0x0C, 0x10, 0x2A)
SHU_RGB = (0xF2, 0x5C, 0x2E)


def font(file, wght):
    tf = skia.Typeface.MakeFromFile(str(FONTS / file))
    coord = skia.FontArguments.VariationPosition.Coordinate(0x77676874, wght)  # 'wght'
    pos = skia.FontArguments.VariationPosition(
        skia.FontArguments.VariationPosition.Coordinates([coord]))
    args = skia.FontArguments()
    args.setVariationDesignPosition(pos)
    return tf.makeClone(args)


SERIF = font("NotoSerifJP-VF.ttf", 800)
SANS = font("NotoSansJP-VF.ttf", 900)
SANS_BOLD = font("NotoSansJP-VF.ttf", 700)


def glyph_path(tf, text, size):
    f = skia.Font(tf, size)
    glyphs = f.textToGlyphs(text)
    path = skia.Path()
    x = 0.0
    for g, w in zip(glyphs, f.getWidths(glyphs)):
        p = f.getPath(g)
        if p is not None:
            p.offset(x, 0)
            path.addPath(p)
        x += w
    return path


def paint(color):
    return skia.Paint(AntiAlias=True, Color=color)


def tile(c, small):
    inset = 16 if small else 72
    rect = skia.Rect.MakeLTRB(inset, inset, U - inset, U - inset)
    rad = (U - 2 * inset) * (0.2 if small else 0.23)
    p = skia.Paint(AntiAlias=True)
    p.setShader(skia.GradientShader.MakeLinear([(0, inset), (0, U - inset)], [INK_TOP, INK_BOT]))
    c.drawRoundRect(rect, rad, rad, p)
    if not small:
        # 上端のわずかな光(Fluent の奥行き)
        hl = skia.Paint(AntiAlias=True, Style=skia.Paint.kStroke_Style, StrokeWidth=5)
        hl.setShader(skia.GradientShader.MakeLinear(
            [(0, inset), (0, U / 2)], [skia.ColorSetARGB(60, 255, 255, 255), skia.ColorTRANSPARENT]))
        c.drawRoundRect(rect.makeInset(2.5, 2.5), rad - 2.5, rad - 2.5, hl)


def with_dot(c, path, dot_r, dot_dx, shift=-6.0):
    """字と朱の点をまとめて真ん中に置いて描く。"""
    b = path.computeTightBounds()
    left = U / 2 - (b.width() + dot_dx + dot_r) / 2
    path.offset(left - b.left(), U / 2 - (b.top() + b.bottom()) / 2 + shift)
    c.drawPath(path, paint(WHITE))
    b = path.computeTightBounds()
    c.drawCircle(b.right() + dot_dx, b.bottom() - dot_r, dot_r, paint(SHU))


def glyph(c, small):
    """タイルなしの「こ」と朱の点(暗い地に置く)。"""
    if small:
        with_dot(c, glyph_path(SANS, "こ", 640), 74, 120)
    else:
        with_dot(c, glyph_path(SERIF, "こ", 600), 44, 58)


def product(c, small):
    tile(c, small)
    glyph(c, small)


def render(draw, size):
    """draw(canvas, small) を size px の PIL 画像にする。"""
    surf = skia.Surface(U, U)
    c = surf.getCanvas()
    c.clear(skia.ColorTRANSPARENT)
    draw(c, size <= SMALL)
    img = Image.fromarray(surf.makeImageSnapshot().toarray(colorType=skia.kRGBA_8888_ColorType))
    return img.resize((size, size), Image.LANCZOS)
