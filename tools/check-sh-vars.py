#!/usr/bin/env python3
"""拦住 `$VAR` 后面紧跟中文标点的写法。

bash 的变量名只认 ASCII，但 `$BIN（先在仓库根目录…）` 这种写法在 UTF-8 环境下会把
全角括号的字节啃进变量名：普通模式输出一串乱码，`set -u` 下直接
`BIN?: unbound variable` 退出。脚本正文全是中文，这种位置很容易顺手写出来，
所以拿个静态检查兜住——一旦出现就要求写成 `${VAR}`。

用法：python3 tools/check-sh-vars.py [文件…]（默认扫仓库里的 sh / PKGBUILD / APKBUILD）
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PATTERN = re.compile(r"\$([A-Za-z_][A-Za-z0-9_]*)([^\x00-\x7f])")
SKIP_DIRS = {".git", "target", "node_modules", "dist"}


def shell_files(explicit: list[str]) -> list[Path]:
    if explicit:
        return [Path(p) for p in explicit]
    found: list[Path] = []
    for path in ROOT.rglob("*"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.is_file() and (path.suffix == ".sh" or path.name in {"PKGBUILD", "APKBUILD"}):
            found.append(path)
    return sorted(found)


def main(argv: list[str]) -> int:
    bad = 0
    files = shell_files(argv)
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        shown = path if path.is_relative_to(ROOT) else path
        try:
            shown = path.relative_to(ROOT)
        except ValueError:
            shown = path
        for lineno, line in enumerate(text.splitlines(), 1):
            for match in PATTERN.finditer(line):
                bad += 1
                name, nxt = match.group(1), match.group(2)
                print(
                    f"{shown}:{lineno}: "
                    f"`${name}` 后面紧跟非 ASCII 字符 {nxt!r}，写成 ${{{name}}}"
                )
                print(f"    {line.strip()[:120]}")
    print(f"扫描 {len(files)} 个脚本，问题 {bad} 处")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
