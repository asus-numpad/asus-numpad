#!/usr/bin/env bash
# 构建 Arch 包（.pkg.tar.zst）。
# 有 makepkg 就走标准流程（源码打 tarball → makepkg）；没有 makepkg（非 Arch 主机）
# 就手工拼一个同构的包，方便在 Fedora/Debian 机器上也能产出 Arch 可用的归档。

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
DO_BUILD=1
OUT_DIR=""

while (($#)); do
    case "$1" in
        --version)  VERSION="${2:?}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        --out)      OUT_DIR="${2:?}"; shift 2 ;;
        -h|--help)  sed -n '2,6p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

require_linux
ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
[[ -n "$VERSION" ]] || die "读不到版本号"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"
# makepkg 会在子目录里跑，PKGDEST 必须是绝对路径，否则产物落到子目录里找不到
OUT_DIR="$(cd "$OUT_DIR" && pwd)"

WORK="$(mktmp)"
SRCTREE="$WORK/$PKG_NAME-$VERSION"

log "导出源码树"
mkdir -p "$SRCTREE"
# 排除构建产物与历史交付目录，避免把 target/（几百 MB）塞进包
tar -C "$ROOT" \
    --exclude='./target' --exclude='./dist' --exclude='./.git' \
    -cf - . | tar -C "$SRCTREE" -xf -

if have makepkg && [[ $EUID -ne 0 ]]; then
    log "使用 makepkg 构建"
    ARCHBUILD="$WORK/archbuild"
    mkdir -p "$ARCHBUILD"
    tar -C "$WORK" -czf "$ARCHBUILD/$PKG_NAME-$VERSION.tar.gz" "$PKG_NAME-$VERSION"

    sed -e "s/^pkgver=.*/pkgver=$VERSION/" \
        -e "s/^pkgrel=.*/pkgrel=1/" \
        "$SELF_DIR/PKGBUILD" > "$ARCHBUILD/PKGBUILD"
    cp "$SELF_DIR/$PKG_NAME.install" "$ARCHBUILD/"

    # makepkg 走 PKGBUILD 的 build()，一定自己编译一次；--no-build 在这里只保证
    # 不额外重复调用 cargo_build，不会让 makepkg 用现成二进制跳过编译。
    if [[ $DO_BUILD -eq 0 ]]; then
        warn "makepkg 流程下仍会执行 PKGBUILD 的 build()，--no-build 不会跳过编译"
    fi
    ( cd "$ARCHBUILD" && PKGDEST="$OUT_DIR" makepkg -f --noconfirm --nodeps ) \
        || die "makepkg 失败"
    # 用 find 按 mtime 取最新产物：makepkg 会清掉旧包，但目录里可能还有历史版本
    pkgfile=$(find "$OUT_DIR" -maxdepth 1 -type f \
        -name "${PKG_NAME}-${VERSION}-1-*.pkg.tar.*" \
        -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -n1 | cut -d' ' -f2- || true)
else
    # makepkg 明确拒绝以 root 运行（容器/CI 里就是这样），此时走手工拼包；
    # 没有任何编译工具链的 Debian/Fedora 主机也走这条路。
    if have makepkg; then
        warn "以 root 运行，makepkg 不允许 root 构建，改走手工拼包"
    else
        warn "本机没有 makepkg，手工构建同构包（不含 .MTREE/.BUILDINFO）"
    fi
    require bsdtar
    if [[ $DO_BUILD -eq 1 ]]; then
        cargo_build
    else
        [[ -x "$(built_binary)" ]] || die "--no-build 但 $(built_binary) 不存在"
    fi

    PKGROOT="$WORK/pkgroot"
    install_common_files "$ROOT" "$PKGROOT"
    install -d "$PKGROOT/etc/numdeck"
    cat > "$PKGROOT/etc/numdeck/numdeck.json" <<'JSON'
{
  "numlock": "sync",
  "calc_gesture": true,
  "calc_gesture_mode": "long",
  "mixed_mode": "auto",
  "grab": true
}
JSON

    cat > "$PKGROOT/.PKGINFO" <<EOF
pkgname = $PKG_NAME
pkgbase = $PKG_NAME
pkgver = $VERSION-1
pkgdesc = $PKG_SUMMARY
url = $PKG_URL
builddate = $(date +%s)
packager = numdeck maintainers <maintainers@example.invalid>
size = $(du -sb "$PKGROOT" | cut -f1 | tr -d ' ')
arch = $(arch_pkg)
license = $PKG_LICENSE
depend = glibc
backup = etc/numdeck/numdeck.json
EOF

    pkgfile="$OUT_DIR/${PKG_NAME}-${VERSION}-1-$(arch_pkg).pkg.tar.zst"
    log "打包 $pkgfile"
    ( cd "$PKGROOT" && bsdtar --zstd -cf "$pkgfile" . )
fi

[[ -n "${pkgfile:-}" && -f "$pkgfile" ]] || die "未产出 .pkg.tar.zst"
log "完成: $pkgfile ($(du -h "$pkgfile" | cut -f1))"
log "安装: sudo pacman -U $(basename "$pkgfile")"
