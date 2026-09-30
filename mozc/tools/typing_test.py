"""打鍵を再現して入力中の候補を確かめる(docs/adr/0019)。1 文字ずつ間隔の秒おきに suggest を送り、
候補(上位 3 件)と応答時間を出す。AI の予測は、1 打鍵前の読みで裏で作ったものが出る。

使い方: python mozc/tools/typing_test.py [間隔の秒(既定 0.15)] [環境変数=値 ...]
  KOTORI_CONVERTER_MAIN、KOTORI_INSTALL_DIR は stress_test.py と同じ。
"""
import os
import subprocess
import sys
import threading
import time

exe = os.environ.get("KOTORI_CONVERTER_MAIN", r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
cwd = exe + ".runfiles/_main"
inst = os.environ.get("KOTORI_INSTALL_DIR", r"C:\Program Files (x86)\Kotori")
interval = float(sys.argv[1]) if len(sys.argv) > 1 else 0.15
env = dict(os.environ, KOTORI_RUNTIME_DIR=inst, KOTORI_MODEL_DIR=inst)
env.update(dict(a.split("=", 1) for a in sys.argv[2:]))
p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     cwd=cwd, env=env)
out = []
threading.Thread(target=lambda: [out.append(line.decode("utf-8", "replace").rstrip()) for line in p.stdout],
                 daemon=True).start()
time.sleep(6)  # モデルの読み込み
for word in ["おせわになっております", "よろしくおねがいいたします", "きょうはいいてんきですね"]:
    print(f"== {word}")
    for n in range(2, len(word) + 1):
        typed = word[:n]
        start = len(out)
        t0 = time.time()
        p.stdin.write(f"suggest {typed}\nreset\nkotori_sep\n".encode("utf-8"))
        p.stdin.flush()
        while not any(line.startswith("ExecCommand() return false") for line in out[start:]) and time.time() - t0 < 10:
            time.sleep(0.001)
        dt = (time.time() - t0) * 1000
        cands = []
        for line in out[start:]:
            parts = line.strip().split(" ", 1)
            if line.startswith("  ") and len(parts) == 2 and "/" in parts[0] and parts[0].split("/")[0].isdigit():
                cands.append(parts[1])
        print(f"  {typed:<14} {dt:5.0f} ms  {cands[:3]}")
        time.sleep(interval)
p.stdin.close()
