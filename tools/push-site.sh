#!/bin/sh
# 把仓库内的 site/ 发布到组织根域名站点 numdeck-dev.github.io。
# 站点源码只存在本仓库 site/ 一处；该脚本负责同步到承载 Pages 的仓库。
# 需要 ~/.ssh/config 里的 github.com-numdeck-org（部署密钥，仅站点仓库可写）。
set -eu

REMOTE="${SITE_REMOTE:-git@github.com-numdeck-org:numdeck-dev/numdeck-dev.github.io.git}"
BRANCH="${SITE_BRANCH:-main}"

root=$(git rev-parse --show-toplevel)
src="$root/site"
[ -f "$src/index.html" ] || { echo "site/index.html 不存在" >&2; exit 1; }

work=$(mktemp -d)
trap 'command rm -rf "$work"' EXIT

git clone --quiet --depth 1 --branch "$BRANCH" "$REMOTE" "$work/site"

# 清空站点仓库工作区（.git 除外），再整体覆盖为 site/ 的内容
find "$work/site" -mindepth 1 -maxdepth 1 ! -name .git -exec command rm -rf {} +
# 整体覆盖为 site/ 的全部内容（含 .nojekyll 这类隐藏文件）：
# 以后新增页面或资源不必再改这份白名单
cp -R "$src"/. "$work/site"/
find "$work/site" -name '.DS_Store' -delete

cd "$work/site"
git add -A
if git diff --cached --quiet; then
    echo "站点无变化"
    exit 0
fi
git -c user.name="numdeck" -c user.email="bot@numdeck-dev.github.io" \
    commit --quiet -m "site: 同步自 numdeck-dev/numdeck $(git -C "$root" rev-parse --short HEAD)"
git push --quiet origin "$BRANCH"
echo "已推送：" "$(git rev-parse --short HEAD)"
