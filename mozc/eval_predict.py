#!/usr/bin/env python3
"""Tab の予測(PREDICTION)を測る(docs/adr/0018)。

AJIMEE-Bench の各問で、読みの先頭 N 文字(ひらがな)を「打った途中」とし、前の文を渡して予測させる。
上位 K 件に、正解の文の先頭と一致する候補(MIN 文字以上)があれば当たり。当たった候補の長さの平均も出す。

使い方: python3 mozc/eval_predict.py <converter_main> [--data 評価セット.json] [--typed 5] [--top 3] [--min 5]
  --warm 秒: 実際の入力のように、読みを 1 文字ずつ 0.15 秒おきに入力中の候補(suggest)として送り、
  指定の秒数待ってから Tab(predict)を押す(裏で計算した予測を使う経路、docs/adr/0021)。
  Tab の応答時間の中央値と最大も出す。
  --suggest: --warm と同じように打ったあと、Tab ではなく入力中の候補(suggest)をもう一度出して測る
  (打鍵ごとに裏で作る軽い予測の当たり。docs/adr/0024)。
"""
import argparse
import threading
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def kata_to_hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def run_warm(exe, cwd, env, typed, warm, tab_ms, final="predict"):
    """1 文字ずつ入力中の候補を出してから Tab を押す。predict の出力だけを返す。"""
    p = subprocess.Popen([str(exe)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, cwd=cwd, env=env)
    lines = []
    threading.Thread(target=lambda: [lines.append(l) for l in p.stdout], daemon=True).start()

    def send(cmd):
        start = len(lines)
        t = time.time()
        p.stdin.write(f"{cmd}\nreset\nkotori_eval_separator\n".encode("utf-8"))
        p.stdin.flush()
        while not any(l.startswith(b"ExecCommand() return false") for l in lines[start:]):
            time.sleep(0.001)
        return lines[start:], (time.time() - t) * 1000

    time.sleep(8)  # モデルの読み込み
    out = []
    for t in typed:
        for n in range(1, len(t) + 1):
            send(f"suggest {t[:n]}")
            time.sleep(0.15)
        time.sleep(warm)
        got, ms = send(f"{final} {t}")
        tab_ms.append(ms)
        out += [l.decode("utf-8", "replace").rstrip("\r\n") for l in got]
    p.stdin.close()
    p.wait()
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("converter_main")
    ap.add_argument("--data", default="eval/data/ajimee-bench.json")
    ap.add_argument("--typed", type=int, default=5)
    ap.add_argument("--top", type=int, default=3)
    ap.add_argument("--min", type=int, default=5)
    ap.add_argument("--out", default="")
    ap.add_argument("--warm", type=float, default=-1)
    ap.add_argument("--suggest", action="store_true", help="最後に Tab ではなく入力中の候補を測る")
    args = ap.parse_args()
    exe = Path(args.converter_main).resolve()
    cwd = exe.parent / (exe.name + ".runfiles") / "_main"
    items = json.load(open(args.data, encoding="utf-8"))
    typed, seen = [], set()
    rows = []
    for it in items:
        if "typed" in it:  # Kotori の予測セット(eval/sets/kotori-predict.json)
            if it["typed"] not in seen:
                seen.add(it["typed"])
                typed.append(it["typed"])
                rows.append({"typed": it["typed"], "context": it["context"], "truth": it["truth"],
                             "conv": it["conv"]})
            continue
        # 読みの先頭 N 文字(記号で始まる問題は除く)
        r = kata_to_hira(it["input"])
        if not ("ぁ" <= r[0] <= "ゖ") or len(r) < args.typed + 3:
            continue
        t = r[: args.typed]
        if t in seen:
            continue
        seen.add(t)
        typed.append(t)
        rows.append({"typed": t, "context": it.get("context_text", ""), "truth": it["expected_output"][0]})
    f = tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".tsv", delete=False)
    for r in rows:
        if r["context"]:
            f.write(f"{r['typed']}\t{r['context']}\n")
    f.close()
    env = dict(os.environ, KOTORI_LM_CONTEXT_MAP=f.name)
    t0 = time.time()
    tab_ms = []
    if args.warm >= 0:
        out = run_warm(exe, cwd, env, typed, args.warm, tab_ms,
                       "suggest" if args.suggest else "predict")
    else:
        script = "".join(f"predict {t}\nreset\nkotori_eval_separator\n" for t in typed) + "quit\n"
        out = subprocess.run([str(exe)], input=script.encode("utf-8"), capture_output=True, cwd=cwd,
                             env=env, timeout=36000).stdout.decode("utf-8", "replace")
    elapsed = time.time() - t0
    blocks, cur = [], []
    for line in out.splitlines():
        if line.startswith("ExecCommand() return false"):
            blocks.append(cur)
            cur = []
        else:
            cur.append(line)
    hit, saved = 0, []
    for i, r in enumerate(rows):
        cands = []
        for line in blocks[i] if i < len(blocks) else []:
            parts = line.strip().split(" ", 1)
            if line.startswith("  ") and len(parts) == 2 and "/" in parts[0] and parts[0].split("/")[0].isdigit():
                cands.append(parts[1])
        r["cands"] = cands[: args.top]
        # 予測セットでは、打った部分の変換(conv)より長い正しい候補だけを当たりとする。
        least = len(r["conv"]) + 1 if "conv" in r else args.min
        good = [c for c in cands[: args.top] if len(c) >= least and r["truth"].startswith(c)]
        if good:
            hit += 1
            saved.append(max(len(c) for c in good))
    n = len(rows)
    full = sum(1 for r in rows if r["truth"] in r.get("cands", []))
    print(f"文まで当たり {full}/{n}  ", end="")
    print(f"当たり {hit}/{n} = {100 * hit / n:.1f}%  当たった候補の長さ {sum(saved) / max(1, len(saved)):.1f} 文字  ({elapsed:.0f} 秒)")
    if tab_ms:
        tab_ms.sort()
        print(f"Tab の応答 中央値 {tab_ms[len(tab_ms) // 2]:.0f} ms、最大 {tab_ms[-1]:.0f} ms")
    if args.out:
        json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    sys.exit(main())
