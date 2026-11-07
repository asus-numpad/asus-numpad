#!/usr/bin/env bash
# 从 CHANGELOG.md 里抽出某个版本的小节标题，生成几行 Release 说明。
# 正文（每组下面的细节）留在 CHANGELOG，Release 页面只给要点，避免上万字铺屏。
#
#   tools/release-notes.sh 1.0.1            # 输出到 stdout
#   tools/release-notes.sh 1.0.1 out.md     # 写入文件
set -euo pipefail
cd "$(dirname "$0")/.."

V="${1:?用法: tools/release-notes.sh <版本号> [输出文件]}"
OUT="${2:-}"
CL="CHANGELOG.md"

[ -f "$CL" ] || { echo "找不到 $CL" >&2; exit 1; }

# 取 "## $V" 到下一个 "## " 之间的 ### 标题
TITLES="$(awk -v v="$V" '
  /^## /{
    if (cur) exit
    if ($0 ~ "^## " v "([[:space:]]|$)") { cur = 1; next }
  }
  cur && /^### /{
    sub(/^###[[:space:]]*/, "")
    # 「保持不变」这类对照小节不是本版变更，不进 Release 说明
    if ($0 ~ /^(保持不变|不变)/) next
    print "- " $0
  }
' "$CL")"

{
  if [ -n "$TITLES" ]; then
    echo "$TITLES"
  else
    echo "- 详见 CHANGELOG"
  fi
  echo
  echo "完整变更见 [CHANGELOG.md](https://github.com/asus-numpad/asus-numpad/blob/main/CHANGELOG.md)。"
} > "${OUT:-/dev/stdout}"
