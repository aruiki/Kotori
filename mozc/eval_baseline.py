#!/usr/bin/env python3
"""Mozc(//converter:converter_main)で AJIMEE-Bench の Acc@1 を測る(docs/adr/0012 段階 3)。

使い方: python3 mozc/eval_baseline.py <converter_main> [--out report.json] [--context]
  <converter_main> は bazel-bin/converter/converter_main。データは runfiles の中にあるので、
  runfiles の _main を作業ディレクトリにして起動する。
第 1 候補は、各文節の第 1 候補をつないだ文。左文脈は使わない(Rust 版の M1 と同じ条件)。
LM リランク(docs/adr/0013)は環境変数 KOTORI_ZENZ_MODEL などで有効にする(lm_rewriter.h)。
--context は問題の前の文(context_text)を AI に渡す(アプリから直前の文を受け取った場合に当たる)。
"""
import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path


def kata_to_hira(s: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


SEG = re.compile(r"^-{10} Segment \d+/\d+ \[.*\] -{10}$")
CAND = re.compile(r"^\s+0/\d+ (.*)$")


def run(exe: Path, cwd: Path, script: str, env=None) -> str:
    return subprocess.run(
        [str(exe.resolve())], input=script, capture_output=True, text=True,
        cwd=cwd, timeout=120, env={**os.environ, **(env or {})},
    ).stdout


def first_candidates(out: str) -> str:
    parts, want = [], False
    for line in out.splitlines():
        if SEG.match(line):
            want = True
        elif want:
            m = CAND.match(line)
            if m:
                parts.append(m.group(1))
                want = False
    return "".join(parts)


def convert(exe: Path, cwd: Path, reading: str, context: str = "") -> str:
    # 前の文は、アプリが渡す直前の文(preceding_text)の代わりに環境変数で渡す(lm_rewriter.cc)。
    env = {"KOTORI_LM_PRECEDING": context} if context else None
    return first_candidates(run(exe, cwd, f"start {reading}\nquit\n", env))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("converter_main")
    ap.add_argument("--data", default="eval/data/ajimee-bench.json")
    ap.add_argument("--out", default="")
    ap.add_argument("--context", action="store_true", help="問題の context_text を前の文として AI に渡す")
    args = ap.parse_args()

    exe = Path(args.converter_main)
    cwd = exe.parent / (exe.name + ".runfiles") / "_main"
    items = json.load(open(args.data, encoding="utf-8"))
    rows, hit = [], 0
    for it in items:
        top = convert(exe, cwd, kata_to_hira(it["input"]), it.get("context_text", "") if args.context else "")
        ok = top in it["expected_output"]
        hit += ok
        rows.append({"index": it["index"], "input": it["input"], "top1": top,
                     "expected": it["expected_output"], "ok": ok})
    print(f"Acc@1 {hit}/{len(items)} = {100 * hit / len(items):.1f}%")
    if args.out:
        json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
