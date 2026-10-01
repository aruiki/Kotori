"""入力がないときにモデルを外して VRAM を空けるか(docs/adr/0034)を確かめる。

converter_main を KOTORI_LM_IDLE_UNLOAD=秒 で動かし、変換してから待つ。待つ前・外した後・入力し直した後の
VRAM(nvidia-smi の GPU 全体の使用量)と、入力し直した後の最初の変換が AI を使えたか、何 ms かかったかを出す。

使い方: python mozc/tools/idle_test.py [--idle 秒] [環境変数=値 ...]
  KOTORI_CONVERTER_MAIN、KOTORI_INSTALL_DIR は typing_test.py と同じ。NVIDIA の GPU が要る。
"""
import argparse
import os
import subprocess
import threading
import time


def vram_mb(pid: int) -> int:
    # Windows(WDDM)ではプロセスごとの使用量が出ないので、GPU 全体の使用量を見る(ほかのアプリで揺れる)。
    out = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                         capture_output=True, text=True).stdout
    return int(out.split()[0]) if out.strip() else 0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--idle", type=int, default=40, help="KOTORI_LM_IDLE_UNLOAD(秒)")
    ap.add_argument("env", nargs="*")
    args = ap.parse_args()
    exe = os.environ.get("KOTORI_CONVERTER_MAIN",
                         r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
    inst = os.environ.get("KOTORI_INSTALL_DIR", r"C:\Program Files (x86)\Kotori")
    env = dict(os.environ, KOTORI_RUNTIME_DIR=inst, KOTORI_MODEL_DIR=inst,
               KOTORI_LM_IDLE_UNLOAD=str(args.idle))
    env.update(dict(a.split("=", 1) for a in args.env))
    p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                         cwd=exe + ".runfiles/_main", env=env)
    out = []
    threading.Thread(target=lambda: [out.append(x.decode("utf-8", "replace")) for x in p.stdout],
                     daemon=True).start()

    def send(cmd: str) -> tuple[float, str]:
        start = len(out)
        t0 = time.time()
        p.stdin.write(f"{cmd}\nreset\nkotori_sep\n".encode("utf-8"))
        p.stdin.flush()
        while time.time() - t0 < 30:
            if any(x.startswith("ExecCommand() return false") for x in out[start:]):
                break
            time.sleep(0.001)
        ms = (time.time() - t0) * 1000
        first = next((x for x in out[start:] if "value:" in x), "")
        return ms, first.strip()

    reading = "きょうはいいてんきですね"
    send(f"start {reading}")
    time.sleep(3)
    ms, top = send(f"start {reading}")
    print(f"読み込み後の変換 {ms:.0f} ms  {top}  VRAM {vram_mb(p.pid)} MB")
    for t in range(args.idle + 40):
        time.sleep(1)
        if t % 10 == 9:
            print(f"  {t + 1} 秒後 VRAM {vram_mb(p.pid)} MB")
    ms, top = send(f"suggest きょう")
    print(f"入力し直した最初の打鍵 {ms:.0f} ms")
    ms, top = send(f"start {reading}")
    print(f"すぐ変換 {ms:.0f} ms  {top}  VRAM {vram_mb(p.pid)} MB")
    time.sleep(3)
    ms, top = send(f"start {reading}")
    print(f"3 秒後の変換 {ms:.0f} ms  {top}  VRAM {vram_mb(p.pid)} MB")
    p.stdin.close()


if __name__ == "__main__":
    main()
