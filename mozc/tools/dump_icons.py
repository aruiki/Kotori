"""exe / dll に入っているアイコン資源を並べて PNG にする(アイコンの差し替えが効いているかを見る。docs/adr/0022)。

使い方: python mozc/tools/dump_icons.py <exe または dll> <出力.png>
"""
import ctypes, io, struct, sys
from ctypes import wintypes
from PIL import Image

k = ctypes.WinDLL("kernel32", use_last_error=True)
k.LoadLibraryExW.restype = wintypes.HMODULE
k.LoadLibraryExW.argtypes = [wintypes.LPCWSTR, wintypes.HANDLE, wintypes.DWORD]
k.FindResourceW.restype = ctypes.c_void_p
k.FindResourceW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p]
k.LoadResource.restype = ctypes.c_void_p
k.LoadResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
k.LockResource.restype = ctypes.c_void_p
k.LockResource.argtypes = [ctypes.c_void_p]
k.SizeofResource.argtypes = [wintypes.HMODULE, ctypes.c_void_p]
P = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HMODULE, ctypes.c_void_p, ctypes.c_void_p, wintypes.LPARAM)
k.EnumResourceNamesW.argtypes = [wintypes.HMODULE, ctypes.c_void_p, P, wintypes.LPARAM]

path, out = sys.argv[1], sys.argv[2]
h = k.LoadLibraryExW(path, None, 0x2 | 0x20)
ids = []
k.EnumResourceNamesW(h, 3, P(lambda m, t, n, l: ids.append(n) or True), 0)
imgs = []
for i in ids:
    r = k.FindResourceW(h, i, 3)
    data = ctypes.string_at(k.LockResource(k.LoadResource(h, r)), k.SizeofResource(h, r))
    # RT_ICON は .ico の中身 1 枚分。.ico の見出しを付けて PIL で読む。
    if data[:4] == b"\x89PNG":
        im = Image.open(io.BytesIO(data))
    else:
        w = struct.unpack("<i", data[4:8])[0]
        hdr = struct.pack("<HHH", 0, 1, 1) + struct.pack("<BBBBHHII", w & 255, w & 255, 0, 0, 1, 32, len(data), 22)
        im = Image.open(io.BytesIO(hdr + data))
    imgs.append(im.convert("RGBA"))
print(len(imgs), "icons", [im.size for im in imgs][:12])
sheet = Image.new("RGBA", (sum(min(im.width, 64) + 4 for im in imgs[:24]) + 4, 72), (200, 200, 200, 255))
x = 4
for im in imgs[:24]:
    t = im.resize((min(im.width, 64),) * 2)
    sheet.alpha_composite(t, (x, 4))
    x += t.width + 4
sheet.save(out)
