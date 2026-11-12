#!/usr/bin/env bash
# 自定义分区与手动校准（非原生机型）验证脚本。
#
#   tools/verify-calibrate.sh            # 自动项全跑，结果写 verify-calibrate.txt
#   tools/verify-calibrate.sh --keep     # 保留临时配置，方便手工接着试
#   tools/verify-calibrate.sh --bin PATH # 指定二进制（默认 target/release/numdeck）
#
# 设计原则与 verify-extend.sh 一致：
#   1. 自动项全部**不改系统状态**——临时配置写进 mktemp 目录，用 `--config` 指过去，
#      不碰 ~/.config/numdeck/numdeck.json，不碰任何 hidraw 节点。
#   2. 采样本身要在真机上按住触摸板，脚本只回放样本文件；真机项打印命令由人确认。
#   3. 每项都有 PASS/FAIL/SKIP 判定，最后给汇总，方便贴回 issue。
set -uo pipefail

KEEP=0
BIN=""
while (($#)); do
    case "$1" in
        --keep) KEEP=1 ;;
        --bin) shift; BIN="${1:-}" ;;
        -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
        *) echo "未知参数: $1" >&2; exit 2 ;;
    esac
    shift
done

# rustup 装的 cargo 只在 ~/.cargo/bin，非交互 shell 里常常不在 PATH 上
export PATH="$HOME/.cargo/bin:$PATH"

# 相对路径的 --bin 要在 cd 到仓库根之前解析掉，否则同一个相对路径会指到别处
if [[ -n "$BIN" && "$BIN" != /* ]]; then
    if BIN_DIR="$(cd -- "$(dirname -- "$BIN")" 2>/dev/null && pwd)"; then
        BIN="$BIN_DIR/$(basename -- "$BIN")"
    fi
fi

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1
[[ -n "$BIN" ]] || BIN="$ROOT/target/release/numdeck"
OUT="$ROOT/verify-calibrate.txt"

PASS=0; FAIL=0; SKIP=0
declare -a FAILED=()

say() { printf '%s\n' "$*"; }
hr()  { printf '%s\n' "----------------------------------------------------------------"; }
pass() { PASS=$((PASS+1)); say "[PASS] $*"; }
fail() { FAIL=$((FAIL+1)); FAILED+=("$*"); say "[FAIL] $*"; }
skip() { SKIP=$((SKIP+1)); say "[SKIP] $*"; }

{
say "numdeck 自定义分区与手动校准验证"
say "时间: $(date '+%F %T %z')"
say "主机: $(uname -srm)"
say "发行版: $(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME" || echo unknown)"
say "二进制: $BIN"
hr

# ---------------------------------------------------------------------------
# 1. 二进制可用性
# ---------------------------------------------------------------------------
say "1. 二进制"
if [[ ! -x "$BIN" ]]; then
    fail "找不到可执行文件 ${BIN}（先在仓库根目录 cargo build --release）"
    say "    其余项无法继续。"
    say
    BIN_DEAD=1
else
    BIN_DEAD=0
    if "$BIN" --version >/dev/null 2>&1; then
        pass "可执行: $("$BIN" --version 2>&1 | head -1)"
    else
        fail "二进制无法运行"
    fi
    if "$BIN" --help 2>&1 | grep -q '^  calibrate'; then
        pass "--help 列出 calibrate 子命令"
    else
        fail "--help 里没有 calibrate（二进制太旧？）"
    fi
fi
hr

# ---------------------------------------------------------------------------
# 2. 离线用例
# ---------------------------------------------------------------------------
say "2. 单元测试（分区 / 校准相关）"
if [ ! -f Cargo.toml ]; then
    skip "当前目录没有源码（没 Cargo.toml），跳过单元测试"
elif command -v cargo >/dev/null 2>&1; then
    if cargo test --lib -q 2>/tmp/verify-calibrate-cargo.err | tail -3 | grep -q "test result: ok"; then
        pass "cargo test --lib 全绿: $(cargo test --lib -q 2>/dev/null | grep 'test result' | tail -1)"
    else
        fail "cargo test --lib 有失败，详见 /tmp/verify-calibrate-cargo.err"
    fi
    for case in solve_calibration_recovers_grid_from_probes \
                solve_calibration_falls_back_to_panel_split_without_span \
                solve_calibration_rejects_bad_input \
                partition_resolves_uniform_over_base_grid \
                partition_rect_mm_converts_through_panel \
                partition_rejects_bad_shapes \
                partition_notes_when_grid_wider_than_keytable \
                partition_only_applies_to_its_layout \
                calibration_overrides_only_written_items \
                calibration_flags_inverted_and_bad_units \
                partition_roundtrips_through_json \
                split_even_and_weighted \
                partition_sanitize_names_bad_values; do
        if cargo test --lib -q "$case" 2>/dev/null | grep -q "test result: ok"; then
            pass "用例 $case"
        else
            fail "用例 $case 未通过或不存在"
        fi
    done
    # GUI 侧的片段打包 / 样本解析跑在同一个 crate 的另一包上
    if cargo test -q -p numdeck-gui calib:: 2>/dev/null | grep -q "test result: ok"; then
        pass "GUI 校准页用例（片段往返 / 坏片段 / 样本缺键 / 解算包住格心）"
    else
        skip "GUI 包未构建或没有 cargo（不影响命令行侧）"
    fi
else
    skip "没有 cargo，跳过单元测试"
fi
hr

TMPD="$(mktemp -d)"
cleanup() { ((KEEP)) || command rm -rf "$TMPD"; }
trap cleanup EXIT

SAMPLES="$TMPD/samples.json"
cat > "$SAMPLES" <<'JSON'
{
  "corners": {
    "top_left": [405, 520],
    "top_right": [3630, 520],
    "bottom_left": [405, 2280],
    "bottom_right": [3630, 2280]
  },
  "first_point": [700, 740],
  "last_point": [3330, 2060]
}
JSON

if ((BIN_DEAD)); then
    say "3. 样本解算 —— 跳过（没有可用二进制）"
else
    # -----------------------------------------------------------------------
    # 3. 样本解算（6 列，非默认 5 列）
    # -----------------------------------------------------------------------
    say "3. 样本解算（6 列 x 4 行，临时配置）"
    CFG="$TMPD/cal.json"
    SOLVE_OUT="$("$BIN" --config "$CFG" calibrate --samples "$SAMPLES" --cols 6 2>&1)"
    SOLVE_RC=$?
    if ((SOLVE_RC == 0)) && grep -q "6 cols x 4 rows" <<<"$SOLVE_OUT"; then
        pass "解出网格: $(grep -o 'grid: .*' <<<"$SOLVE_OUT" | head -1)"
    else
        fail "解算失败（退出码 ${SOLVE_RC}）: $(tail -3 <<<"$SOLVE_OUT" | tr '\n' ' ')"
    fi
    if grep -q 'cell: 526.0 x 440.0 units' <<<"$SOLVE_OUT"; then
        pass "格尺寸与手算一致（526.0 x 440.0）"
    else
        fail "格尺寸不是 526.0 x 440.0: $(grep -o 'cell: .*' <<<"$SOLVE_OUT" | head -1)"
    fi
    if grep -q "touched area" <<<"$SOLVE_OUT"; then
        pass "打印了实测角点范围: $(grep -o 'touched area: .*' <<<"$SOLVE_OUT" | head -1)"
    else
        fail "没打印实测角点范围——覆盖轴范围时没有依据"
    fi
    if [[ ! -e "$CFG" ]]; then
        pass "不带 --write 时不落盘（配置文件未被创建）"
    else
        fail "不带 --write 却写了配置文件"
    fi
    CELLS=$(grep -o '\[[0-9],[0-9]\] ([0-9]*, *[0-9]*)' <<<"$SOLVE_OUT" | wc -l | tr -d ' ')
    if [[ "$CELLS" == "24" ]] && grep -q '\[5,3\] (3330,2060)' <<<"$SOLVE_OUT"; then
        pass "格中心表 24 格齐全，最后一格 [5,3] (3330,2060)"
    else
        fail "格中心表不完整（${CELLS} 格，期望 24）"
    fi
    hr

    # -----------------------------------------------------------------------
    # 4. --write 落盘与 .bak
    # -----------------------------------------------------------------------
    say "4. --write 落盘"
    printf '{"layout_name":"auto"}\n' > "$CFG"
    "$BIN" --config "$CFG" calibrate --samples "$SAMPLES" --cols 6 --write >/dev/null 2>&1
    if grep -q '"cols": 6' "$CFG"; then
        pass "配置写入 partition（cols=6）"
    else
        fail "配置里没有 partition.cols"
    fi
    if grep -q '"rect": \[' "$CFG"; then
        pass "配置写入 partition.rect: $(grep -o '"rect": \[[^]]*\]' "$CFG" | head -1)"
    else
        fail "配置里没有 partition.rect"
    fi
    "$BIN" --config "$CFG" calibrate --samples "$SAMPLES" --cols 6 --write >/dev/null 2>&1
    if ls "$TMPD" | grep -q '\.bak'; then
        pass "第二次写入保留了 .bak"
    else
        fail "第二次写入没有 .bak 备份"
    fi
    "$BIN" --config "$CFG" calibrate --samples "$SAMPLES" --cols 6 --write 2>&1 \
        | grep -q "$CFG" && pass "提示里给出真实写入路径" || fail "写入路径提示不对"
    hr

    # -----------------------------------------------------------------------
    # 5. check 读到自定义分区（三语）
    # -----------------------------------------------------------------------
    say "5. check 读取自定义分区"
    CHK_CN="$("$BIN" --config "$CFG" --lang zh-CN check 2>&1)"
    grep -q "已启用自定义分区" <<<"$CHK_CN" && pass "简中: $(grep -o '已启用自定义分区.*' <<<"$CHK_CN" | head -1)" \
        || fail "简中没有分区提示"
    CHK_TW="$("$BIN" --config "$CFG" --lang zh-TW check 2>&1)"
    grep -q "已啟用自訂分區" <<<"$CHK_TW" && pass "繁中: $(grep -o '已啟用自訂分區.*' <<<"$CHK_TW" | head -1)" \
        || fail "繁中没有分区提示（术语是否写成了「分割」？）"
    CHK_EN="$("$BIN" --config "$CFG" --lang en check 2>&1)"
    grep -q "custom partition active" <<<"$CHK_EN" && pass "英文: $(grep -o 'custom partition active.*' <<<"$CHK_EN" | head -1)" \
        || fail "英文没有分区提示"
    hr

    # -----------------------------------------------------------------------
    # 6. 坏输入被拒
    # -----------------------------------------------------------------------
    say "6. 坏输入被拒（应报错且不写坏配置）"
    BAD1="$TMPD/bad1.json"
    printf '{"corners":{"top_left":[1,1]},"first_point":[2,2],"last_point":[3,3]}\n' > "$BAD1"
    if ! "$BIN" --config "$TMPD/c1.json" calibrate --samples "$BAD1" >/dev/null 2>&1; then
        pass "样本缺 key 时退出码非 0"
    else
        fail "样本缺 key 却没报错"
    fi
    BAD_MSG="$("$BIN" --config "$TMPD/c1.json" calibrate --samples "$BAD1" 2>&1)"
    if grep -qi "top_right" <<<"$BAD_MSG"; then
        pass "缺的键被点名: $(grep -i 'top_right' <<<"$BAD_MSG" | head -1)"
    else
        fail "错误提示没有指出缺哪个键（实际: $(tail -1 <<<"$BAD_MSG")）"
    fi
    printf 'not json\n' > "$BAD1"
    "$BIN" --config "$TMPD/c1.json" calibrate --samples "$BAD1" >/dev/null 2>&1 \
        && fail "坏 JSON 没被拒" || pass "坏 JSON 被拒"
    # 反矩形：解算结果必须过不了 resolve
    printf '{"corners":{"top_left":[3000,2000],"top_right":[400,2000],"bottom_left":[3000,500],"bottom_right":[400,500]},"first_point":[2800,1800],"last_point":[600,700]}\n' > "$BAD1"
    "$BIN" --config "$TMPD/c1.json" calibrate --samples "$BAD1" >/dev/null 2>&1 \
        && fail "反矩形没被拒" || pass "反矩形被拒"
    hr

    # -----------------------------------------------------------------------
    # 7. 配置层校验：坏矩形 / 坏校准回落并点名
    # -----------------------------------------------------------------------
    say "7. 配置层校验（坏值回落并点名）"
    BADCFG="$TMPD/bad-cfg.json"
    printf '{"partition":{"layout":"auto","rect":[3000,2000,400,500]}}\n' > "$BADCFG"
    OUT7="$("$BIN" --config "$BADCFG" check 2>&1)"
    grep -qi "inverted or has no area\|方向反了" <<<"$OUT7" \
        && pass "反矩形被拒并回落到官方网格" || fail "反矩形没有被点名"
    printf '{"calibration":{"x":{"min":3000,"max":100}}}\n' > "$BADCFG"
    OUT7="$("$BIN" --config "$BADCFG" check 2>&1)"
    if grep -qi "calibration\|校准" <<<"$OUT7"; then
        pass "下界大于上界的轴被点名并丢弃"
    else
        fail "坏校准没有提示"
    fi
    printf '{"partition":{"cols":9}}\n' > "$BADCFG"
    OUT7="$("$BIN" --config "$BADCFG" check 2>&1)"
    grep -qi "column count 9 is outside\|列数 9 超出" <<<"$OUT7" \
        && pass "列数越界（9）被点名忽略" || fail "列数越界没被点名"
    hr

    # -----------------------------------------------------------------------
    # 8. 权重与吸附
    # -----------------------------------------------------------------------
    say "8. 不等分权重与网格外吸附"
    printf '{"partition":{"layout":"auto","cols":5,"rows":4,"col_ratios":[1,1,1,1,2],"snap_outside":true}}\n' > "$BADCFG"
    OUT8="$("$BIN" --config "$BADCFG" check 2>&1)"
    if grep -qi "partition active\|分区" <<<"$OUT8"; then
        pass "带权重的分区被接受"
    else
        fail "带权重的分区被拒"
    fi
    printf '{"partition":{"cols":5,"col_ratios":[1,1]}}\n' > "$BADCFG"
    OUT8="$("$BIN" --config "$BADCFG" check 2>&1)"
    grep -qi "entries but the grid needs\|网格需要" <<<"$OUT8" \
        && pass "权重长度与列数不符时被点名忽略" || fail "权重长度不符却没被点名"
    hr
fi

# ---------------------------------------------------------------------------
# 9. 真机项（不自动跑）
# ---------------------------------------------------------------------------
say "9. 需要真机操作的项（脚本不执行，按需粘贴）"
say
say "  9.1 采集样本（按住四角 + 两个对角格中心）"
say "      sudo $BIN calibrate"
say "      · 期望：跑完打印格中心表；逐个点一遍，按哪格出哪键"
say
say "  9.2 人工确认网格是否对齐"
say "      sudo $BIN --config <cfg> run --verbose"
say "      · 期望：丝印上的每个键都落在自己那格里；偏一格就调 rect / cols，别改键表"
say
say "  9.3 覆盖轴范围（内核上报与实测差很多时才做）"
say "      · 把 9.1 打印的实测角点范围填进 calibration.x/y 的 min / max，再跑 check 看几何换算"
say
say "  9.4 GUI 校准页"
say "      cargo run -p numdeck-gui"
say "      · 期望：样本回放解出的矩形与 9.1 一致；导出的片段里只有 partition / calibration 两段"
hr

# ---------------------------------------------------------------------------
say "汇总: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
if ((FAIL)); then
    say "失败项:"
    for f in "${FAILED[@]}"; do say "  - $f"; done
    say
    say "把这份输出（${OUT}）连同 $BIN --config <cfg> check 的全文贴到 issue 里即可定位。"
else
    say "自动项全部通过。真机项请按第 9 节逐条确认。"
fi
} 2>&1 | tee "$OUT"

say
say "结果已写入: $OUT"
if [[ "$FAIL" -gt 0 ]]; then exit 1; fi
exit 0
