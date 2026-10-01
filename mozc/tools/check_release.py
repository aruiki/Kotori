"""公開した MSI を確かめる(作業カード 35)。リリースの後に必ず回す。

GitHub Releases から `Kotori64.msi` と `.sha256` を取り、次を確かめる。1 つでも外れたら終了コード 1。
- SHA-256 が `.sha256` と一致する
- MSI の ProductVersion が、比べる版(既定は 1 つ前のリリース)より大きい(上書きで入れ替わるための条件)
- 展開した中身に、同梱するモデルと実行ファイルと第三者の表示がそろっていて、入れない物(通信の部品)がない
- `mozc_server.exe` のファイルの版が ProductVersion と一致する

使い方: python mozc/tools/check_release.py [タグ(既定は Latest)] [--work フォルダ]
       python mozc/tools/check_release.py --local <MSI>(手元でビルドした MSI の中身だけを確かめる)
  gh(GitHub CLI)と Windows の msiexec を使う。
"""
import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys

REPO = "aruiki/KotoriIME-japanese-"
BUNDLED = ["mozc_server.exe", "mozc_tool.exe", "mozc_renderer.exe", "mozc_tip64.dll", "llama.dll",
           "zenz-v2.5-small-q8_0.gguf", "zenz-v2.5-medium-q8_0.gguf", "tinyswallow-1.5b-q5_k_m.gguf",
           "NOTICE-zenz.txt", "NOTICE-tinyswallow.txt",
           # 第三者の表示(作業カード 41、REQ-16-1)
           "NOTICE-third-party.txt", "LICENSE-llama.cpp.txt", "LICENSE-llvm-openmp.txt", "LICENSE-GPL-3.0.txt",
           "documents/credits_en.html"]
# 入れない物。ggml-rpc.dll は別の PC に計算を任せる通信の部品(REQ-15-1、作業カード 41)。
ABSENT = ["ggml-rpc.dll"]


def run(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, check=True, **kw).stdout.strip()


def ps(script: str) -> str:
    return run(["powershell", "-NoProfile", "-Command", script])


def msi_version(msi: pathlib.Path) -> str:
    return ps(
        "$wi = New-Object -ComObject WindowsInstaller.Installer;"
        f"$db = $wi.GetType().InvokeMember('OpenDatabase','InvokeMethod',$null,$wi,@('{msi}',0));"
        "$v = $db.GetType().InvokeMember('OpenView','InvokeMethod',$null,$db,"
        "@(\"SELECT Value FROM Property WHERE Property='ProductVersion'\"));"
        "$v.GetType().InvokeMember('Execute','InvokeMethod',$null,$v,$null);"
        "$r = $v.GetType().InvokeMember('Fetch','InvokeMethod',$null,$v,$null);"
        "$r.GetType().InvokeMember('StringData','GetProperty',$null,$r,1)")


def download(tag: str, dest: pathlib.Path) -> pathlib.Path:
    dest.mkdir(parents=True, exist_ok=True)
    run(["gh", "release", "download", tag, "--repo", REPO, "--pattern", "Kotori64.msi*",
         "--dir", str(dest), "--clobber"])
    return dest / "Kotori64.msi"


def as_tuple(v: str):
    return tuple(int(x) for x in v.split("."))


class Checks:
    """確かめた結果を OK / NG で出し、1 つでも NG があったかを覚える。"""

    def __init__(self):
        self.ok = True

    def __call__(self, cond: bool, msg: str):
        print(("OK  " if cond else "NG  ") + msg)
        self.ok = self.ok and cond


def check_contents(msi: pathlib.Path, work: pathlib.Path, check: Checks):
    """MSI を展開して、同梱物・入れない物・実行ファイルの版を確かめる。"""
    ver = msi_version(msi)
    ext = work / "ext"
    shutil.rmtree(ext, ignore_errors=True)
    subprocess.run(["msiexec", "/a", str(msi), "/qn", f"TARGETDIR={ext}"], check=True)
    root = ext / "PFiles" / "Kotori"
    for name in BUNDLED:
        check((root / name).exists(), f"同梱: {name}")
    for name in ABSENT:
        check(not (root / name).exists(), f"入れない: {name}")
    fv = ps(f"(Get-Item '{root / 'mozc_server.exe'}').VersionInfo.FileVersion")
    check(fv == ver, f"mozc_server.exe の版 {fv} = ProductVersion")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("tag", nargs="?", default="")
    ap.add_argument("--work", default=str(pathlib.Path.home() / "kotori-dev" / "work" / "check-release"))
    ap.add_argument("--local", default="", help="手元の MSI の中身だけを確かめる(ハッシュと前の版は見ない)")
    args = ap.parse_args()
    check = Checks()
    work = pathlib.Path(args.work)
    if args.local:
        check_contents(pathlib.Path(args.local).resolve(), work, check)
    else:
        releases = json.loads(run(["gh", "release", "list", "--repo", REPO, "--limit", "10",
                                   "--json", "tagName,isLatest"]))
        tags = [r["tagName"] for r in releases]
        tag = args.tag or next(r["tagName"] for r in releases if r["isLatest"])
        prev = tags[tags.index(tag) + 1] if tags.index(tag) + 1 < len(tags) else ""
        shutil.rmtree(work, ignore_errors=True)
        msi = download(tag, work / tag)
        want = (work / tag / "Kotori64.msi.sha256").read_text().split()[0]
        got = hashlib.sha256(msi.read_bytes()).hexdigest()
        check(got == want, f"SHA-256 {got[:16]}…")
        ver = msi_version(msi)
        if prev:
            prev_ver = msi_version(download(prev, work / prev))
            check(as_tuple(ver)[:3] > as_tuple(prev_ver)[:3],
                  f"ProductVersion {ver} > {prev}({prev_ver})(先頭 3 つで比べる)")
        else:
            print(f"--  ProductVersion {ver}(比べる前の版がない)")
        check_contents(msi, work, check)
    print("すべて OK" if check.ok else "確かめに失敗した項目がある")
    return 0 if check.ok else 1


if __name__ == "__main__":
    sys.exit(main())
