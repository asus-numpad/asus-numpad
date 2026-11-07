#!/usr/bin/env bash
# 通用 tarball 包：给没有维护者出包的发行版（NixOS、Void、Slackware、openEuler、
# 各种不可变系统）一个不依赖包管理器的安装路径。
#
# 产物结构（解包后）：
#   asus-numpad-<version>/
#     usr/bin/asus-numpad              —— 二进制（--static 时为 musl 静态链接）
#     usr/lib/systemd/system/*.service
#     usr/lib/udev/rules.d/*.rules
#     usr/share/man/man1/*.1
#     usr/share/asus-numpad/lang/*.lang
#     etc/asus-numpad/asus-numpad.json.sample
#     install.sh / uninstall.sh        —— 纯 POSIX sh，可 --prefix
#
# 用法: ./packaging/tarball/make-tarball.sh [--version X] [--no-build] [--static]
#                                          [--out DIR] [--arch ARCH]

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
DO_BUILD=1
OUT_DIR=""
STATIC=0
TARGET_ARCH=""

while (($#)); do
    case "$1" in
        --version)  VERSION="${2:?}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        --out)      OUT_DIR="${2:?}"; shift 2 ;;
        --static)   STATIC=1; shift ;;
        --arch)     TARGET_ARCH="${2:?}"; shift 2 ;;
        -h|--help)  sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
[[ -n "$VERSION" ]] || die "读不到 Cargo.toml 里的版本号"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
TARGET_ARCH="${TARGET_ARCH:-$(uname -m)}"
BUNDLE="$PKG_NAME-$VERSION"

# --------------------------------------------------------------------------- #
# 编译：--static 走 musl，产出的二进制在任意 x86_64 发行版上都能跑
# --------------------------------------------------------------------------- #
if [[ $STATIC -eq 1 ]]; then
    MUSL_TRIPLE="${TARGET_ARCH}-unknown-linux-musl"
    require cargo
    if ! rustup target list --installed 2>/dev/null | grep -qx "$MUSL_TRIPLE"; then
        log "安装目标 $MUSL_TRIPLE"
        rustup target add "$MUSL_TRIPLE" || die "装不上 ${MUSL_TRIPLE}（需要 rustup）"
    fi
    log "静态编译（${MUSL_TRIPLE}）"
    # musl 目标默认自带自包含链接，不再需要 glibc 版本假设
    ( cd "$ROOT" && cargo build --release --locked --target "$MUSL_TRIPLE" ) \
        || ( cd "$ROOT" && cargo build --release --target "$MUSL_TRIPLE" )
    export ASUS_NUMPAD_BIN="$ROOT/target/$MUSL_TRIPLE/release/$BIN_NAME"
    [[ -x "$ASUS_NUMPAD_BIN" ]] || die "静态编译产物不存在: $ASUS_NUMPAD_BIN"
elif [[ $DO_BUILD -eq 1 ]]; then
    cargo_build
fi

[[ -x "$(built_binary)" ]] || die "找不到二进制: $(built_binary)"

# --------------------------------------------------------------------------- #
# 布置目录树
# --------------------------------------------------------------------------- #
WORK="$(mktmp)"
STAGE="$WORK/$BUNDLE"

log "布置 tarball 目录树"
install_common_files "$ROOT" "$STAGE"

install -d "$STAGE/etc/asus-numpad"
default_config_json > "$STAGE/etc/asus-numpad/asus-numpad.json.sample"

install -m 0755 "$SELF_DIR/install.sh"   "$STAGE/install.sh"
install -m 0755 "$SELF_DIR/uninstall.sh" "$STAGE/uninstall.sh"
install -m 0644 "$SELF_DIR/README.md"    "$STAGE/README.md"
[[ -f "$ROOT/LICENSE" ]] && install -m 0644 "$ROOT/LICENSE" "$STAGE/LICENSE"

# 记录链接方式，install.sh 靠它决定要不要提示 glibc 版本
if [[ $STATIC -eq 1 ]]; then
    printf 'static-musl\n' > "$STAGE/BUILD-MODE"
else
    printf 'dynamic-glibc\n' > "$STAGE/BUILD-MODE"
fi

TARBALL="$OUT_DIR/$BUNDLE-$TARGET_ARCH.tar.gz"
mkdir -p "$OUT_DIR"
log "打包 $TARBALL"
tar -C "$WORK" --owner=0 --group=0 --numeric-owner -czf "$TARBALL" "$BUNDLE"

log "完成: $TARBALL ($(du -h "$TARBALL" | cut -f1))"
log "安装: tar xzf $(basename "$TARBALL") && sudo ./$BUNDLE/install.sh"
