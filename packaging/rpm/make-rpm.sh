#!/usr/bin/env bash
# 构建 RPM。用 rpmbuild 走标准 spec，源码按 tarball 喂进去（%autosetup 依赖它）。
#
# 用法: ./packaging/rpm/make-rpm.sh [--version X] [--release N] [--check] [--out DIR]
#
# --release 指定 RPM 的 Release 号（默认 1）。同一次发版要给多个发行版出包时，
# 用不同的 Release 让文件名互不覆盖；spec 里的 `%{?dist}` 会再补上 .el9 / .fc44 之类的
# 发行版标记，所以通常不需要手动区分。
# 只出一个包：机型配置由程序在运行期自动识别，不再按机型分包。
#
# 注意：编译发生在 spec 的 %build 里，由 rpmbuild 自己完成，所以这里没有
# “跳过编译”的选项；--check 只做 spec 语法校验（rpmbuild --nobuild），不产出包。

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../lib/common.sh
source "$SELF_DIR/../lib/common.sh"

VERSION=""
RELEASE="1"
CHECK_ONLY=0
OUT_DIR=""

while (($#)); do
    case "$1" in
        --version)  VERSION="${2:?}"; shift 2 ;;
        --release)  RELEASE="${2:?}"; shift 2 ;;
        --check)    CHECK_ONLY=1; shift ;;
        --out)      OUT_DIR="${2:?}"; shift 2 ;;
        -h|--help)  sed -n '2,14p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

require_linux
require rpmbuild tar

ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
[[ -n "$VERSION" ]] || die "读不到版本号"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"
mkdir -p "$OUT_DIR"

WORK="$(mktmp)"
TOPDIR="$WORK/rpmbuild"
mkdir -p "$TOPDIR"/{SOURCES,SPECS,BUILD,BUILDROOT,RPMS,SRPMS}

log "导出源码 tarball"
tar -C "$ROOT" \
    --exclude='./target' --exclude='./dist' --exclude='./.git' \
    --exclude-vcs-ignores \
    --transform "s,^\.,$PKG_NAME-$VERSION," \
    -czf "$TOPDIR/SOURCES/$PKG_NAME-$VERSION.tar.gz" .

# spec 里 %license LICENSE 要求源码树真有这个文件，缺了 rpmbuild 会直接失败
[[ -f "$ROOT/LICENSE" ]] || die "缺少 LICENSE 文件（spec 的 %license 需要它）"

sed -e "s/@VERSION@/$VERSION/g" -e "s/@RELEASE@/$RELEASE/g" \
    "$SELF_DIR/numdeck.spec" > "$TOPDIR/SPECS/$PKG_NAME.spec"

DEFINES=(
    --define "_topdir $TOPDIR"
    --define "_sourcedir $TOPDIR/SOURCES"
    --define "_specdir $TOPDIR/SPECS"
    --define "_builddir $TOPDIR/BUILD"
    --define "_rpmdir $TOPDIR/RPMS"
    --define "_srcrpmdir $TOPDIR/SRPMS"
    --define "debug_package %{nil}"
    --define "_build_id_links none"
)
# 跳过 BuildRequires 检查：CI 与本地常用 rustup 提供工具链，rpm 库里并不存在
# 「rust」这个包，不加 --nodeps 时 rpmbuild 会在「检查构建依赖」阶段直接失败。
# spec 里的 BuildRequires 仍然保留，发行版官方构建（mock/koji）照常据此装依赖。
DEFINES+=(--nodeps)

log "rpmbuild -bb"
if [[ $CHECK_ONLY -eq 1 ]]; then
    # --nobuild 只解析 spec，不执行 %prep/%build/%install。这条路没有 rpm 产出，
    # 所以必须在下面的“产物存在性”检查之前退出，否则会被自己判成失败；
    # 但解析失败仍是失败，不能吞掉退出码。
    if rpmbuild "${DEFINES[@]}" -bb --nobuild "$TOPDIR/SPECS/$PKG_NAME.spec"; then
        log "spec 校验通过，未产出二进制包（$TOPDIR/SPECS/$PKG_NAME.spec）"
    else
        die "spec 校验失败，见上方 rpmbuild 输出"
    fi
    exit 0
fi

rpmbuild "${DEFINES[@]}" -bb "$TOPDIR/SPECS/$PKG_NAME.spec"

shopt -s nullglob
rpms=("$TOPDIR"/RPMS/*/*.rpm)
shopt -u nullglob
((${#rpms[@]})) || die "rpmbuild 未产出 rpm"

for r in "${rpms[@]}"; do
    cp -f "$r" "$OUT_DIR/"
    log "完成: $OUT_DIR/$(basename "$r") ($(du -h "$r" | cut -f1))"
done
log "安装: sudo dnf install ./$(basename "${rpms[0]}")"
