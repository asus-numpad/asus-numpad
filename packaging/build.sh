#!/usr/bin/env bash
# asus-numpad 统一建包入口。
#
#   ./packaging/build.sh                 # 按当前发行版自动出包
#   ./packaging/build.sh deb             # 只出 .deb
#   ./packaging/build.sh rpm arch        # 出 rpm + Arch 包
#   ./packaging/build.sh all             # deb + rpm + arch + tarball 全出
#   ./packaging/build.sh tarball         # 通用 tarball（任何发行版都能装）
#   ./packaging/build.sh alpine          # Alpine .apk（需 abuild）
#   ./packaging/build.sh --version 5.0.1 deb
#
# 选项:
#   --version V   指定版本（默认取 Cargo.toml）
#   --out DIR     产物目录（默认 <repo>/dist）
#   --no-build    跳过 cargo 编译（已有 target/release/asus-numpad 时用）
#   --check       只做打包前自检（工具链/文件齐全性），不产出包

set -euo pipefail
SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
source "$SELF_DIR/lib/common.sh"

VERSION=""
OUT_DIR=""
DO_BUILD=1
CHECK_ONLY=0
TARGETS=()

while (($#)); do
    case "$1" in
        --version)  VERSION="${2:?}"; shift 2 ;;
        --out)      OUT_DIR="${2:?}"; shift 2 ;;
        --no-build) DO_BUILD=0; shift ;;
        --check)    CHECK_ONLY=1; shift ;;
        -h|--help)  sed -n '2,20p' "${BASH_SOURCE[0]}"; exit 0 ;;
        deb|rpm|arch|tarball|alpine|all) TARGETS+=("$1"); shift ;;
        *) die "未知参数: $1（可用: deb rpm arch tarball alpine all）" ;;
    esac
done

require_linux
ROOT="$(repo_root)"
VERSION="${VERSION:-$(cargo_version)}"
OUT_DIR="${OUT_DIR:-$ROOT/dist}"

# 没有显式指定就按发行版推断，避免用户在 Debian 上手滑打 all 白等
if ((${#TARGETS[@]} == 0)); then
    if [[ -r /etc/os-release ]]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        case "${ID:-}${ID_LIKE:-}" in
            *debian*|*ubuntu*|*linuxmint*|*pop*)      TARGETS=(deb) ;;
            *fedora*|*rhel*|*centos*|*rocky*|*alma*)  TARGETS=(rpm) ;;
            *arch*|*manjaro*|*endeavouros*)           TARGETS=(arch) ;;
            *)                                        TARGETS=(deb) ;;
        esac
        log "按发行版 $ID 推断目标: ${TARGETS[*]}"
    else
        TARGETS=(deb)
        warn "读不到 /etc/os-release，默认出 deb"
    fi
fi

# all 展开
EXPANDED=()
for t in "${TARGETS[@]}"; do
    if [[ "$t" == all ]]; then EXPANDED+=(deb rpm arch tarball); else EXPANDED+=("$t"); fi
done
# 去重且保序
mapfile -t EXPANDED < <(printf '%s\n' "${EXPANDED[@]}" | awk '!seen[$0]++')

log "版本 ${VERSION}，目标 ${EXPANDED[*]}，产物目录 $OUT_DIR"

# --------------------------------------------------------------------------- #
# 打包前自检：缺文件比缺工具更常见（.spec 里的 %license、service、rules 都一样致命）
# --------------------------------------------------------------------------- #
check_inputs() {
    local fail=0 f
    for f in \
        "$ROOT/Cargo.toml" \
        "$ROOT/LICENSE" \
        "$ROOT/packaging/common/asus-numpad.service" \
        "$ROOT/packaging/common/99-asus-numpad.rules" \
        "$ROOT/packaging/common/asus-numpad.1" \
        "$ROOT/packaging/deb/postinst" \
        "$ROOT/packaging/deb/prerm" \
        "$ROOT/packaging/deb/postrm" \
        "$ROOT/packaging/arch/PKGBUILD" \
        "$ROOT/packaging/arch/asus-numpad.install" \
        "$ROOT/packaging/rpm/asus-numpad.spec"
    do
        [[ -f "$f" ]] || { warn "缺少文件: ${f#"$ROOT"/}"; fail=1; }
    done

    # 脚本必须带可执行位，否则 dpkg/rpm 装出来的服务调不到
    for f in "$ROOT"/packaging/build.sh "$ROOT"/packaging/deb/make-deb.sh \
             "$ROOT"/packaging/arch/make-arch.sh "$ROOT"/packaging/rpm/make-rpm.sh; do
        [[ -x "$f" ]] || warn "脚本没有可执行位: ${f#"$ROOT"/}（可 chmod +x）"
    done

    # deb postinst/prerm/postrm 必须是可执行且以 #!/bin/sh 开头
    for f in postinst prerm postrm; do
        [[ "$(head -c 9 "$ROOT/packaging/deb/$f" 2>/dev/null)" == "#!/bin/sh" ]] \
            || warn "packaging/deb/$f 的 shebang 不是 #!/bin/sh"
    done

    for t in "${EXPANDED[@]}"; do
        case "$t" in
            deb)  have dpkg-deb || { warn "缺少 dpkg-deb（无法出 deb）"; fail=1; } ;;
            rpm)  have rpmbuild || { warn "缺少 rpmbuild（无法出 rpm）"; fail=1; } ;;
            arch) have makepkg && continue; have bsdtar || { warn "既无 makepkg 也无 bsdtar（无法出 Arch 包）"; fail=1; } ;;
            tarball) have tar || { warn "缺少 tar（无法出 tarball）"; fail=1; } ;;
            alpine)  have abuild || { warn "缺少 abuild（无法出 .apk，需 Alpine 环境）"; fail=1; } ;;
        esac
    done

    # Cargo.lock 缺失只告警：首次克隆还没有锁文件，不该因此挡住打包
    [[ -f "$ROOT/Cargo.lock" ]] || warn "缺少 Cargo.lock（发布前建议 cargo generate-lockfile 后提交）"

    have cargo || { warn "缺少 cargo（无法编译）"; fail=1; }

    return $fail
}

if ! check_inputs; then
    log "自检有告警（见上），继续执行；若准备发布请先修干净"
fi

if [[ $CHECK_ONLY -eq 1 ]]; then
    log "自检完成（--check，不产出包）"
    exit 0
fi

# --------------------------------------------------------------------------- #
# 编译一次，三个目标共用同一份产物
# --------------------------------------------------------------------------- #
if [[ $DO_BUILD -eq 1 ]]; then
    cargo_build
else
    [[ -x "$(built_binary)" ]] || die "--no-build 但 $(built_binary) 不存在"
fi

mkdir -p "$OUT_DIR"
for t in "${EXPANDED[@]}"; do
    case "$t" in
        deb)
            log "── 构建 deb ──"
            "$SELF_DIR/deb/make-deb.sh" --version "$VERSION" --no-build --out "$OUT_DIR"
            ;;
        rpm)
            log "── 构建 rpm ──"
            # 不传 --no-build：rpm 的编译在 spec 的 %build 里由 rpmbuild 自己做，
            # 这里传了反而会把 rpmbuild 一起跳过，最后拿不到 .rpm
            "$SELF_DIR/rpm/make-rpm.sh" --version "$VERSION" --out "$OUT_DIR"
            ;;
        arch)
            log "── 构建 arch ──"
            "$SELF_DIR/arch/make-arch.sh" --version "$VERSION" --no-build --out "$OUT_DIR"
            ;;
        alpine)
            log "── 构建 alpine ──"
            # abuild 自己会跑一遍 cargo（在净网沙箱里），这里不能传 --no-build：
            # .apk 的编译发生在 APKBUILD 的 build() 内，跳过就出不了包。
            "$SELF_DIR/alpine/make-alpine.sh" --version "$VERSION" --out "$OUT_DIR"
            ;;
        tarball)
            log "── 构建 tarball ──"
            "$SELF_DIR/tarball/make-tarball.sh" --version "$VERSION" --no-build --out "$OUT_DIR"
            ;;
        *)
            # 以前这里没有兜底：`all` 展开出来的 tarball/alpine 会被静默跳过，
            # 用户看到「全部完成」却少一半产物。现在直接报错，宁可失败也不假装成功。
            die "内部错误：目标 $t 没有对应的构建分支"
            ;;
    esac
done

log "全部完成，产物："
find "$OUT_DIR" -maxdepth 1 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.*' \
    -o -name '*.apk' -o -name '*.tar.gz' \) \
    -printf '  %f  (%s bytes)\n' 2>/dev/null || ls -l "$OUT_DIR"
