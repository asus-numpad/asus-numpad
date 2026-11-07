#!/usr/bin/env bash
# 构建 Alpine 包（.apk）。
#
# 前置：apk add alpine-sdk；abuild-keygen -a -i（一次即可，否则 abuild 拒绝签名）
#
# 用法: ./packaging/alpine/make-alpine.sh [--version X] [--out DIR]

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
OUT_DIR=""

while (($#)); do
    case "$1" in
        --version) VERSION="${2:?}"; shift 2 ;;
        --out)     OUT_DIR="${2:?}"; shift 2 ;;
        -h|--help) sed -n '2,7p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)         die "未知参数: $1" ;;
    esac
done

ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"

have abuild || die "缺少 abuild（apk add alpine-sdk）"
if ! ls "$HOME/.abuild/"*.rsa.pub >/dev/null 2>&1; then
    warn "没有 abuild 密钥，先执行 abuild-keygen -a -i，否则 abuild 会拒绝出包"
fi

# APKBUILD 的 source 指向 GitHub 归档，本地出包时改用当前工作树打出来的 tarball，
# 这样未发布的改动也能测到
WORK="$(mktmp)"
# abuild 要求 APKBUILD 所在目录名等于 pkgname（它按目录名推导 $pkgname 与产物的
# 落点），直接把 APKBUILD 丢在 aports/ 根下会被拒绝，所以按 aports 的层级摆好。
APKROOT="$WORK/aports/community/$PKG_NAME"
SRCDEST="$WORK/srcdest"
mkdir -p "$APKROOT" "$SRCDEST"

log "导出源码树 $PKG_NAME-$VERSION"
SRCTREE="$WORK/$PKG_NAME-$VERSION"
mkdir -p "$SRCTREE"
tar -C "$ROOT" --exclude='./target' --exclude='./dist' --exclude='./.git' -cf - . \
    | tar -C "$SRCTREE" -xf -
tar -C "$WORK" -czf "$SRCDEST/$PKG_NAME-$VERSION.tar.gz" "$PKG_NAME-$VERSION"
SHA="$(sha512sum "$SRCDEST/$PKG_NAME-$VERSION.tar.gz" | cut -d' ' -f1)"

sed -e "s/^pkgver=.*/pkgver=$VERSION/" \
    -e "s|^sha512sums=.*|sha512sums=\"$SHA  $PKG_NAME-$VERSION.tar.gz\"|" \
    -e "s|^source=.*|source=\"$PKG_NAME-\$pkgver.tar.gz\"|" \
    "$SELF_DIR/APKBUILD" > "$APKROOT/APKBUILD"
install -m 0644 "$SELF_DIR/$PKG_NAME.post-install" "$APKROOT/"
# source 里写的是裸文件名（不带 URL），abuild 会当成「本地源码」直接在 APKBUILD
# 所在目录找；只放进 SRCDEST 它会报 sha512sum FAILED。这里复制一份到 startdir。
cp "$SRCDEST/$PKG_NAME-$VERSION.tar.gz" "$APKROOT/"

log "abuild -r（产物目录 ${OUT_DIR}）"
(
    cd "$APKROOT"
    SRCDEST="$SRCDEST" PKGDEST="$OUT_DIR" REPODEST="$WORK/packages" \
        abuild -r -P "$WORK/packages"
)

# abuild 对产物落点有两套变量：PKGDEST 与 -P 指定的 REPODEST，不同版本优先级
# 不一样。两边都扫一遍，把 .apk 收进 OUT_DIR，免得「abuild 成功了但 dist 是空的」。
shopt -s nullglob
if [ -z "$(echo "$OUT_DIR"/*.apk)" ]; then
    find "$WORK/packages" -name '*.apk' -exec cp -f {} "$OUT_DIR/" \; 2>/dev/null || true
fi
apks=("$OUT_DIR"/*.apk)
if [ "${#apks[@]}" -eq 0 ]; then
    die "abuild 跑完了但没找到 .apk（检查 abuild 的输出与 REPODEST）"
fi
log "完成，产物："
# 不用 find -printf：Alpine 带的是 busybox find，没有这个动作
for f in "${apks[@]}"; do
    log "  $(basename "$f")"
done
