"""起動したアプリのウィンドウだけを PrintWindow で PNG にする(画面の他の部分は撮らない。docs/adr/0022)。
設定画面の見た目を確かめるのに使う。MSI を `msiexec /a <msi> /qn TARGETDIR=<フォルダ>` で展開すると、
インストールせずに mozc_tool.exe を取り出せる。

使い方: python mozc/tools/capture_window.py <exe> <引数> <出力.png> [待つ秒]
  例: python mozc/tools/capture_window.py <展開先>/PFiles/Kotori/mozc_tool.exe --mode=config_dialog cfg.png
"""
import ctypes, subprocess, sys, time
from ctypes import wintypes
from PIL import Image

user32, gdi32 = ctypes.windll.user32, ctypes.windll.gdi32
user32.SetProcessDPIAware()
exe, arg, out = sys.argv[1], sys.argv[2], sys.argv[3]
wait = float(sys.argv[4]) if len(sys.argv) > 4 else 5
p = subprocess.Popen([exe] + ([arg] if arg else []))
time.sleep(wait)

found = []
EnumProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)


def cb(h, _):
    pid = wintypes.DWORD()
    user32.GetWindowThreadProcessId(h, ctypes.byref(pid))
    if pid.value == p.pid and user32.IsWindowVisible(h):
        r = wintypes.RECT()
        user32.GetWindowRect(h, ctypes.byref(r))
        if r.right - r.left > 100:
            found.append((h, r))
    return True


user32.EnumWindows(EnumProc(cb), 0)
if not found:
    print("no window"); p.kill(); sys.exit(1)
h, r = found[0]
w, hh = r.right - r.left, r.bottom - r.top
hdc = user32.GetWindowDC(h)
mdc = gdi32.CreateCompatibleDC(hdc)
bmp = gdi32.CreateCompatibleBitmap(hdc, w, hh)
gdi32.SelectObject(mdc, bmp)
user32.PrintWindow(h, mdc, 2)


class BMI(ctypes.Structure):
    _fields_ = [("biSize", wintypes.DWORD), ("biWidth", wintypes.LONG), ("biHeight", wintypes.LONG),
                ("biPlanes", wintypes.WORD), ("biBitCount", wintypes.WORD), ("biCompression", wintypes.DWORD),
                ("biSizeImage", wintypes.DWORD), ("biXPelsPerMeter", wintypes.LONG),
                ("biYPelsPerMeter", wintypes.LONG), ("biClrUsed", wintypes.DWORD), ("biClrImportant", wintypes.DWORD)]


bi = BMI(ctypes.sizeof(BMI), w, -hh, 1, 32, 0, 0, 0, 0, 0, 0)
buf = ctypes.create_string_buffer(w * hh * 4)
gdi32.GetDIBits(mdc, bmp, 0, hh, buf, ctypes.byref(bi), 0)
Image.frombuffer("RGBA", (w, hh), buf, "raw", "BGRA", 0, 1).convert("RGB").save(out)
gdi32.DeleteObject(bmp); gdi32.DeleteDC(mdc); user32.ReleaseDC(h, hdc)
print("saved", w, hh)
p.kill()
