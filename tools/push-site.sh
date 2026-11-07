#!/bin/sh
# 把仓库内的 site/ 发布到组织根域名站点 asus-numpad.github.io。
# 站点源码只存在本仓库 site/ 一处；该脚本负责同步到承载 Pages 的仓库。
# 需要 ~/.ssh/config 里的 github.com-asus-numpad-org（部署密钥，仅站点仓库可写）。
set -eu

REMOTE="${SITE_REMOTE:-git@github.com-asus-numpad-org:asus-numpad/asus-numpad.github.io.git}"
BRANCH="${SITE_BRANCH:-main}"

root=$(git rev-parse --show-toplevel)
src="$root/site"
[ -f "$src/index.html" ] || { echo "site/index.html 不存在" >&2; exit 1; }

work=$(mktemp -d)
trap 'command rm -rf "$work"' EXIT

git clone --quiet --depth 1 --branch "$BRANCH" "$REMOTE" "$work/site"

# 清空站点仓库工作区（.git 除外），再整体覆盖为 site/ 的内容
find "$work/site" -mindepth 1 -maxdepth 1 ! -name .git -exec command rm -rf {} +
cp -R "$src/index.html" "$src/.nojekyll" "$src/assets" "$work/site/"

cd "$work/site"
git add -A
if git diff --cached --quiet; then
    echo "站点无变化"
    exit 0
fi
git -c user.name="asus-numpad" -c user.email="bot@asus-numpad.github.io" \
    commit --quiet -m "site: 同步自 asus-numpad/asus-numpad $(git -C "$root" rev-parse --short HEAD)"
git push --quiet origin "$BRANCH"
echo "已推送：" "$(git rev-parse --short HEAD)"
