#!/usr/bin/env python3
"""Mozc(//converter:converter_main)で AJIMEE-Bench などの Acc@1 を測る(docs/adr/0012 段階 3)。

使い方: python3 mozc/eval_baseline.py <converter_main> [--data 評価セット.json] [--out 結果.json] [--context]
  <converter_main> は bazel-bin/converter/converter_main(Windows は .exe)。データは runfiles の中にあるので、
  runfiles の _main を作業ディレクトリにして起動する。
1 回の起動で全問を解く(AI のモデルの読み込みは 1 回だけ)。第 1 候補は各文節の第 1 候補をつないだ文。
LM リランク(docs/adr/0013、0016)は環境変数 KOTORI_ZENZ_MODEL などで設定する(rewriter/lm_rewriter.h)。
--context は問題の前の文(context_text)を AI に渡す(アプリから直前の文を受け取った場合に当たる)。
前の文は「読み<TAB>前の文」のファイルを KOTORI_LM_CONTEXT_MAP で渡す。
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def kata_to_hira(s: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


SEG = re.compile(r"^-{10} Segment \d+/\d+ \[.*\] -{10}$")
CAND = re.compile(r"^\s+0/\d+ (.*)$")
SEP = "kotori_eval_separator"


def first_candidates(lines) -> str:
    parts, want = [], False
    for line in lines:
        if SEG.match(line):
            want = True
        elif want:
            m = CAND.match(line)
            if m:
                parts.append(m.group(1))
                want = False
    return "".join(parts)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("converter_main")
    ap.add_argument("--data", default="eval/data/ajimee-bench.json")
    ap.add_argument("--out", default="")
    ap.add_argument("--context", action="store_true", help="問題の context_text を前の文として AI に渡す")
    args = ap.parse_args()

    exe = Path(args.converter_main).resolve()
    name = exe.name[:-4] if exe.name.endswith(".exe") else exe.name
    cwd = exe.parent / (exe.name + ".runfiles") / "_main"
    items = json.load(open(args.data, encoding="utf-8"))
    readings = [kata_to_hira(it["input"]) for it in items]

    env = dict(os.environ)
    if args.context:
        f = tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".tsv", delete=False)
        for r, it in zip(readings, items):
            if it.get("context_text"):
                f.write(f"{r}\t{it['context_text']}\n")
        f.close()
        env["KOTORI_LM_CONTEXT_MAP"] = f.name
    # 問題ごとに start → reset し、知らないコマンドの出力を区切りにする。
    script = "".join(f"start {r}\nreset\n{SEP}\n" for r in readings) + "quit\n"
    t0 = time.time()
    proc = subprocess.run([str(exe)], input=script.encode("utf-8"), capture_output=True,
                          cwd=cwd, env=env, timeout=36000)
    out = proc.stdout.decode("utf-8", "replace")
    # KOTORI_LM_TIME を付けたときは、AI の変換にかかった時間(文全体の選択)の分布も出す。
    times = sorted(float(m.group(1)) for m in
                   re.finditer(r"\[time\].* total=(\d+)ms", proc.stderr.decode("utf-8", "replace")))
    elapsed = time.time() - t0
    blocks, cur = [], []
    for line in out.splitlines():
        if line.startswith("ExecCommand() return false"):
            blocks.append(cur)
            cur = []
        else:
            cur.append(line)
    rows, hit = [], 0
    for i, it in enumerate(items):
        top = first_candidates(blocks[i]) if i < len(blocks) else ""
        ok = top in it["expected_output"]
        hit += ok
        rows.append({"index": it["index"], "input": it["input"], "top1": top,
                     "expected": it["expected_output"], "ok": ok})
    print(f"Acc@1 {hit}/{len(items)} = {100 * hit / len(items):.1f}%  ({elapsed:.0f} 秒)")
    if times:
        print(f"AI の変換 中央値 {times[len(times) // 2]:.0f} ms、p95 {times[int(len(times) * 0.95)]:.0f} ms、"
              f"最大 {times[-1]:.0f} ms")
    if args.out:
        json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
