"""品質 × 機器(GPU / CPU)ごとの性能表を作る(docs/IMPROVEMENT_PROPOSALS.md の案 09)。

変換器(converter_main)を品質ごとに起動し、日常の文(`eval/sets/kotori-daily.json`)を 1 問ずつ変換して、
モデルを読み込むまでの時間、変換の応答(中央値 / p95 / 最大)、メモリ(ワーキングセットの最大)、VRAM の増分
(nvidia-smi。GPU のとき)を Markdown の表で出す。

使い方: python mozc/tools/perf_table.py [--devices gpu,cpu] [--n 文の数]
  KOTORI_CONVERTER_MAIN、KOTORI_INSTALL_DIR は typing_test.py と同じ。Low の zenz-v2.5-medium は
  KOTORI_LOW_ZENZ(既定はインストール先)。
"""
import argparse
import json
import os
import subprocess
import threading
import time

QUALITIES = {
    # rewriter/lm_rewriter.cc の QualityOf と同じ
    "Low": {"KOTORI_LM_BEAMS": "2", "KOTORI_LM_LLM_TOP": "0", "KOTORI_LM_K": "4"},
    "Standard": {"KOTORI_LM_BEAMS": "4", "KOTORI_LM_LLM_TOP": "4", "KOTORI_LM_K": "8"},
    "High": {"KOTORI_LM_BEAMS": "8", "KOTORI_LM_LLM_TOP": "32", "KOTORI_LM_K": "16"},
}


def kata_to_hira(s: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def vram_used() -> int:
    try:
        out = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                             capture_output=True, text=True, timeout=10).stdout
        return int(out.split()[0])
    except (OSError, ValueError, IndexError, subprocess.SubprocessError):
        return -1


def working_set_mb(pid: int) -> float:
    out = subprocess.run(["powershell", "-NoProfile", "-Command",
                          f"(Get-Process -Id {pid}).PeakWorkingSet64"],
                         capture_output=True, text=True, timeout=30).stdout.strip()
    try:
        return int(out) / 1e6
    except ValueError:
        return -1


def measure(exe: str, env: dict, readings: list) -> dict:
    v0 = vram_used()
    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, cwd=exe + ".runfiles/_main", env=env)
    out = []
    threading.Thread(target=lambda: [out.append(x) for x in p.stdout], daemon=True).start()

    def send(cmd: str) -> float:
        start = len(out)
        t0 = time.time()
        p.stdin.write(f"{cmd}\nreset\nkotori_sep\n".encode("utf-8"))
        p.stdin.flush()
        while time.time() - t0 < 60:
            if any(x.startswith(b"ExecCommand() return false") for x in out[start:]):
                break
            time.sleep(0.001)
        return (time.time() - t0) * 1000

    load = send("start おはようございます")  # 最初の変換はモデルの読み込みを含む
    lat = sorted(send(f"start {r}") for r in readings)
    ws = working_set_mb(p.pid)
    v1 = vram_used()
    p.stdin.close()
    p.wait(timeout=30)
    return {"load": load, "p50": lat[len(lat) // 2], "p95": lat[int(len(lat) * 0.95)],
            "max": lat[-1], "ws": ws, "vram": (v1 - v0) if v0 >= 0 and v1 >= 0 else -1}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--devices", default="gpu,cpu")
    ap.add_argument("--n", type=int, default=40)
    ap.add_argument("--data", default="eval/sets/kotori-daily.json")
    args = ap.parse_args()
    exe = os.environ.get("KOTORI_CONVERTER_MAIN",
                         r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
    inst = os.environ.get("KOTORI_INSTALL_DIR", r"C:\Program Files (x86)\Kotori")
    low_zenz = os.environ.get("KOTORI_LOW_ZENZ", os.path.join(inst, "zenz-v2.5-medium-q8_0.gguf"))
    readings = [kata_to_hira(it["input"]) for it in
                json.load(open(args.data, encoding="utf-8"))[: args.n]]
    print("| 機器 | 品質 | 最初の変換(読み込みを含む) | 変換 中央値 / p95 / 最大 | メモリ(最大) | VRAM の増分 |")
    print("| --- | --- | ---: | --- | ---: | ---: |")
    for device in args.devices.split(","):
        for name, q in QUALITIES.items():
            if device == "cpu" and name != "Low":
                continue  # GPU がなければ、どの品質でも Low で動く(docs/adr/0024)
            env = dict(os.environ, KOTORI_RUNTIME_DIR=inst, KOTORI_MODEL_DIR=inst,
                       KOTORI_LM_PRELOAD="0", **q)
            if name == "Low":
                env["KOTORI_ZENZ_MODEL"] = low_zenz.replace("\\", "/")
            if device == "cpu":
                env["KOTORI_LM_DEVICE"] = "cpu"
            r = measure(exe, env, readings)
            vram = f"{r['vram']} MB" if r["vram"] >= 0 and device == "gpu" else "-"
            print(f"| {device.upper()} | {name} | {r['load']:.0f} ms | {r['p50']:.0f} / {r['p95']:.0f} / "
                  f"{r['max']:.0f} ms | {r['ws']:.0f} MB | {vram} |", flush=True)


if __name__ == "__main__":
    main()
