#!/usr/bin/env bash
# 渲染可提交 AUR 的 PKGBUILD（+ 有 makepkg 时顺带生成 .SRCINFO）。
#
# 用法：
#   bash ./packaging/aur/gen-aur.sh --version 1.0.0 --out auraur
#   bash ./packaging/aur/gen-aur.sh --version 1.0.0 --sha256 <hash> --out auraur
#
# 不给 --sha256 时会现下载 GitHub 的 tag tarball 算一个，所以要求该 tag 已存在。
# CI 里由 release.yml 的 aur job 调用；本地手动发 AUR 也用同一个入口。
set -euo pipefail

REPO_URL="https://github.com/asus-numpad/asus-numpad"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TPL="${HERE}/PKGBUILD.in"
INSTALL_FILE="${HERE}/asus-numpad-rs.install"

die() { echo "gen-aur: $*" >&2; exit 1; }
log() { echo "gen-aur: $*"; }

VERSION=""
SHA256=""
OUT=""
SOURCE_URL=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="${2:?}"; shift 2 ;;
        --sha256)  SHA256="${2:?}"; shift 2 ;;
        --out)     OUT="${2:?}"; shift 2 ;;
        --source-url) SOURCE_URL="${2:?}"; shift 2 ;;
        -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
        *) die "未知参数 $1" ;;
    esac
done

[[ -n "${VERSION}" ]] || die "缺少 --version"
[[ "${VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "版本号必须是 x.y.z，收到 ${VERSION}"
[[ -f "${TPL}" ]] || die "找不到模板 ${TPL}"
OUT="${OUT:-$PWD/aur-out}"
# 默认取 GitHub 的 tag archive；GitHub 不保证 archive 的校验和永久不变，
# 真遇上漂移就用 --source-url 指向别处（比如自己 Release 里的源码包）重发一次。
SOURCE_URL="${SOURCE_URL:-${REPO_URL}/archive/v${VERSION}.tar.gz}"
mkdir -p "${OUT}"

if [[ -z "${SHA256}" ]]; then
    log "下载源码包计算校验和：${SOURCE_URL}"
    # 用显式模板：GNU mktemp 的 -t 要求模板自带至少三个 X，macOS 的 -t 则是
    # 直接给名字，两边不通用，CI 上一版就死在 "too few X's in template"。
    tmp="$(mktemp "${TMPDIR:-/tmp}/asus-numpad-aur.XXXXXX")"
    trap 'rm -f "$tmp"' EXIT
    # --retry-all-errors：tag 刚推上去时 archive 会短暂 404，普通 --retry 不重试 404
    if ! curl -fsSL --retry 6 --retry-delay 5 --retry-all-errors -o "$tmp" "${SOURCE_URL}"; then
        code="$(curl -sL -o /dev/null -w '%{http_code}' "${SOURCE_URL}" || true)"
        die "下载失败（HTTP ${code}）：${SOURCE_URL}"
    fi
    if command -v sha256sum >/dev/null 2>&1; then
        SHA256="$(sha256sum "$tmp" | awk '{print $1}')"
    else
        SHA256="$(shasum -a 256 "$tmp" | awk '{print $1}')"
    fi
fi
[[ "${SHA256}" =~ ^[0-9a-f]{64}$ ]] || die "校验和格式不对：${SHA256}"

# AUR 规范要求 Maintainer 行带真实联系方式，CI 里用提交者，本地用 git 配置兜底
MAINTAINER="${AUR_MAINTAINER:-}"
if [[ -z "${MAINTAINER}" ]]; then
    name="$(git config user.name 2>/dev/null || true)"
    mail="$(git config user.email 2>/dev/null || true)"
    if [[ -n "$name" && -n "$mail" ]]; then
        MAINTAINER="$name <$mail>"
    else
        MAINTAINER="RunningVAN <runningvan@users.noreply.github.com>"
    fi
fi

sed -e "s|@PKGVER@|${VERSION}|g" \
    -e "s|@SHA256@|${SHA256}|g" \
    -e "s|@SOURCE@|${SOURCE_URL}|g" \
    -e "s|@MAINTAINER@|${MAINTAINER}|g" \
    "${TPL}" > "${OUT}/PKGBUILD"
cp "${INSTALL_FILE}" "${OUT}/asus-numpad-rs.install"
chmod 0644 "${OUT}/PKGBUILD" "${OUT}/asus-numpad-rs.install"
log "已生成 ${OUT}/PKGBUILD（${VERSION}, sha256=${SHA256}）"

# .SRCINFO 只能由 makepkg 生成，Arch 环境里顺手做掉；没有就留空由 CI 补
if command -v makepkg >/dev/null 2>&1; then
    ( cd "${OUT}" && makepkg --printsrcinfo > .SRCINFO )
    log "已生成 ${OUT}/.SRCINFO"
else
    log "本机没有 makepkg，跳过 .SRCINFO（CI 的 arch 容器里会生成）"
fi

ls -l "${OUT}"
