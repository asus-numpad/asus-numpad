#!/usr/bin/env python3
"""把 asus-numpad 标识符全量改成 numdeck。

只替换确定的标识符串，不碰硬件型号名（NumberPad / ASUS 作为厂商名照旧）。
排除 repo/（历史发布产物，文件名与签名不能动）与 site/assets 下的二进制。
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 长串在前，避免 "asus-numpad-gui" 被 "asus-numpad" 先吃掉
RULES = [
    ("asus-numpad-gui", "numdeck-gui"),
    ("asus_numpad_gui", "numdeck_gui"),
    ("ASUS_NUMPAD_GUI", "NUMDECK_GUI"),
    ("asus-numpad.github.io", "numdeck-dev.github.io"),
    ("asus-numpad/asus-numpad", "numdeck-dev/numdeck"),
    ("asus-numpad-rs", "numdeck"),
    ("asus-numpad", "numdeck"),
    ("asus_numpad", "numdeck"),
    ("ASUS_NUMPAD", "NUMDECK"),
    ("asus-numpad", "numdeck"),  # 上面的 -rs 规则替换后可能再次产出的兜底
]

TEXT_EXT = {
    ".rs", ".toml", ".lock", ".md", ".sh", ".nix", ".spec", ".ebuild",
    ".rules", ".service", ".install", ".conf", ".yml", ".yaml", ".json",
    ".html", ".css", ".js", ".lang", ".1", ".in", ".txt", ".xml", ".py",
    ".svg", ".webmanifest", ".png.txt",
}
TEXT_NAME = {
    "PKGBUILD", "APKBUILD", "PKGBUILD.in", "postinst", "prerm", "postrm",
    "pre-install", "post-install", "post-install", "install.sh", "uninstall.sh", ".nojekyll",
}
SKIP_DIRS = {"target", ".git", "repo", "dist", "node_modules"}
# 历史文档保留旧名（记录当时确实叫 asus-numpad），只在新条目里用 numdeck
SKIP_FILES = {"CHANGELOG.md", "docs/release-notes/1.0.1.md",
              "docs/release-notes/1.0.2.md", "docs/release-notes/1.0.3.md"}


def is_text(path: str) -> bool:
    base = os.path.basename(path)
    if base in TEXT_NAME:
        return True
    ext = os.path.splitext(base)[1].lower()
    return ext in TEXT_EXT


def main() -> int:
    apply = "--apply" in sys.argv
    files = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.split()
    changed = []
    for rel in files:
        parts = rel.split("/")
        if parts[0] in SKIP_DIRS or rel in SKIP_FILES:
            continue
        path = os.path.join(ROOT, rel)
        if not os.path.isfile(path) or not is_text(rel):
            continue
        try:
            src = open(path, encoding="utf-8").read()
        except (UnicodeDecodeError, OSError):
            continue
        out = src
        for old, new in RULES:
            out = out.replace(old, new)
        if out != src:
            hits = sum(src.count(old) for old, _ in RULES)
            changed.append((rel, hits))
            if apply:
                open(path, "w", encoding="utf-8").write(out)
    total = sum(h for _, h in changed)
    print(f"{'已改写' if apply else '将改写'} {len(changed)} 个文件，{total} 处")
    for rel, hits in changed:
        print(f"  {hits:4d}  {rel}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
