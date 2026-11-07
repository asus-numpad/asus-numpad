#!/usr/bin/env bash
# 构建 .deb。用 dpkg-deb 直接打包，不依赖 debhelper —— 项目只有一个二进制
# 加两个数据文件，走 dh 那套反而要拉一整套构建依赖。
#
# 用法: ./packaging/deb/make-deb.sh [--version X] [--distro NAME] [--no-build] [--out DIR]
#
# --distro 给版本号加 `~NAME` 后缀（如 `1.0.0~jammy`），用于同一次发版里
# 为多个发行版各出一个包：文件名不同才不会互相覆盖，apt 也认这种版本序
# （`1.0.0~jammy` < `1.2.0`，升级链不断）。出"通用基线"包时不带这个参数。

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
DISTRO=""
DO_BUILD=1
OUT_DIR=""

while (($#)); do
    case "$1" in
        --version)  VERSION="${2:?}"; shift 2 ;;
        --distro)   DISTRO="${2:?}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        --out)      OUT_DIR="${2:?}"; shift 2 ;;
        -h|--help)  sed -n '2,12p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

require_linux
require dpkg-deb install sed awk

ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
[[ -n "$VERSION" ]] || die "读不到 Cargo.toml 里的版本号"
DEB_ARCH="$(arch_deb)"
# 发行版标签只进版本号，不改包名——包名一变就成了另一个软件包，升级链会断
if [[ -n "$DISTRO" ]]; then
    VERSION="${VERSION}~${DISTRO}"
fi
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
if [[ $DO_BUILD -eq 1 ]]; then
    cargo_build
fi
mkdir -p "$OUT_DIR"

STAGE="$(mktmp)"
PKGROOT="$STAGE/${PKG_NAME}_${VERSION}_${DEB_ARCH}"

log "布置 deb 目录树"
install_common_files "$ROOT" "$PKGROOT"

# 配置文件放进 /etc，conffiles 让 dpkg 在升级时保留用户改过的版本
install -d "$PKGROOT/etc/asus-numpad"
cat > "$PKGROOT/etc/asus-numpad/asus-numpad.json" <<'JSON'
{
  "numlock": "sync",
  "calc_gesture": true,
  "calc_gesture_mode": "long",
  "mixed_mode": "auto",
  "grab": true
}
JSON

install -d "$PKGROOT/DEBIAN"
for script in postinst prerm postrm; do
    install -m 0755 "$SELF_DIR/$script" "$PKGROOT/DEBIAN/$script"
done

cat > "$PKGROOT/DEBIAN/control" <<EOF
Package: $PKG_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $DEB_ARCH
Depends: libc6 (>= 2.31)
Recommends: systemd
Installed-Size: $(du -sk --exclude=DEBIAN "$PKGROOT" | cut -f1)
Maintainer: asus-numpad maintainers <maintainers@example.invalid>
Homepage: $PKG_URL
Description: $PKG_SUMMARY
$(deb_indent_desc)
EOF

printf '/etc/asus-numpad/asus-numpad.json\n' > "$PKGROOT/DEBIAN/conffiles"

# 校验：漏掉 section 的 control 会被 lintian 拦下，这里先自己挡
grep -q '^Package: ' "$PKGROOT/DEBIAN/control" || die "control 生成异常"

DEB="$OUT_DIR/${PKG_NAME}_${VERSION}_${DEB_ARCH}.deb"
log "打包 $DEB"
dpkg-deb --root-owner-group -Zxz --build "$PKGROOT" "$DEB" >/dev/null

log "完成: $DEB ($(du -h "$DEB" | cut -f1))"
log "安装: sudo apt install ./$(basename "$DEB")   或   sudo dpkg -i $DEB"
