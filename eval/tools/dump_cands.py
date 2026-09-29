"""Kotori(Mozc + zenz)の文節ごとの候補を JSON に書き出す。
使い方: python3 dump_cands.py <converter_main> <data.json> <out.json> [K]"""
import json, os, re, subprocess, sys
from pathlib import Path

SEG = re.compile(r"^-{10} Segment (\d+)/(\d+) \[.*\] -{10}$")
CAND = re.compile(r"^\s+(\d+)/\d+ (.*)$")


def kata_to_hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def main():
    exe, data, out = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
    k = int(sys.argv[4]) if len(sys.argv) > 4 else 8
    cwd = exe.parent / (exe.name + ".runfiles") / "_main"
    items = json.load(open(data, encoding="utf-8"))
    res = []
    for it in items:
        reading = kata_to_hira(it["input"])
        o = subprocess.run([str(exe.resolve())], input=f"start {reading}\nquit\n",
                           capture_output=True, text=True, cwd=cwd, timeout=120,
                           env={**os.environ}).stdout.splitlines()
        segs, cur = [], None
        for i, line in enumerate(o):
            if SEG.match(line):
                cur = {"key": o[i + 1].strip(), "cands": []}
                segs.append(cur)
            elif cur is not None:
                m = CAND.match(line)
                if m and int(m.group(1)) < k and m.group(2) not in cur["cands"]:
                    cur["cands"].append(m.group(2))
        res.append({"index": it["index"], "context": it.get("context_text", ""),
                    "segments": segs, "expected": it["expected_output"]})
    json.dump(res, open(out, "w", encoding="utf-8"), ensure_ascii=False)
    acc = sum("".join(s["cands"][0] for s in r["segments"]) in r["expected"] for r in res)
    print(f"base Acc@1 {acc}/{len(res)} = {100*acc/len(res):.1f}%")


main()
