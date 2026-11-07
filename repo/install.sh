#!/bin/sh
# asus-numpad 一键安装：导入公钥 → 写入软件源 → 安装 → 版本自检
#
#   curl -fsSL https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/install.sh | sudo sh
#
# 换仓库地址（镜像、本地目录等）：
#   curl -fsSL https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/install.sh | sudo ASUS_NUMPAD_BASE=https://... sh
#
# 只写源不装：上面命令末尾加 --repo-only
set -eu

BASE="${ASUS_NUMPAD_BASE:-https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo}"
SIGNED=1
FPR="79F282157F2D3FF8E3C6806314C2BF611986E168"
PKG=asus-numpad
MODE="${1:-install}"

log() { printf '==> %s\n' "$*"; }
die() { printf '错误: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

fetch() {
    if have curl; then curl -fsSL "$1"
    elif have wget; then wget -qO- "$1"
    else die "需要 curl 或 wget"
    fi
}

[ "$(id -u)" = 0 ] || die "需要 root：curl -fsSL $BASE/install.sh | sudo sh"

install_deb() {
    log "Debian 系：写 apt 源"
    install -d -m 0755 /usr/share/keyrings
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.gpg" > /usr/share/keyrings/asus-numpad.gpg
        echo "deb [signed-by=/usr/share/keyrings/asus-numpad.gpg] $BASE/apt stable main" \
            > /etc/apt/sources.list.d/asus-numpad.list
    else
        echo "deb [trusted=yes] $BASE/apt stable main" \
            > /etc/apt/sources.list.d/asus-numpad.list
    fi
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    apt-get update -q
    apt-get install -y "$PKG"
}

install_rpm() {
    mgr="$1"
    log "$mgr：写 rpm 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.asc" > /tmp/asus-numpad.asc
        rpm --import /tmp/asus-numpad.asc
    fi
    {
        echo "[asus-numpad]"
        echo "name=asus-numpad"
        echo "baseurl=$BASE/rpm/\$basearch"
        echo "enabled=1"
        if [ "$SIGNED" = 1 ]; then
            echo "gpgcheck=1"
            echo "repo_gpgcheck=1"
            echo "gpgkey=$BASE/keys/asus-numpad-repo.asc"
        else
            echo "gpgcheck=0"
            echo "repo_gpgcheck=0"
        fi
    } > /etc/yum.repos.d/asus-numpad.repo
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    "$mgr" install -y "$PKG"
}

install_zypper() {
    log "openSUSE：写 zypper 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.asc" > /tmp/asus-numpad.asc
        rpm --import /tmp/asus-numpad.asc
    fi
    {
        echo "[asus-numpad]"
        echo "name=asus-numpad"
        echo "enabled=1"
        echo "autorefresh=1"
        echo "type=rpm-md"
        echo "baseurl=$BASE/rpm/\$basearch"
        if [ "$SIGNED" = 1 ]; then
            echo "gpgcheck=1"
            echo "repo_gpgcheck=1"
            echo "gpgkey=$BASE/keys/asus-numpad-repo.asc"
        else
            echo "gpgcheck=0"
            echo "repo_gpgcheck=0"
        fi
    } > /etc/zypp/repos.d/asus-numpad.repo
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    zypper --gpg-auto-import-keys --non-interactive refresh asus-numpad
    zypper --non-interactive install "$PKG"
}

install_pacman() {
    log "Arch 系：写 pacman 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.asc" > /tmp/asus-numpad.asc
        pacman-key --add /tmp/asus-numpad.asc
        pacman-key --lsign-key "$FPR"
    fi
    if ! grep -q '^\[asus-numpad\]' /etc/pacman.conf; then
        {
            echo ""
            echo "[asus-numpad]"
            if [ "$SIGNED" = 1 ]; then
                echo "SigLevel = Required DatabaseRequired"
            else
                echo "SigLevel = Optional TrustAll"
            fi
            echo "Server = $BASE/arch/\$arch"
        } >> /etc/pacman.conf
    fi
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    pacman -Syu --noconfirm "$PKG"
}

install_apk() {
    log "Alpine：写 apk 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-apk.rsa.pub" > /etc/apk/keys/asus-numpad-apk.rsa.pub
    fi
    arch="$(apk --print-arch)"
    if ! grep -qxF "$BASE/alpine/$arch" /etc/apk/repositories; then
        echo "$BASE/alpine/$arch" >> /etc/apk/repositories
    fi
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    apk update
    if [ "$SIGNED" = 1 ]; then
        apk add "$PKG"
    else
        apk add --allow-untrusted "$PKG"
    fi
}

if have zypper; then
    install_zypper
elif have dnf; then
    install_rpm dnf
elif have yum; then
    install_rpm yum
elif have pacman; then
    install_pacman
elif have apk; then
    install_apk
elif have apt-get; then
    install_deb
else
    die "认不出包管理器，请按 $BASE/index.html 手工装"
fi

if [ "$MODE" = "--repo-only" ]; then
    exit 0
fi

log "装好了，自检："
"$PKG" --version || die "命令跑不起来，先看 dmesg 与 README 的排查段"
