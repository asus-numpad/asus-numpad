#!/usr/bin/env bash
# 通用 tarball 包：给没有维护者出包的发行版（NixOS、Void、Slackware、openEuler、
# 各种不可变系统）一个不依赖包管理器的安装路径。
#
# 产物结构（解包后）：
#   numdeck-<version>/
#     usr/bin/numdeck              —— 二进制（--static 时为 musl 静态链接）
#     usr/lib/systemd/system/*.service
#     usr/lib/udev/rules.d/*.rules
#     usr/share/man/man1/*.1
#     usr/share/numdeck/lang/*.lang
#     etc/numdeck/numdeck.json.sample
#     install.sh / uninstall.sh        —— 纯 POSIX sh，可 --prefix
#
# 用法: ./packaging/tarball/make-tarball.sh [--version X] [--no-build] [--static]
#                                          [--out DIR] [--arch ARCH] [--zig]

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
DO_BUILD=1
OUT_DIR=""
STATIC=0
# TARGET_ARCH 已由 common.sh 从 NUMDECK_TARGET 派生（可能是 aarch64）。
# 命令行值另存 _ARCH_ARG，避免把派生结果清空后退回 uname -m。
_ARCH_ARG=""

while (($#)); do
    case "$1" in
        --version)  VERSION="${2:?}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        --out)      OUT_DIR="${2:?}"; shift 2 ;;
        --static)   STATIC=1; shift ;;
        --arch)     _ARCH_ARG="${2:?}"; shift 2 ;;
        --zig)      NUMDECK_USE_ZIGBUILD=1; shift ;;
        -h|--help)  sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
[[ -n "$VERSION" ]] || die "读不到 Cargo.toml 里的版本号"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
TARGET_ARCH="${_ARCH_ARG:-${TARGET_ARCH:-$(uname -m)}}"
BUNDLE="$PKG_NAME-$VERSION"

# --------------------------------------------------------------------------- #
# 编译：--static 走 musl，产出的二进制在任意 x86_64 / aarch64 发行版上都能跑
# --------------------------------------------------------------------------- #
if [[ $STATIC -eq 1 ]]; then
    MUSL_TRIPLE="${TARGET_ARCH}-unknown-linux-musl"
    require cargo
    # 跨架构时 rust-lld 的自包含链接不成立，换 zig 提供链接器与 libc.a
    CROSS=0
    [[ "$TARGET_ARCH" != "$(uname -m)" ]] && CROSS=1
    if [[ $CROSS -eq 1 && "${NUMDECK_USE_ZIGBUILD:-0}" != 1 ]]; then
        die "交叉出 ${TARGET_ARCH} 的静态包需要 zig：加 --zig（或 NUMDECK_USE_ZIGBUILD=1）"
    fi
    if ! rustup target list --installed 2>/dev/null | grep -qx "$MUSL_TRIPLE"; then
        log "安装目标 $MUSL_TRIPLE"
        rustup target add "$MUSL_TRIPLE" || die "装不上 ${MUSL_TRIPLE}（需要 rustup）"
    fi
    if [[ $CROSS -eq 1 ]]; then
        have cargo-zigbuild || die "需要 cargo-zigbuild：cargo install cargo-zigbuild（还要 zig 在 PATH 中）"
        log "静态编译（${MUSL_TRIPLE}，zig 交叉）"
        ( cd "$ROOT" && cargo zigbuild --release --locked --target "$MUSL_TRIPLE" ) \
            || ( cd "$ROOT" && cargo zigbuild --release --target "$MUSL_TRIPLE" )
    else
        log "静态编译（${MUSL_TRIPLE}）"
        # 同架构时 musl 目标自带自包含链接，不再需要 glibc 版本假设
        ( cd "$ROOT" && cargo build --release --locked --target "$MUSL_TRIPLE" ) \
            || ( cd "$ROOT" && cargo build --release --target "$MUSL_TRIPLE" )
    fi
    export NUMDECK_BIN="$ROOT/target/$MUSL_TRIPLE/release/$BIN_NAME"
    [[ -x "$NUMDECK_BIN" ]] || die "静态编译产物不存在: $NUMDECK_BIN"
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

install -d "$STAGE/etc/numdeck"
default_config_json > "$STAGE/etc/numdeck/numdeck.json.sample"

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
