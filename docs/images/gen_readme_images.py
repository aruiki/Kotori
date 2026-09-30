"""README の画像(見出し、精度の比較、しくみ)を作る。数値は eval/README.md の記録。

使い方: python docs/images/gen_readme_images.py
必要: skia-python、Pillow、Noto Serif JP / Noto Sans JP(mozc/tools/kotori_mark.py と同じ)。
"""
import sys
from pathlib import Path

import skia

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "mozc" / "tools"))
from kotori_mark import (INK_BOT, INK_TOP, SERIF, SHU, U, WHITE, font, glyph, glyph_path,  # noqa: E402
                         product)

SANS_R = font("NotoSansJP-VF.ttf", 400)
SANS_M = font("NotoSansJP-VF.ttf", 500)
SANS_B = font("NotoSansJP-VF.ttf", 700)
TEXT = skia.Color(0x1F, 0x23, 0x33)
SUB = skia.Color(0x5B, 0x60, 0x70)
LINE = skia.Color(0xE3, 0xE5, 0xEC)
CARD = skia.Color(0xFF, 0xFF, 0xFF)
BAR_GRAY = skia.Color(0xB9, 0xBE, 0xCB)
INK_MID = skia.Color(0x3B, 0x47, 0x8F)
S = 2  # 2 倍で描く(高解像度の画面向け)


def surface(w, h):
    surf = skia.Surface(w * S, h * S)
    c = surf.getCanvas()
    c.scale(S, S)
    c.clear(skia.ColorTRANSPARENT)
    return surf, c


def save(surf, name):
    surf.makeImageSnapshot().save(str(HERE / name), skia.kPNG)


def text(c, s, x, y, tf, size, color, align="left"):
    f = skia.Font(tf, size)
    f.setEdging(skia.Font.Edging.kSubpixelAntiAlias)
    w = f.measureText(s)
    if align == "center":
        x -= w / 2
    elif align == "right":
        x -= w
    c.drawString(s, x, y, f, skia.Paint(AntiAlias=True, Color=color))
    return w


def card(c, w, h, r=20):
    """明るい画面でも暗い画面でも読めるよう、白いカードに描く。"""
    c.drawRoundRect(skia.Rect.MakeWH(w, h), r, r, skia.Paint(AntiAlias=True, Color=CARD))
    c.drawRoundRect(skia.Rect.MakeXYWH(0.5, 0.5, w - 1, h - 1), r, r,
                    skia.Paint(AntiAlias=True, Style=skia.Paint.kStroke_Style, StrokeWidth=1, Color=LINE))


def mark(c, x, y, size, draw=product):
    """kotori_mark の 1024 単位の絵を (x, y) に size で描く。"""
    c.save()
    c.translate(x, y)
    c.scale(size / U, size / U)
    draw(c, size <= 32)
    c.restore()


def hero():
    w, h = 880, 300
    surf, c = surface(w, h)
    p = skia.Paint(AntiAlias=True)
    p.setShader(skia.GradientShader.MakeLinear([(0, 0), (w, h)], [INK_TOP, INK_BOT]))
    c.drawRoundRect(skia.Rect.MakeWH(w, h), 24, 24, p)
    glow = skia.Paint(AntiAlias=True)
    glow.setShader(skia.GradientShader.MakeRadial(
        (140, 60), 360, [skia.ColorSetARGB(70, 110, 126, 220), skia.ColorTRANSPARENT]))
    c.drawRoundRect(skia.Rect.MakeWH(w, h), 24, 24, glow)
    mark(c, 40, 40, 220, glyph)
    text(c, "Kotori日本語入力", 290, 128, SANS_B, 46, WHITE)
    text(c, "Mozc の安定した変換に、文脈を読む AI を。", 292, 176, SANS_M, 21, skia.Color(0xDD, 0xE1, 0xF4))
    text(c, "Windows 用 ・ オフラインで動作 ・ 無料・オープンソース", 292, 214, SANS_R, 16,
         skia.Color(0xA9, 0xB0, 0xD6))
    c.drawRect(skia.Rect.MakeXYWH(292, 238, 28, 3), skia.Paint(Color=SHU))
    save(surf, "hero.png")


def accuracy():
    rows = [
        ("Kotori日本語入力  High", 91.5, "GPU", True),
        ("Kotori日本語入力  Standard(既定)", 90.5, "GPU", True),
        ("azooKey + Zenzai(参考)", 85.0, "", False),
        ("Kotori日本語入力  Low", 84.5, "GPU なしでも動く", True),
        ("Mozc(Google 日本語入力のオープンソース版)", 51.0, "", False),
    ]
    w, row_h, top = 880, 58, 104
    h = top + row_h * len(rows) + 64
    surf, c = surface(w, h)
    card(c, w, h)
    text(c, "変換の正確さ", 36, 50, SANS_B, 24, TEXT)
    text(c, "AJIMEE-Bench 200 問で、1 番目の候補が正解だった割合(前の文を AI に渡したとき)", 36, 78, SANS_R, 14, SUB)
    label_w, bar_x = 330, 372
    bar_max = w - bar_x - 90
    for i, (name, v, note, ours) in enumerate(rows):
        y = top + i * row_h
        text(c, name, 36, y + 27, SANS_M if ours else SANS_R, 15, TEXT if ours else SUB)
        if note:
            text(c, note, 36, y + 46, SANS_R, 12, SUB)
        bw = bar_max * v / 100
        paint = skia.Paint(AntiAlias=True)
        if ours:
            paint.setShader(skia.GradientShader.MakeLinear([(bar_x, 0), (bar_x + bw, 0)], [INK_MID, INK_TOP]))
        else:
            paint.setColor(BAR_GRAY)
        c.drawRoundRect(skia.Rect.MakeXYWH(bar_x, y + 10, bw, 26), 6, 6, paint)
        if ours:
            c.drawCircle(bar_x + bw - 13, y + 23, 5, skia.Paint(AntiAlias=True, Color=SHU))
        text(c, f"{v:.1f}%", bar_x + bw + 12, y + 29, SANS_B if ours else SANS_M, 17, TEXT if ours else SUB)
    text(c, "測定: 2026-09-30、RTX 3060。azooKey は zenz-v3.2-small、推論 5 回。Google 日本語入力と Microsoft IME は未測定。",
         36, h - 26, SANS_R, 12, SUB)
    save(surf, "accuracy.png")


def how():
    w, h = 880, 330
    surf, c = surface(w, h)
    card(c, w, h)
    text(c, "しくみ", 36, 50, SANS_B, 24, TEXT)
    text(c, "変換キーを押すと、候補を集めて、AI が前の文とのつながりで選びます。", 36, 78, SANS_R, 14, SUB)
    steps = [
        ("読み", ["はがいたいので", "きょうははいしゃに…"], None),
        ("候補を集める", ["Mozc の辞書が文を組む", "変換用 AI(zenz)が", "読みから文を書く"], None),
        ("AI が選ぶ", ["zenz と日本語 LLM が", "「自然な日本語か」を", "採点する"], None),
        ("結果", ["歯が痛いので", "今日は歯医者に…"], SHU),
    ]
    bx, by, bw, bh, gap = 36, 104, 187, 176, 20
    for i, (title, lines, accent) in enumerate(steps):
        x = bx + i * (bw + gap)
        fill = skia.Paint(AntiAlias=True, Color=skia.Color(0xF4, 0xF5, 0xFA))
        c.drawRoundRect(skia.Rect.MakeXYWH(x, by, bw, bh), 14, 14, fill)
        c.drawCircle(x + 26, by + 30, 13, skia.Paint(AntiAlias=True, Color=INK_TOP if accent is None else SHU))
        text(c, str(i + 1), x + 26, by + 36, SANS_B, 15, WHITE, align="center")
        text(c, title, x + 48, by + 36, SANS_B, 16, TEXT)
        for j, line in enumerate(lines):
            last = i == len(steps) - 1
            text(c, line, x + 18, by + 76 + j * 26, SANS_M if last else SANS_R, 16 if last else 14,
                 TEXT if last else SUB)
        if i < len(steps) - 1:
            ax = x + bw + 4
            p = skia.Path()
            p.moveTo(ax, by + bh / 2 - 7)
            p.lineTo(ax + 13, by + bh / 2)
            p.lineTo(ax, by + bh / 2 + 7)
            p.close()
            c.drawPath(p, skia.Paint(AntiAlias=True, Color=BAR_GRAY))
    text(c, "Space で変換、入力中は AI が続きを予測して候補に出します(Tab で選ぶ)。処理はすべてこの PC の中。",
         36, h - 24, SANS_R, 13, SUB)
    save(surf, "how.png")


hero()
accuracy()
how()
print("ok")
