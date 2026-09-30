"""入力中の AI の計算量を測る(docs/adr/0024)。日常の文の読みを 1 文字ずつ間隔の秒おきに打ち
(入力中の候補)、打ち終わったら待ってから変換(Space)する。これを文の数だけ繰り返し、
AI の計算に使った時間(zenz と LLM の Score・生成。GPU の使用率の代わり)を、かかった時間で割る。

GPU の使用率はほかのアプリの描画が混ざって揺れるので、計算の時間を KOTORI_LM_STATS で数える。
計算は 1 本のスレッドで順に行うので、計算の時間 / かかった時間 が「AI が GPU を使っている割合」に当たる。

使い方: python mozc/tools/cost_bench.py [--interval 秒] [--pause 秒] [--n 文の数] [環境変数=値 ...]
  KOTORI_CONVERTER_MAIN、KOTORI_INSTALL_DIR は typing_test.py と同じ。
"""
import argparse
import json
import os
import re
import subprocess
import threading
import time

STATS = re.compile(r"\[stats\] zenz work=(\d+)ms decode=(\d+)ms calls=(\d+) tokens=(\d+) \| "
                   r"llm work=(\d+)ms decode=(\d+)ms calls=(\d+) tokens=(\d+)")


def kata_to_hira(s: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--interval", type=float, default=0.2, help="1 文字の間隔(秒)")
    ap.add_argument("--pause", type=float, default=0.8, help="打ち終わってから変換までの間(秒)")
    ap.add_argument("--n", type=int, default=12, help="文の数")
    ap.add_argument("--data", default="eval/sets/kotori-daily.json")
    ap.add_argument("env", nargs="*")
    args = ap.parse_args()

    exe = os.environ.get("KOTORI_CONVERTER_MAIN",
                         r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
    inst = os.environ.get("KOTORI_INSTALL_DIR", r"C:\Program Files (x86)\Kotori")
    env = dict(os.environ, KOTORI_RUNTIME_DIR=inst, KOTORI_MODEL_DIR=inst, KOTORI_LM_STATS="1")
    env.update(dict(a.split("=", 1) for a in args.env))
    items = json.load(open(args.data, encoding="utf-8"))[: args.n]
    readings = [kata_to_hira(it["input"]) for it in items]

    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         cwd=exe + ".runfiles/_main", env=env)
    out, err = [], []
    threading.Thread(target=lambda: [out.append(x) for x in p.stdout], daemon=True).start()
    threading.Thread(target=lambda: [err.append(x.decode("utf-8", "replace")) for x in p.stderr],
                     daemon=True).start()

    def send(cmd: str, wait: bool = True) -> float:
        start = len(out)
        t0 = time.time()
        p.stdin.write(f"{cmd}\nreset\nkotori_sep\n".encode("utf-8"))
        p.stdin.flush()
        while wait and time.time() - t0 < 30:
            if any(x.startswith(b"ExecCommand() return false") for x in out[start:]):
                break
            time.sleep(0.001)
        return (time.time() - t0) * 1000

    def stats():
        send("suggest あい")
        for line in reversed(err):
            m = STATS.search(line)
            if m:
                v = [int(x) for x in m.groups()]
                return v[0] + v[4], v
        return 0, [0] * 8

    # モデルの読み込み(最初の変換と予測)を済ませてから測る。
    send("start おはようございます")
    send("suggest おはよう")
    time.sleep(3)
    w0, v0 = stats()
    t0 = time.time()
    keys, lat = 0, []
    for r in readings:
        for n in range(1, len(r) + 1):
            lat.append(send(f"suggest {r[:n]}"))
            keys += 1
            time.sleep(args.interval)
        time.sleep(args.pause)
        send(f"start {r}")
    time.sleep(2)
    wall = (time.time() - t0) * 1000
    w1, v1 = stats()
    p.stdin.close()
    d = [b - a for a, b in zip(v0, v1)]
    lat.sort()
    print(f"文 {len(readings)}、打鍵 {keys}、{wall / 1000:.1f} 秒")
    print(f"AI の計算 {w1 - w0} ms = {100 * (w1 - w0) / wall:.1f}%(zenz {d[0]} ms、LLM {d[4]} ms)"
          f"  1 打鍵あたり {(w1 - w0) / keys:.0f} ms")
    print(f"  decode: zenz {d[1]} ms / {d[2]} 回 / {d[3]} トークン、"
          f"LLM {d[5]} ms / {d[6]} 回 / {d[7]} トークン")
    print(f"入力中の候補の応答 p50 {lat[len(lat) // 2]:.0f} ms、p95 {lat[int(len(lat) * 0.95)]:.0f} ms")


if __name__ == "__main__":
    main()
