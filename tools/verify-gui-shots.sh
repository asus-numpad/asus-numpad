#!/usr/bin/env bash
# 无人值守自检：把所有页面离屏渲染一遍，看有没有崩、有没有画出空白窗口。
#
# 这块硬件平时不在手上，界面的问题基本靠这条路兜住：
#   - 页面渲染时 panic（退化网格、反向矩形、零面积预览之类）→ 进程非零退出 / 不出图
#   - 帧还没画完就退出 → 出图但尺寸不对
#   - 渲染成一片纯色（字体没加载、整页被裁）→ 出图但几乎没有非背景像素
#
# 用法：
#   tools/verify-gui-shots.sh              # 默认全部页面 + 深浅两套配色
#   tools/verify-gui-shots.sh --keep       # 保留中间 PPM（默认删）
#   tools/verify-gui-shots.sh --pages lighting,system
#
# 只能在有图形栈的机器上跑：macOS 本机、或者带 X11/Wayland 的 Linux 桌面。
# 纯 SSH 的 Linux 机器上这里给 SKIP 而不是 FAIL——不是代码有问题，是环境拍不了图。
set -u

cd "$(dirname "$0")/.." || exit 1

# 与 gui/src/app/mod.rs 的 `Page::from_env` 一一对应。写错页名不会报错，
# 程序会安静地落到灯光页——那样每一页拍出来都是同一张图，看着全 PASS 其实什么都没测到，
# 所以下面先用白名单卡一道。
PAGES="light driver extend calib system command settings"
KEEP=0
while [ $# -gt 0 ]; do
    case "$1" in
        --keep) KEEP=1 ;;
        --pages) shift; PAGES="${1//,/ }" ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        *) echo "未知参数: $1" >&2; exit 2 ;;
    esac
    shift
done

BIN="target/debug/numdeck-gui"
OUT="target/gui-shots"
PASS=0
FAIL=0
SKIP=0
# 每张图的指纹：页面名写错、渲染提前返回、整页被裁，都可能让不同页拍出一模一样的图。
# 只看「有没有出图」是抓不到这种的，最后比一遍指纹。
DIGESTS=""

say() { printf '%s\n' "$*"; }

# macOS 自带的是 `md5 -q`，GNU 系是 `md5sum`；两者都没有时就退回 `shasum`
digest() {
    if command -v md5sum >/dev/null 2>&1; then
        md5sum "$1" | cut -d' ' -f1
    elif command -v md5 >/dev/null 2>&1; then
        md5 -q "$1"
    else
        shasum "$1" | cut -d' ' -f1
    fi
}
pass() { PASS=$((PASS + 1)); say "PASS  $*"; }
fail() { FAIL=$((FAIL + 1)); say "FAIL  $*"; }
skip() { SKIP=$((SKIP + 1)); say "SKIP  $*"; }

# 1. 能不能拍
if [ "$(uname -s)" = "Darwin" ]; then
    :
elif [ -n "${WAYLAND_DISPLAY:-}${DISPLAY:-}" ]; then
    :
else
    say "== 离屏渲染 =="
    skip "没有图形会话（DISPLAY / WAYLAND_DISPLAY 都空），跳过全部页面"
    say "----------------------------------------------------------------"
    say "汇总: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
    exit 0
fi

if [ ! -x "$BIN" ]; then
    say "先编译（没找到 $BIN）"
    if ! cargo build --offline -q 2>/dev/null && ! cargo build -q; then
        fail "GUI 编不出来"
        say "汇总: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
        exit 1
    fi
fi

mkdir -p "$OUT"
say "== 逐页离屏渲染 =="

ppm_pixels() { # $1=ppm → 非背景像素占比（0~100 的整数），解析失败回 -1
    python3 - "$1" <<'PY'
import sys
p = sys.argv[1]
try:
    with open(p, "rb") as f:
        data = f.read()
except OSError:
    print(-1); raise SystemExit
# 头：P6 <w> <h> <max>\n（本项目写出的 PPM 就这一种形态，够用）
try:
    parts = data.split(b"\n", 3)
    if parts[0].strip() != b"P6":
        print(-1); raise SystemExit
    w, h = (int(x) for x in parts[1].split())
    body = parts[3]
except Exception:
    print(-1); raise SystemExit
n = min(w * h, len(body) // 3)
if n <= 0:
    print(-1); raise SystemExit
step = max(1, n // 20000)   # 抽样即可，不必逐像素
sample = 0
# 以左上角像素为背景色基准；窗口背景是整块的，这个假设成立
bg = body[0:3]
diff = 0
for i in range(0, n, step):
    o = i * 3
    px = body[o:o + 3]
    if abs(px[0] - bg[0]) + abs(px[1] - bg[1]) + abs(px[2] - bg[2]) > 12:
        diff += 1
    sample += 1
print(int(diff * 100 / sample) if sample else -1)
PY
}

KNOWN=" light driver extend calib system command settings "
for page in $PAGES; do
    case "$KNOWN" in
        *" $page "*) ;;
        *) fail "未知页名 '$page'（会静默落到灯光页，等于没测）"; continue ;;
    esac
    for theme in light dark; do
        shot="$OUT/${page}-${theme}.ppm"
        rm -f "$shot"
        # NUMDECK_GUI_THEME 存在与否都要能跑：不认这个变量时就是默认配色
        if NUMDECK_GUI_FULL=1 \
           NUMDECK_GUI_PAGE="$page" \
           NUMDECK_GUI_THEME="$theme" \
           NUMDECK_GUI_SHOT="$PWD/$shot" \
           "$BIN" >"$OUT/${page}-${theme}.log" 2>&1; then
            :
        else
            # 页面名写错时程序按默认页启动，不报错；这里以「有没有出图」为准
            :
        fi

        if [ ! -s "$shot" ]; then
            fail "$page/$theme 没有出图（看 $OUT/${page}-${theme}.log）"
            continue
        fi
        DIGESTS="$DIGESTS $(digest "$shot")"
        ratio=$(ppm_pixels "$shot")
        size=$(wc -c <"$shot" | tr -d ' ')
        if [ "$ratio" -lt 0 ]; then
            fail "$page/$theme 出图但读不出像素（$size 字节）"
        elif [ "$ratio" -lt 3 ]; then
            fail "$page/$theme 几乎全是一片底色（非背景像素 ${ratio}%），像是没画出来"
        else
            pass "$page/$theme 渲染正常（非背景像素 ${ratio}%，${size} 字节）"
        fi
    done
done

say "== 指纹去重 =="
total=$(printf '%s\n' $DIGESTS | grep -c . || true)
uniq=$(printf '%s\n' $DIGESTS | grep . | sort -u | grep -c . || true)
if [ "${total:-0}" -ge 2 ] && [ "${uniq:-0}" -lt "${total:-0}" ]; then
    fail "有 $((total - uniq)) 张图与其他页完全相同（页名写错或渲染提前返回）"
else
    pass "每张图内容各异（$uniq/$total 个不同指纹）"
fi

say "----------------------------------------------------------------"
say "汇总: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
if [ "$KEEP" -eq 0 ] && [ "$FAIL" -eq 0 ]; then
    rm -rf "$OUT"
fi
[ "$FAIL" -eq 0 ]
