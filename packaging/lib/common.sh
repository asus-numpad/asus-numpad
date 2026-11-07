#!/usr/bin/env bash
# 打包公共函数。所有脚本都 source 这一份，避免逻辑各写一遍。
# 语言：Bash（与 asus-ctl 的发行版集成方式一致），不依赖 Python。
# shellcheck disable=SC2034  # 这些常量由 source 本文件的调用方消费，不属于本文件内的未使用变量

set -euo pipefail

PKG_NAME="asus-numpad"
BIN_NAME="asus-numpad"
PKG_LICENSE="GPL-3.0-or-later"
PKG_URL="https://github.com/asus-numpad/asus-numpad"
PKG_SUMMARY="ASUS 触摸板数字小键盘（NumberPad）Linux 用户态驱动"
PKG_DESC="ASUS 触摸板数字小键盘用户态驱动，参数取自官方 AsusNUMPADFilter.inf。"

# --------------------------------------------------------------------------- #
# 输出
# --------------------------------------------------------------------------- #
if [[ -t 1 ]]; then
    C_RED=$'\033[31m'; C_GRN=$'\033[32m'; C_YEL=$'\033[33m'; C_RST=$'\033[0m'
else
    C_RED=''; C_GRN=''; C_YEL=''; C_RST=''
fi

log()  { printf '%s==>%s %s\n' "$C_GRN" "$C_RST" "$*"; }
warn() { printf '%s[警告]%s %s\n' "$C_YEL" "$C_RST" "$*" >&2; }
die()  { printf '%s[错误]%s %s\n' "$C_RED" "$C_RST" "$*" >&2; exit 1; }

# --------------------------------------------------------------------------- #
# 工具
# --------------------------------------------------------------------------- #
have() { command -v "$1" >/dev/null 2>&1; }

require() {
    local missing=()
    for t in "$@"; do have "$t" || missing+=("$t"); done
    ((${#missing[@]} == 0)) || die "缺少依赖命令: ${missing[*]}"
}

# 仓库根目录（本文件在 <root>/packaging/lib/ 下）
repo_root() {
    local here
    here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
    (cd -- "$here/../.." && pwd)
}

# 从 Cargo.toml 读版本，避免两处版本号漂移
cargo_version() {
    local root
    root="$(repo_root)"
    sed -n 's/^version[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$root/Cargo.toml" | head -n1
}

# 目标架构 -> 各发行版叫法
arch_deb()  { case "$(uname -m)" in x86_64) echo amd64;; aarch64) echo arm64;; *) uname -m;; esac; }
arch_rpm()  { uname -m; }
arch_pkg()  { uname -m; }

# 是否在 Linux
require_linux() {
    [[ "$(uname -s)" == "Linux" ]] || die "打包只能在 Linux 上执行（当前 $(uname -s)）"
}

# 临时目录：在 source 时就建好。
# 不能懒加载——`mktmp` 是在 $(...) 子 shell 里调用的，在子 shell 里给 TMP_ROOT 赋值
# 父进程看不到，等于白建；而 trap 注册在子 shell 里也不会生效。
TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/asus-numpad-build.XXXXXX")"

mktmp() {
    mktemp -d "$TMP_ROOT/work.XXXXXX"
}

cleanup() {
    [[ -n "$TMP_ROOT" && -d "$TMP_ROOT" ]] && rm -rf -- "$TMP_ROOT"
    return 0
}

trap cleanup EXIT INT TERM

# 编译产物路径。
# `ASUS_NUMPAD_BIN` 可覆盖：交叉编译或复用 CI 产物时不必让每个打包器都重新编译一次。
built_binary() {
    if [[ -n "${ASUS_NUMPAD_BIN:-}" ]]; then
        echo "$ASUS_NUMPAD_BIN"
        return 0
    fi
    local root
    root="$(repo_root)"
    echo "$root/target/release/$BIN_NAME"
}

# 调用 cargo 编译。
# 有 Cargo.lock 就 --locked 保证可复现；没有就先放行（首次克隆 / 依赖刚改过），
# 只告警不中断——否则新用户第一步就卡死，比版本漂移更烦人。
cargo_build() {
    local root args mode
    root="$(repo_root)"
    require cargo
    args=(build --release)
    mode="未锁定"
    if [[ -f "$root/Cargo.lock" ]]; then
        args+=(--locked)
        mode="locked"
    else
        warn "缺少 Cargo.lock，本次不加 --locked；发布前请执行 cargo generate-lockfile 并提交"
    fi
    log "编译 ${BIN_NAME}（release, ${mode}）"
    ( cd "$root" && cargo "${args[@]}" )
    [[ -x "$(built_binary)" ]] || die "编译产物不存在: $(built_binary)"
}

# --------------------------------------------------------------------------- #
# 文件安装（各发行版共用同一份清单，改一处即可）
# --------------------------------------------------------------------------- #
install_common_files() {
    local root="$1" destdir="$2"
    local bin bindir mandir unitdir rulesdir langdir

    bin="$(built_binary)"
    bindir="$destdir/usr/bin"
    mandir="$destdir/usr/share/man/man1"
    unitdir="$destdir/usr/lib/systemd/system"
    rulesdir="$destdir/usr/lib/udev/rules.d"
    langdir="$destdir/usr/share/asus-numpad/lang"

    install -d "$bindir" "$mandir" "$unitdir" "$rulesdir"
    install -m 0755 "$bin" "$bindir/$BIN_NAME"
    install -m 0644 "$root/packaging/common/asus-numpad.service" "$unitdir/asus-numpad.service"
    install -m 0644 "$root/packaging/common/99-asus-numpad.rules" "$rulesdir/99-asus-numpad.rules"
    if [[ -f "$root/packaging/common/asus-numpad.1" ]]; then
        install -m 0644 "$root/packaging/common/asus-numpad.1" "$mandir/asus-numpad.1"
    fi
    # 语言文件：二进制内置一份兜底，这里的可编辑副本供用户整份替换
    if [[ -d "$root/lang" ]]; then
        install -d "$langdir"
        install -m 0644 \
            "$root/lang/zh_CN.lang" "$root/lang/zh_TW.lang" "$root/lang/en.lang" "$langdir/"
    fi
    install -d "$destdir/usr/share/doc/$PKG_NAME"
    [[ -f "$root/README.md" ]] && install -m 0644 "$root/README.md" "$destdir/usr/share/doc/$PKG_NAME/README.md"
    [[ -f "$root/LICENSE" ]]   && install -m 0644 "$root/LICENSE"   "$destdir/usr/share/doc/$PKG_NAME/copyright"
    return 0
}

# 默认配置：三套打包器各抄一份迟早会漂移，统一从这里取
default_config_json() {
    cat <<'JSON'
{
  "numlock": "sync",
  "calc_gesture": true,
  "calc_gesture_mode": "long",
  "mixed_mode": "auto",
  "grab": true,
  "follow_keyboard_brightness": true,
  "manual_brightness_step": 4,
  "language": "auto"
}
JSON
}

# 机型自证文件：装完 `rpm -ql` / `pacman -Ql` / `dpkg -L` 就能看出这包属于哪台机器
# 版本号 -> 打包用的整数（deb/rpm 要求数字开头、arch 与 alpine 只要求干净字符串）
version_is_semver() {
    [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]
}

# deb 的 Description 续行必须以空格开头，否则 dpkg 解析整包失败。
# 这里统一负责把多行描述折成合法格式，别让每个包格式自己拼。
deb_indent_desc() {
    printf '%s\n' "$PKG_DESC" | sed -e 's/^[[:space:]]*$/ ./' -e 's/^/ /'
}
