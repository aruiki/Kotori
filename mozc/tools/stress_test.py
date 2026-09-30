"""負荷試験(docs/adr/0021)。いろいろな読み(1〜300 文字、記号・英数字・絵文字)で入力中の候補・変換・Tab を
交互に送り、落ちないか・遅れないかを見る。

使い方: python mozc/tools/stress_test.py [環境変数=値 ...]
  KOTORI_CONVERTER_MAIN: converter_main.exe の場所(bazel-bin/converter/converter_main.exe)
  KOTORI_INSTALL_DIR: モデルと llama.cpp の DLL のある所(既定はインストール先)
"""
import os
import random
import subprocess
import sys
import threading
import time

exe = os.environ.get("KOTORI_CONVERTER_MAIN", r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
cwd = exe + ".runfiles/_main"
inst = os.environ.get("KOTORI_INSTALL_DIR", r"C:\Program Files (x86)\Kotori")
env = dict(os.environ, KOTORI_RUNTIME_DIR=inst, KOTORI_MODEL_DIR=inst)
env.update(dict(a.split("=", 1) for a in sys.argv[1:]))
random.seed(1)
kana = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをんがぎぐげござじずぜぞだでどばびぶべぼぱぴぷぺぽゃゅょっー"
extra = ["、", "。", "！", "？", "「", "」", "1", "23", "abc", "A", "ｗ", "😀", " ", "・", "〜", "(", ")", "%", "\\"]
readings = []
for _ in range(120):
    n = random.choice([1, 2, 3, 5, 8, 15, 30, 60, 120, 250, 300])
    readings.append("".join(random.choice(kana) if random.random() > 0.12 else random.choice(extra)
                            for _ in range(n)))
readings += ["", "ー", "っっっ", "んんんんん", "ゃ", "12345678901234567890", "aaaaaaaaaaaaaaaaaaaa"]

p = subprocess.Popen([exe], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     cwd=cwd, env=env)
out = []
threading.Thread(target=lambda: [out.append(line) for line in p.stdout], daemon=True).start()
time.sleep(6)  # モデルの読み込み
lat = []
cmds = ["suggest", "start", "predict"]
for r in readings:
    for c in cmds:
        start = len(out)
        t0 = time.time()
        p.stdin.write(f"{c} {r}\nreset\nkotori_sep\n".encode("utf-8"))
        p.stdin.flush()
        while not any(line.startswith(b"ExecCommand() return false") for line in out[start:]):
            if time.time() - t0 > 20 or p.poll() is not None:
                break
            time.sleep(0.001)
        dt = (time.time() - t0) * 1000
        lat.append((dt, c, len(r)))
        if p.poll() is not None:
            print("CRASH exit", p.returncode, "at", c, repr(r[:40]))
            sys.exit(1)
        if dt > 20000:
            print("HANG", c, repr(r[:40]))
# converter_main は入力の終わり(EOF)で終わる(quit というコマンドはない)。
p.stdin.close()
p.wait(timeout=60)
print("exit", p.returncode, "commands", len(lat))
for c in cmds:
    xs = sorted(d for d, cc, _ in lat if cc == c)
    print(f"{c:8s} p50 {xs[len(xs) // 2]:.0f} ms  p95 {xs[int(len(xs) * 0.95)]:.0f} ms  max {xs[-1]:.0f} ms")
for d, c, n in sorted(lat, reverse=True)[:5]:
    print(f"  slow {c} len={n} {d:.0f} ms")
