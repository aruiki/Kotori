"""README の画像(見出し、特長、変換の例、精度、軽さ、しくみ、設定画面)を作る。
数値は eval/README.md・docs/PERFORMANCE.md・docs/adr の記録。

使い方: python docs/images/gen_readme_images.py
必要: skia-python、Pillow、Noto Serif JP / Noto Sans JP(mozc/tools/kotori_mark.py と同じ)。
設定画面の画像は settings-raw.png(mozc/tools/capture_window.py で撮ったもの)から作る。
"""
import difflib
import sys
from pathlib import Path

import skia

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "mozc" / "tools"))
from kotori_mark import (INK_BOT, INK_TOP, SHU, U, WHITE, font, glyph,  # noqa: E402
                         product)

SANS_R = font("NotoSansJP-VF.ttf", 400)
SANS_M = font("NotoSansJP-VF.ttf", 500)
SANS_B = font("NotoSansJP-VF.ttf", 700)
SERIF_B = font("NotoSerifJP-VF.ttf", 700)
TEXT = skia.Color(0x1F, 0x23, 0x33)
SUB = skia.Color(0x5B, 0x60, 0x70)
LINE = skia.Color(0xE3, 0xE5, 0xEC)
CARD = skia.Color(0xFF, 0xFF, 0xFF)
PANEL = skia.Color(0xF4, 0xF5, 0xFA)
BAR_GRAY = skia.Color(0xB9, 0xBE, 0xCB)
INK_MID = skia.Color(0x3B, 0x47, 0x8F)
SHU_SOFT = skia.Color(0xFD, 0xE9, 0xE1)
INK_SOFT = skia.Color(0xE6, 0xE9, 0xF6)
S = 2  # 2 倍で描く(高解像度の画面向け)


def surface(w, h):
    surf = skia.Surface(w * S, h * S)
    c = surf.getCanvas()
    c.scale(S, S)
    c.clear(skia.ColorTRANSPARENT)
    return surf, c


def save(surf, name):
    surf.makeImageSnapshot().save(str(HERE / name), skia.kPNG)


def measure(s, tf, size):
    return skia.Font(tf, size).measureText(s)


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


def rrect(c, x, y, w, h, r, color):
    c.drawRoundRect(skia.Rect.MakeXYWH(x, y, w, h), r, r, skia.Paint(AntiAlias=True, Color=color))


def mark(c, x, y, size, draw=product):
    """kotori_mark の 1024 単位の絵を (x, y) に size で描く。"""
    c.save()
    c.translate(x, y)
    c.scale(size / U, size / U)
    draw(c, size <= 32)
    c.restore()


def title(c, s, sub, w=None):
    text(c, s, 36, 50, SANS_B, 24, TEXT)
    c.drawRect(skia.Rect.MakeXYWH(36, 60, 24, 3), skia.Paint(Color=SHU))
    if sub:
        text(c, sub, 36, 88, SANS_R, 14, SUB)


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
    glow2 = skia.Paint(AntiAlias=True)
    glow2.setShader(skia.GradientShader.MakeRadial(
        (820, 280), 260, [skia.ColorSetARGB(46, 242, 92, 46), skia.ColorTRANSPARENT]))
    c.drawRoundRect(skia.Rect.MakeWH(w, h), 24, 24, glow2)
    mark(c, 40, 40, 220, glyph)
    text(c, "Kotori日本語入力", 290, 118, SANS_B, 46, WHITE)
    text(c, "Mozc の安定した変換に、文脈を読む AI を。", 292, 164, SANS_M, 21, skia.Color(0xDD, 0xE1, 0xF4))
    # 数字の帯
    stats = [("91.5%", "AJIMEE-Bench"), ("97.5%", "日常の文"), ("0.07秒", "変換 1 回"), ("0", "ネット通信")]
    x = 292
    for v, label in stats:
        text(c, v, x, 222, SANS_B, 24, WHITE)
        text(c, label, x, 244, SANS_R, 12, skia.Color(0xA9, 0xB0, 0xD6))
        x += 130
    c.drawRect(skia.Rect.MakeXYWH(292, 186, 28, 3), skia.Paint(Color=SHU))
    save(surf, "hero.png")


def features():
    items = [
        ("正", "文脈で選ぶ", ["前の文を読んで同音異義語を", "選び分ける。Mozc 51.0% →", "91.5%(AJIMEE-Bench)"]),
        ("予", "先回りの予測", ["打っている間に文の続きを", "考えて候補に出す。", "「よろし」→「よろしくお願いします」"]),
        ("速", "待たせない", ["1 打鍵 1〜3 ms、変換 0.07 秒", "(RTX 3060)。重いときは", "上限で打ち切って固まらない"]),
        ("守", "外に出さない", ["辞書も AI もこの PC の中。", "インターネットに接続せず、", "入力した文字を送らない"]),
        ("軽", "GPU に優しい", ["入力中の GPU の計算を約 4 割減。", "10 分使わなければ VRAM を", "空ける(ゲームの邪魔をしない)"]),
        ("楽", "MSI ひとつ", ["AI のモデルと実行環境も同梱。", "GPU がなくても動く。", "CUDA などの準備は要らない"]),
    ]
    w, h = 880, 470
    surf, c = surface(w, h)
    card(c, w, h)
    title(c, "特長", "Mozc(Google 日本語入力のオープンソース版)の操作と辞書はそのまま。AI がその上で働きます。")
    cw, ch, gx, gy, x0, y0 = 258, 160, 17, 18, 36, 112
    for i, (k, head, lines) in enumerate(items):
        x = x0 + (i % 3) * (cw + gx)
        y = y0 + (i // 3) * (ch + gy)
        rrect(c, x, y, cw, ch, 14, PANEL)
        accent = SHU if i == 0 else INK_TOP
        rrect(c, x + 16, y + 16, 40, 40, 10, accent)
        text(c, k, x + 36, y + 46, SERIF_B, 24, WHITE, align="center")
        text(c, head, x + 68, y + 43, SANS_B, 17, TEXT)
        for j, line in enumerate(lines):
            text(c, line, x + 18, y + 86 + j * 22, SANS_R, 13, SUB)
    save(surf, "features.png")


def examples():
    rows = [
        ("はしのはしをはしでつまむ", "", "橋の橋を橋でつまむ", "箸の箸を箸でつまむ"),
        ("かれはこうえんでこうえんをした", "", "彼は公園で公園をした", "彼は公園で公演をした"),
        ("おんせいにんしきのせいどがあがった", "", "音声認識の制度が上がった", "音声認識の精度が上がった"),
        ("あしたのかいぎはじゅうじからです", "", "明日の会議は従事からです", "明日の会議は１０時からです"),
        ("このやくはむずかしい", "英語の文を日本語にしています。", "この薬は難しい", "この訳は難しい"),
        ("ここではきものをぬいでください", "玄関で靴を脱いで。", "ここでは着物を脱いでください",
         "ここで履物を脱いでください"),
    ]
    w, row_h, top = 880, 96, 108
    h = top + row_h * len(rows) + 40
    surf, c = surface(w, h)
    card(c, w, h)
    title(c, "変換の例", "同じ読みでも、AI が前後の文脈から正しい語を選びます(実際の出力、品質 Standard、2026-10-01)。")
    for i, (yomi, ctx, mozc, kotori) in enumerate(rows):
        y = top + i * row_h
        if i:
            c.drawLine(36, y - 8, w - 36, y - 8, skia.Paint(Color=LINE, StrokeWidth=1))
        label = yomi if not ctx else f"{yomi}   (前の文: {ctx})"
        text(c, label, 36, y + 16, SANS_R, 13, SUB)
        # Mozc
        rrect(c, 36, y + 28, 64, 24, 6, PANEL)
        text(c, "Mozc", 68, y + 45, SANS_M, 12, SUB, align="center")
        text(c, mozc, 112, y + 46, SANS_R, 17, SUB)
        # Kotori(Mozc と違う文字を朱で)
        rrect(c, 36, y + 58, 64, 24, 6, INK_TOP)
        text(c, "Kotori", 68, y + 75, SANS_B, 12, WHITE, align="center")
        x = 112
        sm = difflib.SequenceMatcher(None, mozc, kotori)
        for op, _, _, j1, j2 in sm.get_opcodes():
            seg = kotori[j1:j2]
            if not seg:
                continue
            if op == "equal":
                x += text(c, seg, x, y + 76, SANS_B, 17, TEXT)
            else:
                sw = measure(seg, SANS_B, 17)
                rrect(c, x - 1, y + 59, sw + 2, 23, 4, SHU_SOFT)
                x += text(c, seg, x, y + 76, SANS_B, 17, SHU)
    save(surf, "examples.png")


def bars(c, rows, top, row_h, w, label_w=330, unit="%", vmax=100, fmt="{:.1f}"):
    bar_x = 36 + label_w + 6
    bar_max = w - bar_x - 96
    for i, (name, v, note, ours) in enumerate(rows):
        y = top + i * row_h
        text(c, name, 36, y + 27, SANS_M if ours else SANS_R, 15, TEXT if ours else SUB)
        if note:
            text(c, note, 36, y + 45, SANS_R, 12, SUB)
        bw = bar_max * v / vmax
        paint = skia.Paint(AntiAlias=True)
        if ours:
            paint.setShader(skia.GradientShader.MakeLinear([(bar_x, 0), (bar_x + bw, 0)], [INK_MID, INK_TOP]))
        else:
            paint.setColor(BAR_GRAY)
        c.drawRoundRect(skia.Rect.MakeXYWH(bar_x, y + 10, bw, 26), 6, 6, paint)
        if ours:
            c.drawCircle(bar_x + bw - 13, y + 23, 5, skia.Paint(AntiAlias=True, Color=SHU))
        text(c, fmt.format(v) + unit, bar_x + bw + 12, y + 29, SANS_B if ours else SANS_M, 17,
             TEXT if ours else SUB)


def accuracy():
    rows = [
        ("Kotori日本語入力  Unreal", 93.0, "速い GPU 向けの最高設定", True),
        ("Kotori日本語入力  High", 92.5, "", True),
        ("Kotori日本語入力  Standard(既定)", 91.5, "GPU があればこれ", True),
        ("Kotori日本語入力  Low", 88.0, "GPU で測定", True),
        ("Kotori日本語入力  GPU のない PC", 83.0, "CPU で測定(時間の上限あり)", True),
        ("azooKey + Zenzai(参考)", 85.0, "", False),
        ("Mozc(Google 日本語入力のオープンソース版)", 51.0, "", False),
    ]
    w, row_h, top = 880, 54, 108
    daily_top = top + row_h * len(rows) + 30
    h = daily_top + 2 * 54 + 64
    surf, c = surface(w, h)
    card(c, w, h)
    title(c, "変換の正確さ", "AJIMEE-Bench 200 問で、1 番目の候補が正解だった割合(前の文を AI に渡したとき)")
    bars(c, rows, top, row_h, w)
    c.drawLine(36, daily_top - 14, w - 36, daily_top - 14, skia.Paint(Color=LINE, StrokeWidth=1))
    text(c, "日常の文(メールやチャット 81 問)", 36, daily_top + 8, SANS_B, 16, TEXT)
    bars(c, [("Kotori日本語入力  Standard", 97.5, "", True),
             ("Mozc", 80.2, "", False)], daily_top + 16, 54, w)
    text(c, "測定: 2026-10-01、RTX 3060。azooKey は zenz-v3.2-small、推論 5 回。Google 日本語入力と Microsoft IME は未測定。",
         36, h - 24, SANS_R, 12, SUB)
    save(surf, "accuracy.png")


def lightness():
    """入力中の GPU の計算・VRAM・GPU のない PC の待ち時間(docs/adr/0024、0032〜0035)。"""
    panels = [
        ("入力中の GPU の計算", "AI が GPU を使っている時間の割合", [("beta.4", 68.6, False), ("今", 42.6, True)],
         "%", 100, "{:.1f}"),
        ("使っていないときの VRAM", "Standard、入力をやめて 10 分後", [("前", 1.66, False), ("今", 0.0, True)],
         " GB", 2.0, "{:.2f}"),
        ("GPU のない PC の変換", "打ってすぐ Space、長い文(中央値)", [("前", 380, False), ("今", 162, True)],
         " ms", 400, "{:.0f}"),
    ]
    w, h = 880, 330
    surf, c = surface(w, h)
    card(c, w, h)
    title(c, "軽さ", "精度はそのままに、負荷と待ち時間を下げ続けています(RTX 3060 / デスクトップの CPU で測定)。")
    pw, gap, x0, y0 = 258, 17, 36, 110
    for i, (head, sub, vals, unit, vmax, fmt) in enumerate(panels):
        x = x0 + i * (pw + gap)
        rrect(c, x, y0, pw, 160, 14, PANEL)
        text(c, head, x + 18, y0 + 30, SANS_B, 16, TEXT)
        text(c, sub, x + 18, y0 + 50, SANS_R, 12, SUB)
        for j, (label, v, ours) in enumerate(vals):
            y = y0 + 72 + j * 40
            text(c, label, x + 18, y + 18, SANS_M, 13, TEXT if ours else SUB)
            bx = x + 70
            bmax = pw - 70 - 86
            bw = max(4, bmax * v / vmax)
            paint = skia.Paint(AntiAlias=True)
            if ours:
                paint.setShader(skia.GradientShader.MakeLinear([(bx, 0), (bx + bw, 0)], [INK_MID, INK_TOP]))
            else:
                paint.setColor(BAR_GRAY)
            c.drawRoundRect(skia.Rect.MakeXYWH(bx, y + 2, bw, 22), 5, 5, paint)
            text(c, fmt.format(v) + unit, bx + bw + 8, y + 19, SANS_B if ours else SANS_M, 15,
                 SHU if ours else SUB)
    text(c, "GPU の計算は cost_bench.py、VRAM は idle_test.py、CPU の待ち時間は space_latency.py で測定(mozc/tools)。",
         36, h - 24, SANS_R, 12, SUB)
    save(surf, "lightness.png")


def how():
    w, h = 880, 330
    surf, c = surface(w, h)
    card(c, w, h)
    title(c, "しくみ", "変換キーを押すと、候補を集めて、AI が前の文とのつながりで選びます。")
    steps = [
        ("読み", ["はがいたいので", "きょうははいしゃに…"], None),
        ("候補を集める", ["Mozc の辞書が文を組む", "変換用 AI(zenz)が", "読みから文を書く"], None),
        ("AI が選ぶ", ["zenz と日本語 LLM が", "「自然な日本語か」を", "採点する"], None),
        ("結果", ["歯が痛いので", "今日は歯医者に…"], SHU),
    ]
    bx, by, bw, bh, gap = 36, 108, 187, 172, 20
    for i, (head, lines, accent) in enumerate(steps):
        x = bx + i * (bw + gap)
        rrect(c, x, by, bw, bh, 14, PANEL)
        c.drawCircle(x + 26, by + 30, 13, skia.Paint(AntiAlias=True, Color=INK_TOP if accent is None else SHU))
        text(c, str(i + 1), x + 26, by + 36, SANS_B, 15, WHITE, align="center")
        text(c, head, x + 48, by + 36, SANS_B, 16, TEXT)
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


def settings():
    raw = skia.Image.open(str(HERE / "settings-raw.png"))
    w, h = 880, 560
    surf, c = surface(w, h)
    card(c, w, h)
    # 左に設定画面(影つき)
    sw = 360
    sh = raw.height() * sw / raw.width()
    x, y = 36, (h - sh) / 2
    shadow = skia.Paint(AntiAlias=True, Color=skia.ColorSetARGB(40, 20, 24, 60),
                        MaskFilter=skia.MaskFilter.MakeBlur(skia.kNormal_BlurStyle, 10))
    c.drawRoundRect(skia.Rect.MakeXYWH(x, y + 6, sw, sh), 8, 8, shadow)
    c.save()
    c.clipRRect(skia.RRect.MakeRectXY(skia.Rect.MakeXYWH(x, y, sw, sh), 8, 8), True)
    c.drawImageRect(raw, skia.Rect.MakeXYWH(x, y, sw, sh), skia.SamplingOptions(skia.FilterMode.kLinear))
    c.restore()
    # 右に説明
    tx = 440
    text(c, "設定はひとつの画面で", tx, 92, SANS_B, 24, TEXT)
    c.drawRect(skia.Rect.MakeXYWH(tx, 102, 24, 3), skia.Paint(Color=SHU))
    points = [
        ("品質を選ぶ", ["Low / Standard / High / Unreal。", "GPU がなければ自動で Low になる"]),
        ("AI の状態が見える", ["使っている GPU と VRAM、", "実際に動いている AI を表示"]),
        ("予測を切り替える", ["入力中の AI の予測は", "ひとつのチェックでオン・オフ"]),
        ("モデルを差し替える", ["llama.cpp で読める日本語 LLM", "(GGUF)に置き換えられる"]),
    ]
    for i, (head, lines) in enumerate(points):
        y0 = 150 + i * 92
        c.drawCircle(tx + 10, y0 - 6, 6, skia.Paint(AntiAlias=True, Color=SHU if i == 0 else INK_TOP))
        text(c, head, tx + 28, y0, SANS_B, 17, TEXT)
        for j, line in enumerate(lines):
            text(c, line, tx + 28, y0 + 26 + j * 21, SANS_R, 14, SUB)
    text(c, "ダークモード・ハイコントラスト・画面の拡大にも対応", tx, h - 40, SANS_R, 13, SUB)
    save(surf, "settings.png")


hero()
features()
examples()
accuracy()
lightness()
how()
settings()
print("ok")
