"""公開した MSI を確かめる(作業カード 35)。リリースの後に必ず回す。

GitHub Releases から `Kotori64.msi` と `.sha256` を取り、次を確かめる。1 つでも外れたら終了コード 1。
- SHA-256 が `.sha256` と一致する
- MSI の大きさが 1.8 GiB 以下(GitHub Releases の 1 ファイルの上限 2 GiB の手前で気づく。カード 40)
- MSI の ProductVersion が、比べる版(既定は 1 つ前のリリース)より大きい(上書きで入れ替わるための条件)
- 展開した中身に、同梱するモデルと実行ファイルがそろっている
- `mozc_server.exe` のファイルの版が ProductVersion と一致する

使い方: python mozc/tools/check_release.py [タグ(既定は Latest)] [--work フォルダ]
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
           "NOTICE-zenz.txt", "NOTICE-tinyswallow.txt"]
MAX_MSI_BYTES = int(1.8 * 2**30)


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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("tag", nargs="?", default="")
    ap.add_argument("--work", default=str(pathlib.Path.home() / "kotori-dev" / "work" / "check-release"))
    args = ap.parse_args()
    releases = json.loads(run(["gh", "release", "list", "--repo", REPO, "--limit", "10",
                               "--json", "tagName,isLatest"]))
    tags = [r["tagName"] for r in releases]
    tag = args.tag or next(r["tagName"] for r in releases if r["isLatest"])
    prev = tags[tags.index(tag) + 1] if tags.index(tag) + 1 < len(tags) else ""
    work = pathlib.Path(args.work)
    shutil.rmtree(work, ignore_errors=True)
    ok = True

    def check(cond: bool, msg: str):
        nonlocal ok
        print(("OK  " if cond else "NG  ") + msg)
        ok = ok and cond

    msi = download(tag, work / tag)
    want = (work / tag / "Kotori64.msi.sha256").read_text().split()[0]
    got = hashlib.sha256(msi.read_bytes()).hexdigest()
    check(got == want, f"SHA-256 {got[:16]}…")
    size = msi.stat().st_size
    check(size <= MAX_MSI_BYTES, f"大きさ {size / 2**30:.2f} GiB ≦ {MAX_MSI_BYTES / 2**30:.1f} GiB(上限 2 GiB)")
    ver = msi_version(msi)
    if prev:
        prev_ver = msi_version(download(prev, work / prev))
        check(as_tuple(ver)[:3] > as_tuple(prev_ver)[:3],
              f"ProductVersion {ver} > {prev}({prev_ver})(先頭 3 つで比べる)")
    else:
        print(f"--  ProductVersion {ver}(比べる前の版がない)")
    ext = work / "ext"
    subprocess.run(["msiexec", "/a", str(msi), "/qn", f"TARGETDIR={ext}"], check=True)
    root = ext / "PFiles" / "Kotori"
    for name in BUNDLED:
        check((root / name).exists(), f"同梱: {name}")
    fv = ps(f"(Get-Item '{root / 'mozc_server.exe'}').VersionInfo.FileVersion")
    check(fv == ver, f"mozc_server.exe の版 {fv} = ProductVersion")
    print("すべて OK" if ok else "確かめに失敗した項目がある")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
