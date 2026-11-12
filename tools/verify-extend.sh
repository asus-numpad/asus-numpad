#!/usr/bin/env bash
# 扩展功能（自定义布局 / 自定义报文 / 压感切换 / 全局热键）验证脚本。
#
#   tools/verify-extend.sh              # 自动项全跑，结果写 verify-extend.txt
#   tools/verify-extend.sh --keep       # 保留临时配置，方便手工接着试
#   tools/verify-extend.sh --bin PATH   # 指定二进制（默认 target/release/numdeck）
#
# 设计原则：
#   1. 自动项全部**不改系统状态**——临时配置写进 mktemp 目录，用 `--config` 指过去，
#      不碰 ~/.config/numdeck/numdeck.json，不写 hidraw，不重启服务。
#   2. 需要真机手感/硬件反馈的项**不自动跑**，只打印可以直接粘贴的命令，由人确认。
#   3. 每项都有 PASS/FAIL/SKIP 判定，最后给汇总，方便贴回 issue。
set -uo pipefail

KEEP=0
BIN=""
while (($#)); do
    case "$1" in
        --keep) KEEP=1 ;;
        --bin) shift; BIN="${1:-}" ;;
        -h|--help) sed -n '2,16p' "$0"; exit 0 ;;
        *) echo "未知参数: $1" >&2; exit 2 ;;
    esac
    shift
done

# rustup 装的 cargo 只在 ~/.cargo/bin，非交互 shell 里常常不在 PATH 上
export PATH="$HOME/.cargo/bin:$PATH"

# 相对路径的 --bin 要按「调用者所在目录」解析：脚本马上会 cd 到仓库根，cd 之后同一个
# 相对路径指向别处（把包解压到任意目录、在上一层 `bash pkg/tools/verify-extend.sh
# --bin ./numdeck` 就会踩到），第 1 步会误报「找不到可执行文件」。
if [[ -n "$BIN" && "$BIN" != /* ]]; then
    if BIN_DIR="$(cd -- "$(dirname -- "$BIN")" 2>/dev/null && pwd)"; then
        BIN="$BIN_DIR/$(basename -- "$BIN")"
    fi
fi

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 1
[[ -n "$BIN" ]] || BIN="$ROOT/target/release/numdeck"
OUT="$ROOT/verify-extend.txt"

PASS=0; FAIL=0; SKIP=0
declare -a FAILED=()

say() { printf '%s\n' "$*"; }
hr()  { printf '%s\n' "----------------------------------------------------------------"; }

pass() { PASS=$((PASS+1)); say "[PASS] $*"; }
fail() { FAIL=$((FAIL+1)); FAILED+=("$*"); say "[FAIL] $*"; }
skip() { SKIP=$((SKIP+1)); say "[SKIP] $*"; }

{
say "numdeck 扩展功能验证"
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
else
    if "$BIN" --version >/dev/null 2>&1; then
        pass "可执行: $("$BIN" --version 2>&1 | head -1)"
    else
        fail "二进制无法运行"
    fi
    if file "$BIN" 2>/dev/null | grep -q ELF; then
        pass "$(file -b "$BIN" | cut -c1-80)"
    else
        skip "file 命令不可用或不是 ELF"
    fi
fi
hr

# ---------------------------------------------------------------------------
# 2. 离线用例：扩展相关的单元测试
# ---------------------------------------------------------------------------
say "2. 单元测试（扩展相关）"
# 两种跑法要区分开：仓库里跑（有 Cargo.toml）才做源码级用例；只拿到发下去的预编译
# 二进制时，机器上就算装了 cargo，在没有 Cargo.toml 的目录里跑 `cargo test` 也只会
# 报「could not find Cargo.toml」，那是脚本自己的错，不该记成 FAIL。
if [ ! -f Cargo.toml ]; then
    skip "当前目录没有源码（没 Cargo.toml），跳过单元测试——预编译二进制请从第 3 步看起"
elif command -v cargo >/dev/null 2>&1; then
    if cargo test --lib -q 2>/tmp/verify-extend-cargo.err | tail -3 | grep -q "test result: ok"; then
        pass "cargo test --lib 全绿: $(cargo test --lib -q 2>/dev/null | grep 'test result' | tail -1)"
    else
        fail "cargo test --lib 有失败，详见 /tmp/verify-extend-cargo.err"
    fi
    for case in hard_press_toggles_through_event_path \
                pressure_switch_off_ignores_hard_press \
                hotkey_ctrl_alt_n_toggles_through_event_path \
                hotkey_action_on_is_idempotent \
                custom_layout_inherits_base_keys \
                extension_config_roundtrips_through_disk \
                extension_defaults_are_inert \
                bad_extension_values_fall_back_with_notes; do
        if cargo test --lib -q "$case" 2>/dev/null | grep -q "test result: ok"; then
            pass "用例 $case"
        else
            fail "用例 $case 未通过或不存在"
        fi
    done
else
    skip "没有 cargo，跳过单元测试（可用预编译二进制继续第 3 步起）"
fi
hr

# ---------------------------------------------------------------------------
# 3. 自定义配置真的被读进去了（不需要硬件）
# ---------------------------------------------------------------------------
say "3. 扩展配置读取（临时配置，不动系统配置）"
TMPD="$(mktemp -d)"
cleanup() { ((KEEP)) || command rm -rf "$TMPD"; }
trap cleanup EXIT
CFG="$TMPD/ext.json"
cat > "$CFG" <<'JSON'
{
  "custom_layouts": [
    {
      "name": "STICKER-4X4",
      "hid_ids": ["ASUE1416"],
      "keys": [
        "kp7 kp8 kp9 slash",
        "kp4 kp5 kp6 asterisk",
        "kp1 kp2 kp3 minus",
        "kp0 period plus enter"
      ],
      "columns": 4,
      "left": 20
    }
  ],
  "led_proto": { "template": [10, 1, 255, 0] },
  "pressure_switch": { "enabled": true, "percent": 65, "frames": 4, "cooldown_ms": 250.0 },
  "hotkeys": [{ "gesture": "ctrl+alt+n", "action": "on" }]
}
JSON

LIST_OUT="$("$BIN" --config "$CFG" layout 2>&1)"
if grep -q "STICKER-4X4" <<<"$LIST_OUT"; then
    pass "layout 列出了自定义布局 STICKER-4X4"
    grep -A1 "Custom layouts\|自定义布局\|自訂佈局" <<<"$LIST_OUT" | sed 's/^/    /'
else
    fail "layout 没有列出 STICKER-4X4（配置未生效）"
fi

CHK_OUT="$("$BIN" --config "$CFG" check 2>&1)"
check_has() { # $1=子串 $2=说明
    if grep -qF -- "$1" <<<"$CHK_OUT"; then
        pass "check 含「$2」: $1"
    else
        fail "check 缺「$2」: $1"
    fi
}
check_has "STICKER-4X4"                      "自定义布局计数"
check_has "0A 01 FF 00"                      "自定义点亮报文"
check_has "65%"                              "压感阈值"
check_has "ctrl+alt+n"                       "热键绑定"

ZH_OUT="$("$BIN" --lang zh-CN --config "$CFG" check 2>&1)"
if grep -qF "扩展开关" <<<"$ZH_OUT"; then
    pass "--lang zh-CN 生效（语言可随时切换）"
else
    fail "--lang zh-CN 没生效，输出仍是英文"
fi
TW_OUT="$("$BIN" --lang zh-TW --config "$CFG" check 2>&1)"
if grep -qF "擴充開關" <<<"$TW_OUT"; then
    pass "--lang zh-TW 生效"
else
    fail "--lang zh-TW 没生效"
fi

if "$BIN" --config "$CFG" layout list 2>&1 | grep -qi "unknown layout name"; then
    fail "layout list 子参数仍然报 Unknown layout name"
else
    pass "layout list 可当只读子参数用"
fi
hr

# ---------------------------------------------------------------------------
# 4. 坏值必须被挡住，而不是静默生效
# ---------------------------------------------------------------------------
say "4. 坏配置回落"
BAD="$TMPD/bad.json"
cat > "$BAD" <<'JSON'
{
  "custom_layouts": [
    { "name": "BROKEN", "keys": ["not-a-key q w e", "a s d f", "z x c v", "1 2 3 4"] }
  ],
  "led_proto": { "template": [1, 2, 3] },
  "pressure_switch": { "enabled": true, "percent": 0, "frames": 0, "cooldown_ms": -5 },
  "hotkeys": [{ "gesture": "hyper+q", "action": "on" }]
}
JSON
BAD_OUT="$("$BIN" --config "$BAD" check 2>&1)"
if grep -qiE "fall|回落|忽略|invalid|skip" <<<"$BAD_OUT"; then
    pass "坏值有提示，未静默生效"
else
    fail "坏值没有任何提示（可能被静默接受）"
fi
if grep -qE 'BROKEN.*(ignored|忽略)' <<<"$BAD_OUT"; then
    pass "非法键表 BROKEN 被拒并留下提示"
elif grep -q "BROKEN" <<<"$BAD_OUT"; then
    fail "非法键表 BROKEN 出现在输出里但没有被忽略的说明"
else
    fail "非法键表 BROKEN 既没被拒绝也没有任何提示（静默消失更糟）"
fi
if grep -qF "1 套" <<<"$BAD_OUT" || grep -qF "1 installed" <<<"$BAD_OUT"; then
    fail "坏配置下仍报有 1 套自定义布局"
else
    pass "坏配置下没有可用自定义布局"
fi
# 同一句提示只该出现一次（曾经 load + CLI 复核各报一遍）
DUPES=$(grep -c 'BROKEN.*ignored' <<<"$BAD_OUT")
if [[ "$DUPES" -eq 1 ]]; then
    pass "配置提示不重复"
else
    fail "配置提示重复了 $DUPES 次"
fi
hr

# ---------------------------------------------------------------------------
# 5. 真机项：只打印命令，不自动执行
# ---------------------------------------------------------------------------
say "5. 需要真机手感确认的项（本脚本不自动执行）"
say "  下面每条都不改配置，只临时跑一次；按键/亮度/切换看完按 Ctrl-C 退出。"
say
say "  5.1 压感切换（用力按触摸板应开关小键盘，轻点应正常出数字）"
say "      sudo $BIN --device /dev/input/eventX run"
say "      · 轻点：出数字；用明显更大的力按住不放：小键盘整体关/开一次"
say "      · 期望：一次按压只切一次，按住不连续翻转（冷却 600ms 默认 / 示例配置 250ms）"
say
say "  5.2 全局热键（按住 Ctrl+Alt+N 切一次）"
say "      sudo $BIN run --verbose   # 看日志里 hotkey 命中行"
say "      · 期望：按下即切，按住不放不重复；松开再按才切第二次"
say
say "  5.3 自定义报文（表外机型补背光）"
say "      sudo $BIN --config <(echo '{\"led_proto\":{\"template\":[10,1,255,0]}}') led 3"
say "      · 期望：能亮且档位随参数变化；不亮说明这台机的 FEATURE 报文不是这个形状"
say "      · 记下真正生效的字节序列，回填到配置的 template"
say
say "  5.4 自定义布局（贴纸/镭雕与官方表不一致的机型）"
say "      sudo $BIN --config <(echo '{\"custom_layouts\":[{\"name\":\"X\",\"hid_ids\":[\"<你的HID ID>\"],\"keys\":[...],\"columns\":5}]}') run --verbose"
say "      · 先跑 $BIN layout --ids 查自己的 HID ID"
say "      · 期望：按位置逐个点，键值与贴纸一致；错位就调 columns / left"
say
say "  看完请把上面两项的实际表现回填到 docs/machines.md 的对应机型小节。"
hr

# ---------------------------------------------------------------------------
say "汇总: PASS=$PASS FAIL=$FAIL SKIP=$SKIP"
if ((FAIL)); then
    say "失败项:"
    for f in "${FAILED[@]}"; do say "  - $f"; done
    say
    say "把这份输出（${OUT}）连同 $BIN --config <cfg> check 的全文贴到 issue 里即可定位。"
else
    say "自动项全部通过。真机项请按第 5 节逐条确认。"
fi
} 2>&1 | tee "$OUT"

say
say "结果已写入: $OUT"
if [[ "$FAIL" -gt 0 ]]; then exit 1; fi
exit 0
