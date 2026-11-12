#!/bin/sh
# numdeck 通用卸载脚本（仅针对 install.sh 装的文件；deb/rpm/arch 包请用包管理器卸）
#
#   sudo ./uninstall.sh
#   sudo ./uninstall.sh --prefix /usr/local --purge

set -eu

PREFIX="/"
DRY_RUN=0
PURGE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)  PREFIX="${2:?--prefix 需要路径}"; shift 2 ;;
        --dry-run) DRY_RUN=1; shift ;;
        --purge)   PURGE=1; shift ;;
        -h|--help) sed -n '2,7p' "$0"; exit 0 ;;
        *)         echo "未知参数: $1" >&2; exit 2 ;;
    esac
done

[ "$(id -u)" -eq 0 ] || { echo "需要 root（sudo $0 ...）" >&2; exit 1; }

PREFIX=${PREFIX%/}
say() { printf '==> %s\n' "$*"; }
run() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '    [dry-run] %s\n' "$*"
    else
        "$@"
    fi
}

# 先停服务，否则删了二进制 systemd 会一直重启失败
if [ "$DRY_RUN" -eq 0 ]; then
    for u in $(loginctl list-users --no-legend 2>/dev/null | awk '{print $1}'); do
        uid=$(id -u "$u" 2>/dev/null) || continue
        [ -d "/run/user/$uid" ] || continue
        su "$u" -s /bin/sh -c \
            "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user disable --now numdeck.service" \
            >/dev/null 2>&1 || true
    done
    systemctl --global disable numdeck.service >/dev/null 2>&1 || true
fi

say "删除文件"
for f in \
    "$PREFIX/usr/bin/numdeck" \
    "$PREFIX/usr/lib/systemd/system/numdeck.service" \
    "$PREFIX/usr/lib/udev/rules.d/99-numdeck.rules" \
    "$PREFIX/usr/share/man/man1/numdeck.1" \
    "$PREFIX/usr/share/numdeck/lang/zh_CN.lang" \
    "$PREFIX/usr/share/numdeck/lang/zh_TW.lang" \
    "$PREFIX/usr/share/numdeck/lang/en.lang" \
    "$PREFIX/usr/share/doc/numdeck/README.md" \
    "$PREFIX/usr/share/doc/numdeck/copyright"
do
    [ -e "$f" ] && run rm -f "$f"
done
run rmdir "$PREFIX/usr/share/numdeck/lang" 2>/dev/null || true
run rmdir "$PREFIX/usr/share/numdeck" 2>/dev/null || true
run rmdir "$PREFIX/usr/share/doc/numdeck" 2>/dev/null || true

if [ "$PURGE" -eq 1 ]; then
    say "删除配置（--purge）"
    run rm -rf "$PREFIX/etc/numdeck"
else
    say "保留配置 $PREFIX/etc/numdeck（要删加 --purge）"
fi

if [ "$DRY_RUN" -eq 0 ] && command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules >/dev/null 2>&1 || true
fi

say "完成"
