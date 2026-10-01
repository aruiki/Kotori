"""リリースの版(タグ)を決める(作業カード 40、docs/adr/0037 の段階)。

段階(channel)ごとに、既にあるタグから次の版を出す。
- beta: v<VERSION>-beta.N(N は既にある beta の次)。Latest として公開する
- rc: v<VERSION>-rc.N。プレリリース(Latest にしない)
- stable: v<VERSION>。Latest として公開する

v<VERSION>(正式版)が既にあれば、どの段階も失敗させる(同じ VERSION で出すと版が戻るため。
次は mozc/VERSION を上げる)。

使い方(ワークフローから): python mozc/tools/release_tag.py --channel rc [--repo URL] [--notes FILE]
  標準出力に TAG=…、PRERELEASE=…、MAKE_LATEST=… を書く($GITHUB_ENV にそのまま足せる)。
"""
import argparse
import pathlib
import re
import subprocess
import sys

CHANNELS = ("beta", "rc", "stable")


def next_release(base: str, channel: str, tags: list[str]) -> dict:
    """既にあるタグ(refs/tags/ を除いた名前)から、次のリリースの版と公開のしかたを決める。"""
    if channel not in CHANNELS:
        raise ValueError(f"段階は {', '.join(CHANNELS)} のどれか: {channel}")
    if not re.fullmatch(r"\d+\.\d+\.\d+", base):
        raise ValueError(f"mozc/VERSION は X.Y.Z の形にする: {base}")
    if f"v{base}" in tags:
        raise ValueError(f"v{base} は正式版として出ている。mozc/VERSION を上げる")
    if channel == "stable":
        return {"TAG": f"v{base}", "PRERELEASE": "false", "MAKE_LATEST": "true"}
    pattern = re.compile(rf"v{re.escape(base)}-{channel}\.(\d+)")
    nums = [int(m.group(1)) for t in tags if (m := pattern.fullmatch(t))]
    return {
        "TAG": f"v{base}-{channel}.{max(nums, default=0) + 1}",
        "PRERELEASE": "true" if channel == "rc" else "false",
        "MAKE_LATEST": "false" if channel == "rc" else "true",
    }


def check_notes(channel: str, notes: str) -> None:
    """正式版のリリースノートに、ベータ版の案内が残っていないか。"""
    if channel == "stable" and "ベータ版" in notes:
        raise ValueError("正式版のリリースノートに「ベータ版」が残っている(mozc/release-notes.md)")


def remote_tags(repo: str) -> list[str]:
    out = subprocess.run(["git", "ls-remote", "--tags", repo], capture_output=True, text=True,
                         check=True).stdout
    return [line.split("refs/tags/", 1)[1] for line in out.splitlines()
            if "refs/tags/" in line and not line.endswith("^{}")]


def main() -> int:
    # Windows のランナーではパイプの文字コードが cp1252 になり、日本語のメッセージで落ちるため。
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    root = pathlib.Path(__file__).resolve().parents[2]
    ap = argparse.ArgumentParser()
    ap.add_argument("--channel", default="beta", choices=CHANNELS)
    ap.add_argument("--repo", default="https://github.com/aruiki/KotoriIME-japanese-")
    ap.add_argument("--notes", default=str(root / "mozc" / "release-notes.md"))
    args = ap.parse_args()
    base = (root / "mozc" / "VERSION").read_text(encoding="utf-8").strip()
    try:
        check_notes(args.channel, pathlib.Path(args.notes).read_text(encoding="utf-8"))
        rel = next_release(base, args.channel, remote_tags(args.repo))
    except ValueError as e:
        print(e, file=sys.stderr)
        return 1
    for k, v in rel.items():
        print(f"{k}={v}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
