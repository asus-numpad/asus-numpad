#!/bin/sh
# numdeck 通用安装脚本（POSIX sh，任何 Linux 都能跑，不依赖包管理器）
#
#   sudo ./install.sh                  # 装到 /
#   sudo ./install.sh --prefix /usr/local
#   sudo ./install.sh --dry-run        # 只打印将要做什么
#   sudo ./install.sh --no-service     # 只装文件，不碰 systemd / udev
#
# 与 deb/rpm/arch 包的关系：这三个包做的事和本脚本等价，只是把依赖交给包管理器。
# 用本脚本装的文件不归包管理器管，卸载请用同目录的 uninstall.sh。

set -eu

PREFIX="/"
DRY_RUN=0
NO_SERVICE=0
SKIP_IF_PRESENT=0

while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)      PREFIX="${2:?--prefix 需要路径}"; shift 2 ;;
        --dry-run)     DRY_RUN=1; shift ;;
        --no-service)  NO_SERVICE=1; shift ;;
        --keep-config) SKIP_IF_PRESENT=1; shift ;;
        -h|--help)     sed -n '2,12p' "$0"; exit 0 ;;
        *)             echo "未知参数: $1" >&2; exit 2 ;;
    esac
done

[ "$(id -u)" -eq 0 ] || { echo "需要 root（sudo $0 ...）" >&2; exit 1; }

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
PREFIX=${PREFIX%/}
DEST="$PREFIX"

say() { printf '==> %s\n' "$*"; }
run() {
    if [ "$DRY_RUN" -eq 1 ]; then
        printf '    [dry-run] %s\n' "$*"
    else
        "$@"
    fi
}

[ -x "$HERE/usr/bin/numdeck" ] || { echo "包内缺少 usr/bin/numdeck" >&2; exit 1; }

say "安装到 ${DEST:-/}"
run install -d "$DEST/usr/bin"
run install -m 0755 "$HERE/usr/bin/numdeck" "$DEST/usr/bin/numdeck"

if [ -d "$HERE/usr/lib/systemd/system" ]; then
    run install -d "$DEST/usr/lib/systemd/system"
    run install -m 0644 "$HERE/usr/lib/systemd/system/numdeck.service" \
        "$DEST/usr/lib/systemd/system/numdeck.service"
fi
if [ -d "$HERE/usr/lib/udev/rules.d" ]; then
    run install -d "$DEST/usr/lib/udev/rules.d"
    run install -m 0644 "$HERE/usr/lib/udev/rules.d/99-numdeck.rules" \
        "$DEST/usr/lib/udev/rules.d/99-numdeck.rules"
fi
if [ -d "$HERE/usr/share/man/man1" ]; then
    run install -d "$DEST/usr/share/man/man1"
    run install -m 0644 "$HERE/usr/share/man/man1/numdeck.1" \
        "$DEST/usr/share/man/man1/numdeck.1"
fi
if [ -d "$HERE/usr/share/numdeck" ]; then
    run install -d "$DEST/usr/share/numdeck"
    for f in "$HERE"/usr/share/numdeck/*; do
        [ -e "$f" ] || continue
        if [ -d "$f" ]; then
            run cp -R "$f" "$DEST/usr/share/numdeck/"
        else
            run install -m 0644 "$f" "$DEST/usr/share/numdeck/"
        fi
    done
fi
if [ -d "$HERE/usr/share/doc/numdeck" ]; then
    run install -d "$DEST/usr/share/doc/numdeck"
    for f in "$HERE"/usr/share/doc/numdeck/*; do
        [ -e "$f" ] || continue
        run install -m 0644 "$f" "$DEST/usr/share/doc/numdeck/"
    done
fi

# 配置只在不存在时放，避免覆盖用户已调好的档位
if [ -f "$HERE/etc/numdeck/numdeck.json.sample" ]; then
    run install -d "$DEST/etc/numdeck"
    if [ -f "$DEST/etc/numdeck/numdeck.json" ] && [ "$SKIP_IF_PRESENT" -eq 1 ]; then
        say "保留现有配置 $DEST/etc/numdeck/numdeck.json"
    elif [ -f "$DEST/etc/numdeck/numdeck.json" ]; then
        say "配置已存在，写入示例文件 numdeck.json.sample"
        run install -m 0644 "$HERE/etc/numdeck/numdeck.json.sample" \
            "$DEST/etc/numdeck/numdeck.json.sample"
    else
        run install -m 0644 "$HERE/etc/numdeck/numdeck.json.sample" \
            "$DEST/etc/numdeck/numdeck.json"
    fi
fi

if [ "$NO_SERVICE" -eq 0 ] && [ "$DRY_RUN" -eq 0 ]; then
    # udev：hidraw 节点装包时已存在，reload 不会重放规则，必须显式 trigger
    if command -v udevadm >/dev/null 2>&1; then
        say "刷新 udev 规则"
        udevadm control --reload-rules >/dev/null 2>&1 || true
        for sub in input misc hidraw; do
            udevadm trigger --subsystem-match="$sub" --action=change >/dev/null 2>&1 || true
        done
        udevadm settle >/dev/null 2>&1 || true
    fi

    # systemd 用户级单元：装完随会话自启，不需要 root 常驻
    if command -v systemctl >/dev/null 2>&1 && [ "$PREFIX" = "" ]; then
        say "启用用户级服务"
        systemctl --global enable numdeck.service >/dev/null 2>&1 || true
        _installer="${SUDO_USER:-}"
        if [ -n "$_installer" ] && [ "$_installer" != "root" ]; then
            # linger：开机即拉起用户单元，不必先登录桌面
            loginctl enable-linger "$_installer" >/dev/null 2>&1 || true
            _uid=$(id -u "$_installer" 2>/dev/null || echo "")
            if [ -n "$_uid" ] && [ -d "/run/user/$_uid" ]; then
                su "$_installer" -s /bin/sh -c \
                    "XDG_RUNTIME_DIR=/run/user/$_uid systemctl --user daemon-reload; \
                     XDG_RUNTIME_DIR=/run/user/$_uid systemctl --user enable --now numdeck.service" \
                    >/dev/null 2>&1 || true
            fi
        fi
    fi
elif [ "$NO_SERVICE" -eq 1 ]; then
    say "--no-service：跳过 systemd / udev（容器或手工管理时用）"
fi

say "完成。自检: numdeck check"
