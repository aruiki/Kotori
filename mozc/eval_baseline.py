#!/usr/bin/env python3
"""Mozc(//converter:converter_main)で AJIMEE-Bench などの Acc@1 を測る(docs/adr/0012 段階 3)。

使い方: python3 mozc/eval_baseline.py <converter_main> [--data 評価セット.json] [--out 結果.json] [--context]
  <converter_main> は bazel-bin/converter/converter_main(Windows は .exe)。データは runfiles の中にあるので、
  runfiles の _main を作業ディレクトリにして起動する。
1 回の起動で全問を解く(AI のモデルの読み込みは 1 回だけ)。第 1 候補は各文節の第 1 候補をつないだ文。
LM リランク(docs/adr/0013、0016)は環境変数 KOTORI_ZENZ_MODEL などで設定する(rewriter/lm_rewriter.h)。
--context は問題の前の文(context_text)を AI に渡す(アプリから直前の文を受け取った場合に当たる)。
前の文は「読み<TAB>前の文」のファイルを KOTORI_LM_CONTEXT_MAP で渡す。

converter_main が異常終了した、出力が問題の数に足りない、時間切れのときは失敗(終了コード 1)にする。
--min-acc を付けると、Acc@1 がそれ未満のときも失敗(終了コード 2)にする(回帰の検出用)。
--out を付けると、結果の隣に実行条件(<out>.manifest.json: データ・変換器・モデルの SHA-256、
KOTORI_ の環境変数、コミット、GPU)も残す(docs/IMPROVEMENT_PROPOSALS.md の案 11)。
"""
import argparse
import hashlib
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


def wilson(hit: int, n: int, z: float = 1.96) -> tuple:
    """Acc@1 の 95% 信頼区間(Wilson)。問題数が少ないと幅が広いので、差がこの幅より小さければ偶然と区別できない。"""
    if n == 0:
        return 0.0, 0.0
    p = hit / n
    d = 1 + z * z / n
    c = p + z * z / (2 * n)
    m = z * ((p * (1 - p) + z * z / (4 * n)) / n) ** 0.5
    return 100 * (c - m) / d, 100 * (c + m) / d


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def manifest(args, exe: Path, env: dict, acc: float, n: int) -> dict:
    """結果を再現するための実行条件。"""
    def run(cmd):
        try:
            return subprocess.run(cmd, capture_output=True, text=True, timeout=30).stdout.strip()
        except (OSError, subprocess.SubprocessError):
            return ""
    models = {}
    model_dir = env.get("KOTORI_MODEL_DIR")
    paths = [env.get("KOTORI_ZENZ_MODEL"), env.get("KOTORI_LLM_MODEL")]
    if model_dir and Path(model_dir).is_dir():
        paths += [str(p) for p in sorted(Path(model_dir).glob("*.gguf"))]
    for p in paths:
        if p and Path(p).is_file() and p not in models:
            models[p] = sha256(Path(p))
    return {
        "data": {"path": args.data, "sha256": sha256(Path(args.data)), "items": n},
        "context": args.context,
        "acc": acc,
        "converter_main": {"path": str(exe), "sha256": sha256(exe)},
        "models": models,
        "env": {k: v for k, v in sorted(env.items()) if k.startswith("KOTORI_")},
        "commit": run(["git", "rev-parse", "HEAD"]),
        "gpu": run(["nvidia-smi", "--query-gpu=name,driver_version", "--format=csv,noheader"]),
        "time": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("converter_main")
    ap.add_argument("--data", default="eval/data/ajimee-bench.json")
    ap.add_argument("--out", default="")
    ap.add_argument("--context", action="store_true", help="問題の context_text を前の文として AI に渡す")
    ap.add_argument("--timeout", type=float, default=36000, help="全体の制限時間(秒)")
    ap.add_argument("--min-acc", type=float, default=None, help="Acc@1(%%)がこれ未満なら失敗にする")
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
    try:
        proc = subprocess.run([str(exe)], input=script.encode("utf-8"), capture_output=True,
                              cwd=cwd, env=env, timeout=args.timeout)
    except subprocess.TimeoutExpired:
        print(f"失敗: {args.timeout:.0f} 秒で終わらなかった", file=sys.stderr)
        return 1
    except OSError as e:
        print(f"失敗: 変換器を起動できない: {e}", file=sys.stderr)
        return 1
    if proc.returncode != 0:
        print(f"失敗: 変換器が終了コード {proc.returncode} で終わった", file=sys.stderr)
        print(proc.stderr.decode("utf-8", "replace")[-2000:], file=sys.stderr)
        return 1
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
    if len(blocks) < len(items):
        print(f"失敗: 出力が {len(blocks)} 問分しかない(問題は {len(items)} 問)", file=sys.stderr)
        return 1
    rows, hit = [], 0
    for i, it in enumerate(items):
        top = first_candidates(blocks[i]) if i < len(blocks) else ""
        ok = top in it["expected_output"]
        hit += ok
        rows.append({"index": it["index"], "input": it["input"], "top1": top,
                     "expected": it["expected_output"], "ok": ok})
    lo, hi = wilson(hit, len(items))
    print(f"Acc@1 {hit}/{len(items)} = {100 * hit / len(items):.1f}%(95% 区間 {lo:.1f}〜{hi:.1f}%)  "
          f"({elapsed:.0f} 秒)")
    if times:
        print(f"AI の変換 中央値 {times[len(times) // 2]:.0f} ms、p95 {times[int(len(times) * 0.95)]:.0f} ms、"
              f"最大 {times[-1]:.0f} ms")
    # 分野(domain)のある評価セット(kotori-heldout)では、分野ごとの Acc@1 も出す。
    domains = {}
    for it, row in zip(items, rows):
        if "domain" in it:
            d = domains.setdefault(it["domain"], [0, 0])
            d[0] += row["ok"]
            d[1] += 1
    if domains:
        print("  " + "、".join(f"{k} {v[0]}/{v[1]}" for k, v in domains.items()))
    acc = 100 * hit / len(items)
    if args.out:
        json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
        json.dump(manifest(args, exe, env, acc, len(items)),
                  open(args.out + ".manifest.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    if args.min_acc is not None and acc < args.min_acc:
        print(f"失敗: Acc@1 {acc:.1f}% が基準 {args.min_acc:.1f}% を下回った", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
