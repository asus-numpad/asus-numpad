//! 配置：默认值即官方 AlgoParameters 原值，改动即偏离官方。

use crate::keycodes as kc;
use crate::layout::KeyMode;
use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

// --------------------------------------------------------------------------- //
// 官方常量（AsusNUMPADDriver V17.0.0.27 / AsusNUMPADFilter.inf AlgoParameters）
// --------------------------------------------------------------------------- //

/// 官方注释 "1 frame 8ms" → 125 Hz
pub const FRAME_MS: u32 = 8;
pub const FRAMES_PER_SEC: u32 = 1000 / FRAME_MS;

/// NUMPAD_00 ContSendKey_Trig_FC = 25 (200 ms)
pub const NUM_KEY_CONT_TRIGGER_FRAMES: u32 = 25;
/// NUMPAD_01 NumLock_Trigger_FC = 40 (320 ms, 通用)
pub const NUMLOCK_TRIGGER_FRAMES: u32 = 40;
/// NUMPAD_GH3XLNDZ_01 NumLock_Trigger_FC = 62 (496 ms, 本机)
pub const NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ: u32 = 62;
/// NUMPAD_GH2XLNDZ_01 同族取值
pub const NUMLOCK_TRIGGER_FRAMES_GH2XLNDZ: u32 = 62;
/// NUMPAD_06 NumAdv_Trigger_FC = 40 (320 ms)
pub const NUMADV_TRIGGER_FRAMES: u32 = 40;
/// NUMPAD_02 Max_Numpad_Work_Time_T = 120000 ms
pub const MAX_WORK_TIME_MS: u32 = 120_000;
/// NUMPAD_03 iUserEnableNumpadAlgo = 1
pub const ENABLE_NUMPAD_ALGO: u8 = 1;
/// NUMPAD_04 iNumpad_FeatureSwitch = 255
pub const NUMPAD_FEATURE_SWITCH: u8 = 0xFF;
/// NUMPAD_07 EnAppGesture_Trig_UM = 7000 µm
pub const APP_GESTURE_TRIG_UM: f64 = 7000.0;
/// NUMPAD_08 iNumLockMaxFailDeltaDist_Ratio = 3
pub const NUMLOCK_MAX_FAIL_DELTA_RATIO: f64 = 3.0;
/// NUMPAD_09 iNumLockMaxVaildDist = 2741 µm
pub const NUMLOCK_MAX_VALID_DIST_UM: f64 = 2741.0;
/// NUMPAD_10 iNumLockMaxFailNum = 3
pub const NUMLOCK_MAX_FAIL_NUM: u32 = 3;

/// GESTURE_00 dwSupport_MixedMode = 1
pub const GESTURE_SUPPORT_MIXED: u8 = 1;
/// GESTURE_01 Gesture_TouchSendKey_Trig_FC = 10 (80 ms)
pub const GESTURE_TOUCH_SENDKEY_FRAMES: u32 = 10;
/// GESTURE_02 Gesture_DelayTouchSendKey_Ratio = 3
pub const GESTURE_DELAY_TOUCH_SENDKEY_RATIO: u32 = 3;
/// GESTURE_03/04 Min_Fir/Sec_Det_TAP_Interval_T = 350 ms
pub const TAP_INTERVAL_MIN_MS: f64 = 350.0;
/// GESTURE_05 Gesture_Trig_UM = 4838 µm
pub const GESTURE_TRIG_UM: f64 = 4838.0;
/// GESTURE_06 iTAPPING_THRESHOLD_MM = 10 mm
pub const TAP_THRESHOLD_MM: f64 = 10.0;
/// GESTURE_07 iGestureEdgeTempDataLen = 30 帧
pub const GESTURE_EDGE_TEMP_FRAMES: u32 = 30;
/// GESTURE_08 iscroll_finger_dist_MM = 40 mm
pub const SCROLL_FINGER_DIST_MM: f64 = 40.0;
/// GESTURE_09 iMixedMode_FeatureSwitch = 255
pub const MIXED_MODE_FEATURE_SWITCH: u8 = 0xFF;

/// LED_00 iLEDLevel = 1（无亮度档位）
pub const LED_LEVEL: u8 = 1;
/// LED_03 SupportLED = 1
pub const SUPPORT_LED: u8 = 1;
/// APSettingParameters COMMONFEATURE_00
pub const CALCULATOR_FEATURE: u8 = 1;
/// FORCE_07 Normal = 1 / FORCE_08 Particular（小键盘态）= 2
pub const FORCE_LEVEL_NORMAL: u8 = 1;
pub const FORCE_LEVEL_NUMPAD: u8 = 2;

/// 点灯命令取值（官方仅两态，另加一个解锁值）
///
/// - `0x01` / `0x00`：亮 / 灭，hidraw FEATURE 0x0d 与 i2c 两条通道通用
/// - `0x60`：i2c 直写 EC 的解锁前置字，hidraw FEATURE 通道**不需要**；
///   亮度档 `0x41..=0x48` 对应 1..8 档
pub const VAL_UNLOCK: u8 = 0x60;
pub const VAL_ON: u8 = 0x01;
pub const VAL_OFF: u8 = 0x00;
/// 亮度档起点：`0x41` 为 1 档，`0x48` 为 8 档
pub const VAL_BRIGHTNESS_BASE: u8 = 0x40;
pub const VAL_BRIGHTNESS_MAX: u8 = 8;

/// 手动调光的默认档数（`asus-numpad bright 1|2|3|4`，另有 0 = 熄灭）。
///
/// 四档在「熄灭 + 硬件全档」这条轴上**线性等分**（`0x00` 与 `0x41..=0x48`，共 9 点），
/// 首档就是熄灭、末档是最亮：默认 8 档表 → `0x00 / 0x43 / 0x45 / 0x48`。
///
/// 这只是默认值：`manual_brightness_levels` 给了自定义表时，档数以该表长度为准，
/// 亮度直接取表内写死的值（见 [`Config::manual_steps`]）。
pub const MANUAL_BRIGHTNESS_STEPS: u8 = 4;

/// 自定义手动档位表的长度上限（受硬件 0x41..=0x48 八个物理档限制）。
pub const MANUAL_BRIGHTNESS_MAX_STEPS: usize = 8;

/// 自定义档位是否合法：`0`（熄灭）或硬件亮度档 `0x41..=0x48`。
///
/// 允许 0 是为了让「从 0 到最大线性四档」能写成 `[0, 0x43, 0x46, 0x48]`，
/// 与 `bright 0` 的熄灭语义一致；其余值一律视为手写配置错误。
pub fn manual_level_allowed(v: u8) -> bool {
    v == VAL_OFF
        || (VAL_BRIGHTNESS_BASE + 1..=VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX).contains(&v)
}

/// 默认手动档位表：四档「从熄灭线性拉到最亮」= `0x00 / 0x43 / 0x45 / 0x48`。
///
/// 与 1.2.2 之前「不配置时现算线性等分」的结果逐字节相同，区别只是**写进默认配置**：
/// 用户打开 JSON 就能看到每一档的字节值，按档改成任意硬件支持的亮度
/// （`0x00` 或 `0x41..=0x48`）即可，不必再自己推算线性关系。
pub const DEFAULT_MANUAL_LEVELS: [u8; 4] = [VAL_OFF, 0x43, 0x45, 0x48];

/// `manual_brightness_levels` 的默认值（配置里不写这一项时用）。
fn default_manual_brightness_levels() -> Option<Vec<u8>> {
    Some(DEFAULT_MANUAL_LEVELS.to_vec())
}

/// 单档改写失败的原因（由 [`Config::set_manual_level`] 返回，CLI 侧映射成文案）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelSetError {
    /// 亮度值不是硬件支持的 `0x00` 或 `0x41..=0x48`
    Value,
    /// 档号越界：必须 `1..=当前档数`，且不超过 [`MANUAL_BRIGHTNESS_MAX_STEPS`]
    Step,
    /// 改完破坏了「从暗到亮」的顺序（允许相邻相等，但不允许后一档比前一档暗）
    Order,
}

pub const PROC_INPUT_DEVICES: &str = "/proc/bus/input/devices";
/// 配置文件正式路径（与 rpm 包内安装路径一致）。
pub const CONFIG_PATH: &str = "/etc/asus-numpad/asus-numpad.json";
/// 历史版本的配置路径（都平铺在 `/etc` 下），按新→旧顺序探测。
///
/// * `/etc/asus-numpad.json` —— 改名后中间版本的路径；
/// * `/etc/rog-numpad.json`   —— 更早 rog-numpad 时代的路径。
///
/// 仅用于一次性迁移与只读兼容，新写入一律走 [`CONFIG_PATH`]。
pub const LEGACY_CONFIG_PATHS: &[&str] = &["/etc/asus-numpad.json", "/etc/rog-numpad.json"];
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

// --------------------------------------------------------------------------- //
// 枚举
// --------------------------------------------------------------------------- //

/// 背光与系统 NumLock 的联动方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum NumLockMode {
    /// 只在 NumLock 已开时点灯；关闭时补按一次 NumLock
    #[default]
    Sync,
    /// 无条件切换 NumLock
    Toggle,
    /// 不动 NumLock
    Off,
}

impl NumLockMode {
    pub fn as_str(self) -> &'static str {
        match self {
            NumLockMode::Sync => "sync",
            NumLockMode::Toggle => "toggle",
            NumLockMode::Off => "off",
        }
    }
}

/// 计算器联动模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CalcMode {
    /// 长按左上图标区（NUMADV_Trigger_FC = 40 帧）
    #[default]
    Long,
    /// 关闭
    Off,
}

/// 混合模式开关：官方 dwSupport_MixedMode = 1 的落地开关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MixedMode {
    /// 有虚拟触摸板就开启（等同官方默认）
    #[default]
    Auto,
    /// 强制开启
    On,
    /// 纯数字键盘（光标不可用）
    Off,
}

impl MixedMode {
    pub fn is_enabled(self) -> bool {
        !matches!(self, MixedMode::Off)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MixedMode::Auto => "auto",
            MixedMode::On => "true",
            MixedMode::Off => "false",
        }
    }
}

/// 兼容上游配置写法：`"auto"` / `true` / `false` / `"on"` / `"off"`。
impl<'de> Deserialize<'de> for MixedMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = MixedMode;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("auto / true / false")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<MixedMode, E> {
                Ok(if v { MixedMode::On } else { MixedMode::Off })
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<MixedMode, E> {
                match v.to_ascii_lowercase().as_str() {
                    "auto" => Ok(MixedMode::Auto),
                    "on" | "true" | "yes" | "1" => Ok(MixedMode::On),
                    "off" | "false" | "no" | "0" => Ok(MixedMode::Off),
                    other => Err(E::custom(crate::i18n::tf(
                        "cfg.err.mixed_mode",
                        &[("v", other)],
                    ))),
                }
            }
        }
        d.deserialize_any(V)
    }
}

/// 键位风格（无效值回落 keypad，与上游一致）。
impl<'de> Deserialize<'de> for KeyMode {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(s.parse().unwrap_or(KeyMode::Keypad))
    }
}

// --------------------------------------------------------------------------- //
// 配置结构
// --------------------------------------------------------------------------- //

/// 全部字段均可缺省；缺省即官方值。
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    // ---- 官方参数覆盖 ----
    /// 一帧的长度（ms）：状态机的所有「帧数」阈值与主循环的等待超时都用它。
    ///
    /// 唯一真源。旧配置里同义的 `poll_interval_ms` 由 [`Config::from_json`] 折到本字段，
    /// 不再单独保存——两个名字各存一份时，改一个另一个不动，帧数与实际节拍会对不上。
    pub frame_ms: u32,
    pub numlock_trigger_frames: u32,
    pub cont_sendkey_trigger_frames: u32,
    pub numadv_trigger_frames: u32,
    pub max_work_time_ms: u32,
    pub app_gesture_trig_um: f64,
    pub numlock_max_valid_dist_um: f64,
    pub numlock_max_fail_num: u32,
    pub gesture_trig_um: f64,
    pub tap_threshold_mm: f64,
    pub scroll_finger_dist_mm: f64,
    // ---- 混合模式手势（NUMPADGESTURE_00..09 官方原值） ----
    /// GESTURE_01 = 10 帧 (80 ms)：手势态触摸后延迟出键，区分「想按键」与「想移动光标」
    pub touch_sendkey_frames: u32,
    /// GESTURE_02 = 3：已在手势中的再次按下，延迟放大为 3 倍（240 ms）
    pub touch_sendkey_ratio: u32,
    /// GESTURE_03/04 = 350 ms：首/次 tap 间隔，用于区分点击与手势
    pub tap_interval_min_ms: f64,
    /// GESTURE_07 = 30 帧：手势边缘临时数据长度
    pub gesture_edge_frames: u32,
    /// 官方未给重复间隔，推断值
    pub repeat_interval_ms: f64,
    // ---- 布局 ----
    /// 官方布局名：`auto` = 按触摸板 HID 硬件 ID 自动识别（推荐）；
    /// 也可写死某套布局（`GH3XLNDZ` / `GH2XLNDZ` / `GH4XLNDZ` 等，见 `asus-numpad layout`）。
    pub layout_name: String,
    /// 允许使用「尚未收录键表」的同族布局出键。
    ///
    /// 默认 `false`：`auto` 识别到非 GH3XLNDZ 布局时只报告不跑，避免键位表不同
    /// 导致按错键；确认可用后再置 `true`（或命令行加 `--allow-unverified`）。
    pub allow_unverified_layout: bool,
    /// 手动指定的机型配置 id（如 `g614`）；不写 = 按 DMI 型号 / 触摸板模组自动识别。
    ///
    /// 只给「硬件信息读不准」的机器留的后路：虚拟机、DMI 被写成占位串的整机、
    /// 换过主板、白牌机箱等。命令行 `--sku` 优先级更高；
    /// `asus-numpad sku auto` 可清空本字段、回到自动识别。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    // ---- 按压力度（FORCE_07 = 1 / FORCE_08 = 2 档位） ----
    /// 官方只给档位号，未在 INF 暴露原始压力值；0 = 不做压力过滤
    pub pressure_numpad_min: i32,
    // ---- 行为 ----
    pub key_mode: KeyMode,
    pub numlock: NumLockMode,
    pub calc_gesture: bool,
    pub calc_gesture_mode: CalcMode,
    /// `auto` 只在点亮期间把双指及以上放给桌面，单指留在数字区（避免从边缘
    /// 留白、格缝处拖走光标）；`on` 恢复「键区外一律透传」；`off` 纯数字键盘
    pub mixed_mode: MixedMode,
    pub grab: bool,
    // ---- 几何兜底（resolution 缺失时使用） ----
    pub touchpad_width_mm: f64,
    pub touchpad_height_mm: f64,
    // ---- 界面语言 ----
    /// 界面语言：`auto`（跟随系统 locale）/ `zh-CN` / `zh-TW` / `en`。
    /// 优先级：环境变量 `ASUS_NUMPAD_LANG` > 命令行 `--lang` > 本字段 > 系统 locale。
    pub language: String,
    // ---- 背光亮度（跟随键盘背光） ----
    /// 是否让数字区亮度跟随键盘背光（读 asus::kbd_backlight 换算成 0x41..0x48）
    pub follow_keyboard_brightness: bool,
    /// 键盘背光 sysfs 目录，默认 `/sys/class/leds/asus::kbd_backlight`
    pub keyboard_brightness_dir: String,
    /// 数字区亮度档命令字，从暗到亮；留空 = 不跟随（退回 0x01 点亮）。
    ///
    /// 接受三种写法：整数（`65`）、十六进制字符串（`"0x41"`）、十进制字符串（`"65"`）；
    /// 写回时统一成整数数组——早期版本写的是 `0x41` 字符串，读不回来会让整份配置
    /// 悄悄退回默认值（跟随开关、手动档一起丢失），这里必须兼容。
    #[serde(
        default = "default_brightness_levels",
        deserialize_with = "de_brightness_levels"
    )]
    pub brightness_levels: Vec<u8>,
    /// 键盘背光轮询间隔（ms）
    pub brightness_poll_ms: u32,
    /// 手动调光的固定档（0 = 熄灭；1..= [`Config::manual_steps`] 从最暗到最亮）。
    /// 仅在「不跟随键盘背光」时作为常亮档使用；`asus-numpad bright N` 会写入这里。
    pub manual_brightness_step: u8,
    /// 手动档位表（从暗到亮）：**默认显式给出四档** `[0, 0x43, 0x45, 0x48]`
    /// （0x00 灭 → 0x48 最亮，线性等分）。
    ///
    /// 每一项都可以换成任意一个硬件支持的亮度：`0x00` 或 `0x41..=0x48`（共 8 个亮档）；
    /// 档数由长度决定（1..=8），所以 `bright 1|2|…|N` 的 N 跟着表长走。
    /// 改单档可以直接用 `asus-numpad levels <N> <VALUE>`，也可以手改这里，
    /// 例如 `[65, 65, 67, 72]`（前两档一样亮）、`[65, 66, 67, 68, 72]`（五档更细）。
    /// 显式写 `null` = 交回驱动按线性等分现算（与默认表数值相同）。
    #[serde(
        default = "default_manual_brightness_levels",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_opt_brightness_levels"
    )]
    pub manual_brightness_levels: Option<Vec<u8>>,
    // ---- 通道 ----
    pub i2c_transfer: String,
    pub i2c_bus: Option<i32>,
    pub i2c_addr: Option<u8>,
    pub hidraw: Option<String>,
    // ---- 运行时（一般由命令行 / systemd 单元给出，不在默认配置文件里） ----
    /// 触摸板事件设备覆盖，如 /dev/input/event7
    pub device_override: Option<String>,
    /// 键盘事件设备覆盖
    pub keyboard_override: Option<String>,
    /// 背光通道：hidraw / i2c / none
    pub led_kind: String,
    /// i2c 载荷首字节（与 LED 命令字构成 w2 写入）
    pub i2c_payload: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            frame_ms: FRAME_MS,
            numlock_trigger_frames: NUMLOCK_TRIGGER_FRAMES,
            cont_sendkey_trigger_frames: NUM_KEY_CONT_TRIGGER_FRAMES,
            numadv_trigger_frames: NUMADV_TRIGGER_FRAMES,
            max_work_time_ms: MAX_WORK_TIME_MS,
            app_gesture_trig_um: APP_GESTURE_TRIG_UM,
            numlock_max_valid_dist_um: NUMLOCK_MAX_VALID_DIST_UM,
            numlock_max_fail_num: NUMLOCK_MAX_FAIL_NUM,
            gesture_trig_um: GESTURE_TRIG_UM,
            tap_threshold_mm: TAP_THRESHOLD_MM,
            scroll_finger_dist_mm: SCROLL_FINGER_DIST_MM,
            touch_sendkey_frames: GESTURE_TOUCH_SENDKEY_FRAMES,
            touch_sendkey_ratio: GESTURE_DELAY_TOUCH_SENDKEY_RATIO,
            tap_interval_min_ms: TAP_INTERVAL_MIN_MS,
            gesture_edge_frames: GESTURE_EDGE_TEMP_FRAMES,
            repeat_interval_ms: 40.0,
            layout_name: "auto".to_string(),
            allow_unverified_layout: false,
            sku: None,
            pressure_numpad_min: 0,
            key_mode: KeyMode::Keypad,
            numlock: NumLockMode::Sync,
            calc_gesture: true,
            calc_gesture_mode: CalcMode::Long,
            mixed_mode: MixedMode::Auto,
            grab: true,
            touchpad_width_mm: 130.0,
            touchpad_height_mm: 80.0,
            language: "auto".to_string(),
            follow_keyboard_brightness: true,
            keyboard_brightness_dir: crate::brightness::DEFAULT_DIR.to_string(),
            brightness_levels: default_brightness_levels(),
            brightness_poll_ms: 500,
            manual_brightness_step: MANUAL_BRIGHTNESS_STEPS,
            manual_brightness_levels: default_manual_brightness_levels(),
            i2c_transfer: "i2ctransfer".to_string(),
            i2c_bus: None,
            i2c_addr: None,
            hidraw: None,
            device_override: None,
            keyboard_override: None,
            led_kind: "auto".to_string(),
            i2c_payload: "0x00".to_string(),
        }
    }
}

/// 默认档位表：0x41..=0x48（8 档）
fn default_brightness_levels() -> Vec<u8> {
    (1..=VAL_BRIGHTNESS_MAX)
        .map(|i| VAL_BRIGHTNESS_BASE + i)
        .collect()
}

/// 宽松解析亮度档位表：兼容整数与 `"0x41"` / `"65"` 字符串。
/// 任何一项解析不了都直接报错（不要静默丢档位），错误信息带下标，方便定位手改错的配置。
fn de_brightness_levels<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
    parse_level_list::<D::Error>(&raw)
}

/// 同上，但允许整项缺失 / 显式 `null`（`manual_brightness_levels` 用）。
fn de_opt_brightness_levels<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<Vec<serde_json::Value>>::deserialize(deserializer)?;
    match raw {
        None => Ok(None),
        Some(list) => parse_level_list::<D::Error>(&list).map(Some),
    }
}

/// 档位数组解析的共用实现（与 serde 解耦，方便两处复用）。
fn parse_level_list<E: de::Error>(raw: &[serde_json::Value]) -> Result<Vec<u8>, E> {
    let mut out = Vec::with_capacity(raw.len());
    for (idx, item) in raw.iter().enumerate() {
        let parsed = match item {
            serde_json::Value::Number(n) => {
                n.as_u64().filter(|v| *v <= u8::MAX as u64).map(|v| v as u8)
            }
            serde_json::Value::String(text) => {
                let t = text.trim();
                match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    Some(hex) => u8::from_str_radix(hex, 16).ok(),
                    None => t.parse::<u8>().ok(),
                }
            }
            _ => None,
        };
        match parsed {
            Some(v) => out.push(v),
            None => {
                return Err(de::Error::custom(crate::i18n::tf(
                    "cfg.err.brightness_levels",
                    &[("idx", &idx.to_string()), ("item", &item.to_string())],
                )))
            }
        }
    }
    Ok(out)
}

impl Config {
    /// 实际生效的数字区亮度档；返回空 = 不跟随键盘背光、退回 0x01 点亮。
    pub fn effective_levels(&self) -> Vec<u8> {
        if self.follow_keyboard_brightness {
            self.brightness_levels.clone()
        } else {
            Vec::new()
        }
    }

    /// 手动调光档数：有自定义表时以表长为准，否则用默认 [`MANUAL_BRIGHTNESS_STEPS`]。
    pub fn manual_steps(&self) -> u8 {
        match &self.manual_brightness_levels {
            Some(v) if !v.is_empty() => v.len().min(MANUAL_BRIGHTNESS_MAX_STEPS) as u8,
            _ => MANUAL_BRIGHTNESS_STEPS,
        }
    }

    /// 手动调光用的档位表（从暗到亮）。
    ///
    /// * 配了 `manual_brightness_levels` → 原样使用（就是用户写的「自定义」）；
    /// * 没配 → 在「熄灭 + 硬件全档」这条轴（`0x00` 与 `0x41..=0x48`，共 9 点）上按
    ///   `manual_steps()` **线性等分**，首档落 `0x00`（灭）、末档落最亮。
    ///   默认 8 档 + 4 档 → `0x00 / 0x43 / 0x45 / 0x48`。
    ///
    /// 与「是否跟随键盘背光」无关：关掉跟随之后 `bright N` 依然可用。
    pub fn manual_levels(&self) -> Vec<u8> {
        if let Some(v) = &self.manual_brightness_levels {
            if !v.is_empty() {
                return v
                    .iter()
                    .take(MANUAL_BRIGHTNESS_MAX_STEPS)
                    .copied()
                    .collect();
            }
        }
        crate::brightness::linear_levels_from_off(
            &self.brightness_levels,
            MANUAL_BRIGHTNESS_STEPS as usize,
        )
    }

    /// 手动档位 → 报文亮度字节。
    ///
    /// - `step = 0` → `0x00`（熄灭）
    /// - `step = 1..=manual_steps()` → 自定义 / 线性表里对应那一档（1 = 灭到最暗端，N = 最亮）
    /// - 越界返回 `None`；档位表为空时退化为 `VAL_ON`
    ///
    /// 默认档表首档就是 `0x00`（用户要求「0 到最大线性四档」），所以 `bright 1` 与
    /// `bright 0` 效果相同——这是硬件只有 8 级造成的，不是 bug。
    /// 想让首档变成「最暗但仍有光」，写 `manual_brightness_levels: [0x41, 0x43, 0x46, 0x48]`。
    pub fn level_for_step(&self, step: u8) -> Option<u8> {
        if step == 0 {
            return Some(VAL_OFF);
        }
        let levels = self.manual_levels();
        if levels.is_empty() {
            return Some(VAL_ON);
        }
        if step as usize > levels.len() {
            return None;
        }
        Some(levels[step as usize - 1])
    }

    /// 不跟随键盘背光时点灯用的常亮值：取手动档，命中 `0x00` 档时退回 `0x01` 点亮。
    ///
    /// 「默认四档首档就是熄灭」之后，若这里原样返回 `0x00`，关掉跟随就会让数字区一直黑着，
    /// 与「常亮」的语义相悖；真正的熄灭仍由 `bright 0` / `off` 负责。
    pub fn static_on_value(&self) -> u8 {
        // 档位 0 是用户显式要求「不点灯」，与「四档里的第 1 档恰好是熄灭」要分开处理
        if self.manual_brightness_step == 0 {
            return VAL_OFF;
        }
        match self.level_for_step(self.manual_brightness_step) {
            Some(VAL_OFF) | None => VAL_ON,
            Some(v) => v,
        }
    }

    /// 把第 `step` 档（**1 起**）改成任意一个硬件支持的亮度值，返回改后的整表。
    ///
    /// 这是「每个档位都能指定任意支持亮度」的落地点：
    /// * `value` 只接受 `0x00`（熄灭）或 `0x41..=0x48`（硬件的 8 个亮档）；
    /// * `step` 必须落在 `1..=manual_steps()`，且不超过 [`MANUAL_BRIGHTNESS_MAX_STEPS`]；
    /// * 未配置档表时（`manual_brightness_levels = null`）先把当前生效的线性表**落成实表**
    ///   再改单档，避免「改一档 → 表只剩一档」把其余档位弄丢；
    /// * 改完仍要求非递减：允许相邻两档相同（硬件就 8 个刻度，重复值只是看不出变化），
    ///   但不允许后一档比前一档暗，否则 `bright up` 会越按越黑。
    ///
    /// 只改内存里的配置，落盘由调用方负责（CLI 走 `persist_config_keys`）。
    pub fn set_manual_level(&mut self, step: usize, value: u8) -> Result<Vec<u8>, LevelSetError> {
        if !manual_level_allowed(value) {
            return Err(LevelSetError::Value);
        }
        let mut levels = self.manual_levels();
        if step == 0 || step > levels.len() || step > MANUAL_BRIGHTNESS_MAX_STEPS {
            return Err(LevelSetError::Step);
        }
        levels[step - 1] = value;
        if !levels.windows(2).all(|w| w[0] <= w[1]) {
            return Err(LevelSetError::Order);
        }
        self.manual_brightness_levels = Some(levels.clone());
        Ok(levels)
    }

    /// 本机布局对应的 NumLock 长按帧数（NUMPAD_GH3XLNDZ_01 = 62）。
    pub fn numlock_frames(&self) -> u32 {
        // 按「实际生效的布局」取长按帧数：auto 会解析成具体布局名，
        // 解析不到时回落 GH3XLNDZ 的 62 帧（≈ 496 ms）。
        match self.layout_entry().name {
            "GH3XLNDZ" => NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ,
            "GH2XLNDZ" | "GH4XLNDZ" => NUMLOCK_TRIGGER_FRAMES_GH2XLNDZ,
            _ => self.numlock_trigger_frames,
        }
    }

    /// 与上游 `load_config` 相同的收敛逻辑：非法值回落官方默认，并说明原因。
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        // 界面语言只接受 auto / zh-CN / zh-TW / en（大小写不敏感），其余回落 auto
        let lang_raw = self.language.trim().to_string();
        if lang_raw.is_empty() {
            self.language = "auto".to_string();
        } else if !lang_raw.eq_ignore_ascii_case("auto") {
            match crate::i18n::Lang::parse(&lang_raw) {
                Some(l) => self.language = l.code().to_string(),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.invalid_language",
                        &[("v", &lang_raw)],
                    ));
                    self.language = "auto".to_string();
                }
            }
        } else {
            self.language = "auto".to_string();
        }
        // 手动指定的机型 id：空串等于不指定；表里没有的 id 直接丢弃并提示，
        // 免得手改配置写错一个字母就让驱动再也起不来。
        if let Some(raw) = self.sku.take() {
            let id = raw.trim().to_string();
            match crate::sku::by_id(&id) {
                Some(entry) => self.sku = Some(entry.id.to_string()),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.sku_unknown",
                        &[("id", &id), ("ids", &crate::sku::known_ids().join(", "))],
                    ));
                }
            }
        }
        self.frame_ms = self.frame_ms.max(1);
        self.numlock_trigger_frames = self.numlock_trigger_frames.max(1);
        self.brightness_poll_ms = self.brightness_poll_ms.max(50);
        if self.keyboard_brightness_dir.is_empty() {
            self.keyboard_brightness_dir = crate::brightness::DEFAULT_DIR.to_string();
        }
        // 亮度档只允许落在 0x41..=0x48，越界值直接剔除（防手写配置把灯发到哨位上）
        let lo = VAL_BRIGHTNESS_BASE + 1;
        let hi = VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX;
        let before = self.brightness_levels.len();
        self.brightness_levels.retain(|v| (lo..=hi).contains(v));
        if self.brightness_levels.len() != before {
            let dropped = (before - self.brightness_levels.len()).to_string();
            notes.push(crate::i18n::tf(
                "note.levels_dropped",
                &[
                    ("n", &dropped),
                    ("lo", &format!("{lo:02x}")),
                    ("hi", &format!("{hi:02x}")),
                ],
            ));
        }
        // 自定义手动档表：非法值剔除、非递增表/空表/超长表按规则收敛，
        // 免得手改配置写成 `[0, 255]` 把灯发到哨位上，或写 20 档却没那么多物理档。
        //
        // 合法取值是 `0`（熄灭）或 `0x41..=0x48`：允许 0 是为了支持「从 0 到最大
        // 线性自定义四档」这种写法（如 `[0, 0x43, 0x46, 0x48]`），硬件侧 0 就是
        // 熄灭，与 `bright 0` 同义。
        if let Some(custom) = self.manual_brightness_levels.take() {
            let mut kept: Vec<u8> = custom
                .iter()
                .copied()
                .filter(|v| manual_level_allowed(*v))
                .collect();
            if kept.len() != custom.len() {
                let dropped = (custom.len() - kept.len()).to_string();
                notes.push(crate::i18n::tf(
                    "note.manual_levels_dropped",
                    &[
                        ("n", &dropped),
                        ("lo", &format!("{lo:02x}")),
                        ("hi", &format!("{hi:02x}")),
                    ],
                ));
            }
            if kept.len() > MANUAL_BRIGHTNESS_MAX_STEPS {
                let n = kept.len().to_string();
                let max = MANUAL_BRIGHTNESS_MAX_STEPS.to_string();
                notes.push(crate::i18n::tf(
                    "note.manual_levels_truncated",
                    &[("n", &n), ("max", &max)],
                ));
                kept.truncate(MANUAL_BRIGHTNESS_MAX_STEPS);
            }
            // 档位必须「从暗到亮」：否则 `bright 2` 可能比 `bright 1` 更暗，与「档位越大
            // 越亮」的直觉相反。允许相邻两档相等——硬件只有 8 个刻度，用户把两档设成
            // 同一个值只是看不出变化，属于合法写法（每档都可任意指定）。写错就整表丢弃，
            // 回落到默认四档（与线性等分数值相同）。
            if kept.is_empty() {
                notes.push(crate::i18n::t("note.manual_levels_reset"));
                self.manual_brightness_levels = default_manual_brightness_levels();
            } else if !kept.windows(2).all(|w| w[0] <= w[1]) {
                notes.push(crate::i18n::t("note.manual_levels_not_increasing"));
                self.manual_brightness_levels = default_manual_brightness_levels();
            } else {
                self.manual_brightness_levels = Some(kept);
            }
        }
        // 手动档上限取决于实际档数（自定义表长度可能不是 4）
        let steps = self.manual_steps();
        if self.manual_brightness_step > steps {
            let step = self.manual_brightness_step.to_string();
            let max = steps.to_string();
            notes.push(crate::i18n::tf(
                "note.manual_step_clamped",
                &[("step", &step), ("max", &max)],
            ));
            self.manual_brightness_step = steps;
        }
        if self.pressure_numpad_min < 0 {
            self.pressure_numpad_min = 0;
        }
        let layout_raw = self.layout_name.trim().to_string();
        if layout_raw.is_empty() {
            // 空值按 auto 处理：跨机型通用靠的是按 HID 自动识别，而不是写死默认布局
            self.layout_name = "auto".to_string();
        } else if !layout_raw.eq_ignore_ascii_case("auto") {
            match crate::layout::by_name(&layout_raw) {
                Some(e) => self.layout_name = e.name.to_string(),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.layout_unknown",
                        &[("v", &layout_raw)],
                    ));
                    self.layout_name = "auto".to_string();
                }
            }
        } else {
            self.layout_name = "auto".to_string();
        }
        notes
    }

    /// 解析一份配置文本，并把已删除的历史字段折到现行字段上。
    ///
    /// 历史字段处理（都在这里做，读路径只有这一条）：
    ///
    /// * `poll_interval_ms` → `frame_ms`：只是同一个量换过名字。旧版把两个都写进
    ///   配置文件，直接当未知字段丢掉会让「用户把帧长调成 20 ms」静默失效；
    /// * `numlock_zone`、`hidraw_prefix`：语义已从实现里移除，读到就忽略，不报错。
    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if let Some(map) = value.as_object_mut() {
            if let Some(legacy) = map.remove("poll_interval_ms") {
                map.entry("frame_ms").or_insert(legacy);
            }
            map.remove("numlock_zone");
            map.remove("hidraw_prefix");
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    /// 读取配置；文件不存在或解析失败时用官方默认值（与上游行为一致）。
    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let mut notes = Vec::new();
        let mut cfg = match std::fs::read_to_string(path) {
            Ok(text) => match Config::from_json(&text) {
                Ok(c) => c,
                Err(err) => {
                    notes.push(crate::i18n::tf(
                        "note.read_failed",
                        &[
                            ("path", &path.display().to_string()),
                            ("err", &err.to_string()),
                        ],
                    ));
                    Config::default()
                }
            },
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    notes.push(crate::i18n::tf(
                        "note.read_failed",
                        &[
                            ("path", &path.display().to_string()),
                            ("err", &err.to_string()),
                        ],
                    ));
                }
                Config::default()
            }
        };
        notes.extend(cfg.sanitize());
        (cfg, notes)
    }

    /// 用户级配置路径（免 root 可写）：`$XDG_CONFIG_HOME/asus-numpad/asus-numpad.json`，
    /// 未设 `XDG_CONFIG_HOME` 时回落 `~/.config/asus-numpad/asus-numpad.json`。
    ///
    /// 服务与 CLI 都以同一个登录用户身份运行，这一份就是唯一事实来源；
    /// 只有拿不到 `HOME` 的异常环境（chroot / initramfs）才退回系统路径。
    pub fn user_config_path() -> PathBuf {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            let dir = PathBuf::from(xdg);
            if !dir.as_os_str().is_empty() {
                return dir.join("asus-numpad").join("asus-numpad.json");
            }
        }
        match std::env::var_os("HOME") {
            Some(home) if !home.as_os_str().is_empty() => PathBuf::from(home)
                .join(".config")
                .join("asus-numpad")
                .join("asus-numpad.json"),
            _ => PathBuf::from(CONFIG_PATH),
        }
    }

    /// 当前进程是否持有写 `/etc` 的特权（root）。
    ///
    /// 这是「不用 sudo」的分支点：普通用户所有写入都落到 [`user_config_path`]，
    /// root（含有人手工 `sudo asus-numpad bright 2`）仍写系统路径，保持管理员习惯。
    pub fn is_privileged() -> bool {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: geteuid 是纯查询，无副作用
            unsafe { libc::geteuid() == 0 }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // 非 Linux 只做交叉编译与单测，保持历史行为（写 /etc）
            true
        }
    }

    /// 落盘目标：普通用户 → 用户级路径；特权进程 → 传入路径（通常是 `/etc`）。
    pub fn writable_path(read_path: &Path) -> PathBuf {
        if Self::is_privileged() {
            read_path.to_path_buf()
        } else {
            Self::user_config_path()
        }
    }

    /// 默认配置路径（读取用）。
    ///
    /// 顺序：用户级 → `/etc` 正式路径 → 历史平铺路径；都不存在时返回用户级路径，
    /// 也就是「首次写入默认落在自己的家目录」，不再因为写不了 `/etc` 而失败。
    pub fn default_path() -> PathBuf {
        let user = Self::user_config_path();
        if user.exists() {
            return user;
        }
        let current = PathBuf::from(CONFIG_PATH);
        if current.exists() {
            return current;
        }
        for legacy in LEGACY_CONFIG_PATHS {
            let path = PathBuf::from(legacy);
            if path.exists() {
                return path;
            }
        }
        user
    }

    /// 迁移源：`/etc` 正式路径或历史路径中第一个存在的。
    fn legacy_config_path() -> Option<&'static Path> {
        std::iter::once(CONFIG_PATH)
            .chain(LEGACY_CONFIG_PATHS.iter().copied())
            .map(Path::new)
            .find(|p| p.exists())
    }

    /// 把 `/etc` 下的既有配置搬到用户级路径（源文件保留不动，避免误删系统默认）。
    ///
    /// 从 1.0.0-15 起配置归用户所有：普通用户改档位 / 语言不再需要 sudo。
    /// 只在「用户级还不存在」时迁移；已存在的用户级配置视为最新意图，不覆盖。
    /// 迁移失败（例如无 `HOME`）静默跳过，读取侧仍有 [`default_path`] 的回落。
    pub fn migrate_legacy_config() -> Option<String> {
        let target = Self::user_config_path();
        if target.exists() {
            return None;
        }
        let source = Self::legacy_config_path()?;
        if source == target.as_path() {
            return None;
        }
        let text = std::fs::read_to_string(source).ok()?;
        // 只搬合法配置，避免把半截文件带进用户目录
        Config::from_json(&text).ok()?;
        std::fs::create_dir_all(target.parent()?).ok()?;
        std::fs::write(&target, text.as_bytes()).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644));
        }
        let to = target.display().to_string();
        Some(crate::i18n::tf(
            "note.config_migrated",
            &[("from", &source.display().to_string()), ("to", &to)],
        ))
    }

    /// 生成示例配置（安装脚本与 `--dump-config` 使用）。
    pub fn to_json(&self) -> String {
        let value = serde_json::json!({
            "frame_ms": self.frame_ms,
            "numlock_trigger_frames": self.numlock_trigger_frames,
            "cont_sendkey_trigger_frames": self.cont_sendkey_trigger_frames,
            "numadv_trigger_frames": self.numadv_trigger_frames,
            "max_work_time_ms": self.max_work_time_ms,
            "app_gesture_trig_um": self.app_gesture_trig_um,
            "numlock_max_valid_dist_um": self.numlock_max_valid_dist_um,
            "numlock_max_fail_num": self.numlock_max_fail_num,
            "gesture_trig_um": self.gesture_trig_um,
            "tap_threshold_mm": self.tap_threshold_mm,
            "scroll_finger_dist_mm": self.scroll_finger_dist_mm,
            "touch_sendkey_frames": self.touch_sendkey_frames,
            "touch_sendkey_ratio": self.touch_sendkey_ratio,
            "tap_interval_min_ms": self.tap_interval_min_ms,
            "gesture_edge_frames": self.gesture_edge_frames,
            "repeat_interval_ms": self.repeat_interval_ms,
            "layout_name": self.layout_name,
            "allow_unverified_layout": self.allow_unverified_layout,
            "pressure_numpad_min": self.pressure_numpad_min,
            "key_mode": self.key_mode.as_str(),
            "numlock": self.numlock.as_str(),
            "calc_gesture": self.calc_gesture,
            "calc_gesture_mode": match self.calc_gesture_mode {
                CalcMode::Long => "long",
                CalcMode::Off => "off",
            },
            "mixed_mode": self.mixed_mode.as_str(),
            "grab": self.grab,
            "touchpad_width_mm": self.touchpad_width_mm,
            "touchpad_height_mm": self.touchpad_height_mm,
            "i2c_transfer": self.i2c_transfer,
            "i2c_bus": self.i2c_bus,
            "i2c_addr": self.i2c_addr,
            "hidraw": self.hidraw,
            "language": self.language,
            "follow_keyboard_brightness": self.follow_keyboard_brightness,
            "keyboard_brightness_dir": self.keyboard_brightness_dir,
            // 整数数组：既保证自己读得回来，也不给手改配置添麻烦
            "brightness_levels": self.brightness_levels,
            "brightness_poll_ms": self.brightness_poll_ms,
            "manual_brightness_step": self.manual_brightness_step,
            "manual_brightness_levels": self.manual_brightness_levels,
            "led_kind": self.led_kind,
        });

        // 只在手动指定机型时写出：默认配置里没有这一项，读一眼就知道是自动识别
        let mut value = value;
        if let Some(id) = &self.sku {
            value["sku"] = serde_json::json!(id);
        }

        let mut text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
        text.push('\n');
        text
    }

    /// 供日志使用的一行摘要。
    pub fn summary(&self) -> String {
        crate::i18n::tf(
            "cfg.summary",
            &[
                ("layout", &self.layout_name),
                ("mode", self.key_mode.as_str()),
                ("frames", &self.numlock_frames().to_string()),
                ("ms", &(self.numlock_frames() * self.frame_ms).to_string()),
                ("idle", &(self.max_work_time_ms / 1000).to_string()),
                ("mixed", self.mixed_mode.as_str()),
                ("numlock", self.numlock.as_str()),
            ],
        )
    }

    /// 布局记录：`auto` 或未知名字都回落到 GH3XLNDZ（键表基准）。
    ///
    /// 实际识别发生在运行时：`linux` 模块会先用触摸板设备名里的 HID 硬件 ID
    /// 查官方映射表，命中后把 `layout_name` 定成具体布局名再调用本函数。
    pub fn layout_entry(&self) -> &'static crate::layout::LayoutEntry {
        crate::layout::by_name(&self.layout_name)
            .or_else(|| crate::layout::by_name(crate::layout::DEFAULT_LAYOUT_NAME))
            .expect("内置布局表必须包含默认布局")
    }

    /// 官方布局参数。
    pub fn layout(&self) -> crate::layout::Layout {
        self.layout_entry().layout
    }
}

/// 官方参数一览（`check` 输出用）；键名与说明随界面语言变化。
pub fn official_parameters() -> Vec<(String, String)> {
    let frames = |n: u32, ms_key: &str| {
        crate::i18n::tf(
            ms_key,
            &[
                ("frames", &n.to_string()),
                ("ms", &(n * FRAME_MS).to_string()),
            ],
        )
    };
    vec![
        (
            crate::i18n::t("param.frame_model"),
            format!("1 frame {FRAME_MS}ms = {FRAMES_PER_SEC} Hz"),
        ),
        (
            "NUMPAD_00 ContSendKey_Trig_FC".to_string(),
            frames(NUM_KEY_CONT_TRIGGER_FRAMES, "param.cont_sendkey"),
        ),
        (
            "NUMPAD_01 NumLock_Trigger_FC".to_string(),
            frames(NUMLOCK_TRIGGER_FRAMES, "param.numlock"),
        ),
        (
            "NUMPAD_GH3XLNDZ_01".to_string(),
            frames(NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ, "param.numlock_gh3"),
        ),
        (
            "NUMPAD_02 Max_Numpad_Work_Time_T".to_string(),
            format!("{MAX_WORK_TIME_MS} ms"),
        ),
        (
            "NUMPAD_06 NumAdv_Trigger_FC".to_string(),
            frames(NUMADV_TRIGGER_FRAMES, "param.cont_sendkey"),
        ),
        (
            "NUMPAD_07 EnAppGesture_Trig_UM".to_string(),
            format!("{APP_GESTURE_TRIG_UM:.0} µm"),
        ),
        (
            "NUMPAD_09 NumLockMaxVaildDist".to_string(),
            format!("{NUMLOCK_MAX_VALID_DIST_UM:.0} µm"),
        ),
        (
            "GESTURE_01 TouchSendKey_Trig_FC".to_string(),
            frames(GESTURE_TOUCH_SENDKEY_FRAMES, "param.cont_sendkey"),
        ),
        (
            "GESTURE_05 Gesture_Trig_UM".to_string(),
            format!("{GESTURE_TRIG_UM:.0} µm"),
        ),
        (
            "GESTURE_06 iTAPPING_THRESHOLD_MM".to_string(),
            format!("{TAP_THRESHOLD_MM:.0} mm"),
        ),
        (
            "GESTURE_08 iscroll_finger_dist_MM".to_string(),
            format!("{SCROLL_FINGER_DIST_MM:.0} mm"),
        ),
        (
            crate::i18n::t("param.led_values_name"),
            crate::i18n::tf(
                "param.led_values",
                &[
                    ("off", &format!("{VAL_OFF:02x}")),
                    ("on", &format!("{VAL_ON:02x}")),
                ],
            ),
        ),
        (
            crate::i18n::t("param.i2c_unlock_name"),
            crate::i18n::tf(
                "param.i2c_unlock",
                &[("unlock", &format!("{VAL_UNLOCK:02x}"))],
            ),
        ),
        (
            crate::i18n::t("param.key_count"),
            format!("{}", kc::ALL_KEYS.len()),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_config_path_uses_xdg_layout() {
        // 不论 HOME/XDG 是否设置，末尾必须是 <dir>/asus-numpad/asus-numpad.json
        let p = Config::user_config_path();
        assert!(
            p.ends_with("asus-numpad/asus-numpad.json"),
            "got {}",
            p.display()
        );
    }

    #[test]
    fn writable_path_follows_privilege() {
        // 契约：root 写传入路径（/etc），普通用户写自己的家目录——这是「免 sudo」的根本
        let sys = Path::new(CONFIG_PATH);
        let got = Config::writable_path(sys);
        if Config::is_privileged() {
            assert_eq!(got, sys);
        } else {
            assert_eq!(got, Config::user_config_path());
            assert!(!got.starts_with("/etc"));
        }
    }

    #[test]
    fn defaults_match_official_inf() {
        let c = Config::default();
        assert_eq!(c.frame_ms, 8);
        assert_eq!(c.tap_threshold_mm, 10.0);
        assert_eq!(c.gesture_trig_um, 4838.0);
        assert_eq!(c.numlock_max_valid_dist_um, 2741.0);
        assert_eq!(c.touch_sendkey_frames, 10);
        assert_eq!(c.touch_sendkey_ratio, 3);
        assert_eq!(c.tap_interval_min_ms, 350.0);
        assert_eq!(c.scroll_finger_dist_mm, 40.0);
        assert_eq!(c.app_gesture_trig_um, 7000.0);
        assert_eq!(c.max_work_time_ms, 120_000);
        assert_eq!(c.cont_sendkey_trigger_frames, 25);
        assert_eq!(c.numadv_trigger_frames, 40);
        assert_eq!(c.pressure_numpad_min, 0);
        assert!(c.calc_gesture);
        assert!(c.grab);
        assert_eq!(c.key_mode, KeyMode::Keypad);
        assert_eq!(c.numlock, NumLockMode::Sync);
        assert_eq!(c.mixed_mode, MixedMode::Auto);
    }

    #[test]
    fn manual_four_steps_span_off_to_brightest() {
        let c = Config::default();
        // 默认：在「熄灭 + 8 档」共 9 点上线性四等分 → 0 / 0x43 / 0x45 / 0x48
        assert_eq!(c.manual_levels(), vec![VAL_OFF, 0x43, 0x45, 0x48]);
        assert_eq!(c.level_for_step(0), Some(VAL_OFF));
        assert_eq!(c.level_for_step(1), Some(VAL_OFF));
        assert_eq!(c.level_for_step(2), Some(0x43));
        assert_eq!(c.level_for_step(3), Some(0x45));
        assert_eq!(c.level_for_step(4), Some(0x48));
        // 越界档位不接受（由 CLI 拦成用法提示）
        assert_eq!(c.level_for_step(5), None);
    }

    #[test]
    fn linear_steps_are_monotonic_and_cover_ends() {
        let c = Config::default();
        let levels = c.manual_levels();
        assert_eq!(levels.len(), MANUAL_BRIGHTNESS_STEPS as usize);
        // 首档 = 熄灭，末档 = 最亮档（「从 0 到最大」的直觉）
        assert_eq!(levels.first().copied(), Some(VAL_OFF));
        assert_eq!(levels.last().copied(), c.brightness_levels.last().copied());
        // 单调不减，中间档必须严格递增
        for w in levels.windows(2) {
            assert!(w[0] <= w[1], "档位必须从暗到亮: {levels:?}");
        }
        assert!(
            levels.windows(2).skip(1).all(|w| w[0] < w[1]),
            "中间档需严格递增: {levels:?}"
        );
    }

    #[test]
    fn linear_from_off_is_stable_across_step_counts() {
        use crate::brightness::linear_levels_from_off;
        let eight: Vec<u8> = (1..=8).map(|i| 0x40 + i).collect();
        // 两端固定：灭 → 最亮
        for n in 2..=8usize {
            let got = linear_levels_from_off(&eight, n);
            assert_eq!(got.len(), n);
            assert_eq!(got[0], VAL_OFF);
            assert_eq!(*got.last().unwrap(), 0x48);
        }
        // 只有一档时没有两端之分，取最亮
        assert_eq!(linear_levels_from_off(&eight, 1), vec![0x48]);
        // 自定义表已含 0 时不重复插入零点
        assert_eq!(
            linear_levels_from_off(&[0x00, 0x43, 0x48], 3),
            vec![0x00, 0x43, 0x48]
        );
        assert!(linear_levels_from_off(&[], 4).is_empty());
    }

    #[test]
    fn custom_levels_may_start_from_off() {
        // 「从 0 到最大线性四档」：0 是熄灭，其余三档线性铺到最亮
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x00, 0x43, 0x46, 0x48]),
            ..Config::default()
        };
        let notes = c.sanitize();
        assert!(
            c.manual_brightness_levels.is_some(),
            "含 0 的自定义表应被保留，却回落到线性等分: {notes:?}"
        );
        assert_eq!(c.manual_steps(), 4);
        assert_eq!(c.level_for_step(1), Some(VAL_OFF));
        assert_eq!(c.level_for_step(2), Some(0x43));
        assert_eq!(c.level_for_step(3), Some(0x46));
        assert_eq!(c.level_for_step(4), Some(0x48));
    }

    #[test]
    fn custom_levels_must_be_increasing() {
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x48, 0x46, 0x43, 0x41]),
            ..Config::default()
        };
        let notes = c.sanitize();
        assert!(!notes.is_empty(), "倒序表应给出告警");
        // 丢弃非法表后回落到默认四档（数值与线性等分一致，首档为熄灭）
        assert_eq!(
            c.manual_brightness_levels,
            Some(DEFAULT_MANUAL_LEVELS.to_vec())
        );
        assert_eq!(c.manual_steps(), MANUAL_BRIGHTNESS_STEPS);
        assert_eq!(c.level_for_step(1), Some(VAL_OFF));
        assert_eq!(c.level_for_step(4), Some(0x48));
    }

    #[test]
    fn default_manual_levels_are_linear_four() {
        // 默认档表显式写在配置里，且与「线性等分现算」逐字节一致（升级不改观感）
        let c = Config::default();
        assert_eq!(
            c.manual_brightness_levels,
            Some(vec![0x00, 0x43, 0x45, 0x48])
        );
        assert_eq!(
            c.manual_levels(),
            crate::brightness::linear_levels_from_off(&c.brightness_levels, 4)
        );
        // 序列化时该项一定要写出来，用户才看得到每档的字节值
        let json: serde_json::Value = serde_json::from_str(&c.to_json()).unwrap();
        assert_eq!(
            json["manual_brightness_levels"],
            serde_json::json!([0, 67, 69, 72])
        );
    }

    #[test]
    fn manual_level_accepts_any_supported_value() {
        let mut c = Config::default();
        // 每档都能单独指定：把第 2 档压到 0x41（最暗亮档），第 3 档保持 0x45
        assert_eq!(
            c.set_manual_level(2, 0x41),
            Ok(vec![0x00, 0x41, 0x45, 0x48])
        );
        assert_eq!(c.level_for_step(2), Some(0x41));
        // 允许把中间档设成 0x48（等于最亮），末档同步抬到 0x48 也合法（相邻相等）
        assert_eq!(
            c.set_manual_level(3, 0x48),
            Ok(vec![0x00, 0x41, 0x48, 0x48])
        );
        // 灭档也可以挪走：第 1 档改成 0x41 → 四档全是亮的
        assert_eq!(
            c.set_manual_level(1, 0x41),
            Ok(vec![0x41, 0x41, 0x48, 0x48])
        );
        // 非法值 / 越界档 / 破坏顺序都要拒绝，且不改动已生效的表
        let before = c.manual_brightness_levels.clone();
        assert_eq!(c.set_manual_level(2, 0x49), Err(LevelSetError::Value));
        assert_eq!(c.set_manual_level(2, 0x30), Err(LevelSetError::Value));
        assert_eq!(c.set_manual_level(0, 0x41), Err(LevelSetError::Step));
        assert_eq!(c.set_manual_level(5, 0x48), Err(LevelSetError::Step));
        assert_eq!(c.set_manual_level(2, 0x00), Err(LevelSetError::Order));
        assert_eq!(c.manual_brightness_levels, before);
    }

    #[test]
    fn manual_level_materializes_linear_table_first() {
        // 表未显式配置（null）时改单档，应先把线性表落成实表，不能只剩一档
        let mut c = Config {
            manual_brightness_levels: None,
            ..Config::default()
        };
        assert_eq!(
            c.set_manual_level(2, 0x42),
            Ok(vec![0x00, 0x42, 0x45, 0x48])
        );
        assert_eq!(c.manual_steps(), 4);
    }

    #[test]
    fn manual_level_supports_non_power_of_two_step_counts() {
        // 档数由表长决定：五档表每档都能任意指定
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x41, 0x42, 0x43, 0x44, 0x48]),
            ..Config::default()
        };
        assert_eq!(
            c.set_manual_level(5, 0x47),
            Ok(vec![0x41, 0x42, 0x43, 0x44, 0x47])
        );
        assert_eq!(c.manual_steps(), 5);
    }

    #[test]
    fn custom_levels_still_drop_out_of_range() {
        let mut c = Config {
            manual_brightness_levels: Some(vec![0x00, 0x30, 0x43, 0x48]),
            ..Config::default()
        };
        let notes = c.sanitize();
        assert!(!notes.is_empty(), "越界档位应被剔除并告警");
        assert_eq!(c.manual_levels(), vec![0x00, 0x43, 0x48]);
    }

    #[test]
    fn linear_levels_helper_handles_edge_cases() {
        use crate::brightness::linear_levels;
        // 空表 / 0 档 → 空
        assert!(linear_levels(&[], 4).is_empty());
        assert!(linear_levels(&[0x41, 0x48], 0).is_empty());
        // 单档恒定把最亮当唯一档（不会因为「只有一档」变暗）
        assert_eq!(linear_levels(&[0x41, 0x48], 1), vec![0x48]);
        // 档数与表长相同 → 原样返回
        assert_eq!(
            linear_levels(&[0x41, 0x44, 0x48], 3),
            vec![0x41, 0x44, 0x48]
        );
        // 8 档取 2 点 → 两端
        assert_eq!(
            linear_levels(&default_brightness_levels(), 2),
            vec![0x41, 0x48]
        );
    }

    #[test]
    fn manual_step_ignored_when_following() {
        let mut c = Config {
            manual_brightness_step: 1,
            ..Config::default()
        };
        assert!(c.follow_keyboard_brightness);
        // 跟随开：effective_levels 用整表，固定档不参与点灯
        assert_eq!(c.effective_levels().len(), 8);
        c.follow_keyboard_brightness = false;
        assert!(c.effective_levels().is_empty());
        // 跟随关：按手动档常亮（1 档 = 熄灭档，常亮退化为 0x01；3 档 = 0x45）
        assert_eq!(c.static_on_value(), 0x01);
        c.manual_brightness_step = 3;
        assert_eq!(c.static_on_value(), 0x45);
        c.manual_brightness_step = 4;
        assert_eq!(c.static_on_value(), 0x48);
        c.manual_brightness_step = 0;
        assert_eq!(c.static_on_value(), VAL_OFF);
    }

    #[test]
    fn manual_step_clamped_on_load() {
        let mut c: Config = serde_json::from_str(r#"{"manual_brightness_step":9}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_brightness_step, MANUAL_BRIGHTNESS_STEPS);
        assert!(notes.iter().any(|n| n.contains("manual_brightness_step")));
    }

    #[test]
    fn manual_levels_use_custom_table() {
        let mut c = Config {
            // 自定义档表：档数由长度决定（这里 3 档），亮度逐档照写，不做等分
            manual_brightness_levels: Some(vec![0x41, 0x44, 0x48]),
            ..Config::default()
        };
        c.sanitize();
        assert_eq!(c.manual_steps(), 3);
        assert_eq!(c.level_for_step(0), Some(VAL_OFF));
        assert_eq!(c.level_for_step(1), Some(0x41));
        assert_eq!(c.level_for_step(2), Some(0x44));
        assert_eq!(c.level_for_step(3), Some(0x48));
        assert_eq!(c.level_for_step(4), None);
        // 空档表：退化为 0x01 点亮
        c.manual_brightness_levels = None;
        c.brightness_levels.clear();
        assert_eq!(c.level_for_step(2), Some(VAL_ON));
    }

    #[test]
    fn custom_levels_are_sanitized() {
        // 越界值剔除 → 表变短，档数跟着变（0 是合法值，代表熄灭）
        let mut c: Config =
            serde_json::from_str(r#"{"manual_brightness_levels":["0x41","0x30","0x46","0x48"]}"#)
                .unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_brightness_levels, Some(vec![0x41, 0x46, 0x48]));
        assert_eq!(c.manual_steps(), 3);
        // 提示文案随语言变化，这里只要求「有提示」且指向该配置项
        assert!(
            notes.iter().any(|n| n.contains("manual_brightness_levels")),
            "notes={notes:?}"
        );
        // 全越界 → 放弃自定义，回落默认四档
        let mut c: Config =
            serde_json::from_str(r#"{"manual_brightness_levels":["0x30","0xff"]}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(
            c.manual_brightness_levels,
            Some(DEFAULT_MANUAL_LEVELS.to_vec())
        );
        assert_eq!(c.manual_levels().len(), MANUAL_BRIGHTNESS_STEPS as usize);
        assert_eq!(notes.len(), 2, "越界剔除 + 回落各一条提示: {notes:?}");
        // 超长表：硬件只有 8 个亮档，超过 8 档就截断；重复值属于合法写法（相邻相等允许），
        // 所以 10 档同值截断后仍是 8 档，不再被判成非法而整表丢弃
        let mut c: Config =
            serde_json::from_str(r#"{"manual_brightness_levels":[65,65,65,65,65,65,65,65,65,65]}"#)
                .unwrap();
        let notes = c.sanitize();
        assert_eq!(
            c.manual_brightness_levels,
            Some(vec![65; MANUAL_BRIGHTNESS_MAX_STEPS])
        );
        assert_eq!(c.manual_steps() as usize, MANUAL_BRIGHTNESS_MAX_STEPS);
        assert!(
            notes.iter().any(|n| n.contains("manual_brightness_levels")),
            "notes={notes:?}"
        );
    }

    #[test]
    fn custom_table_len_drives_step_clamp() {
        let mut c: Config = serde_json::from_str(
            r#"{"manual_brightness_levels":[65,68],"manual_brightness_step":4}"#,
        )
        .unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_steps(), 2);
        assert_eq!(c.manual_brightness_step, 2);
        assert!(notes.iter().any(|n| n.contains("manual_brightness_step")));
    }

    #[test]
    fn gh3xlndz_numlock_frames_is_62() {
        let c = Config::default();
        assert_eq!(c.numlock_frames(), 62);
        assert_eq!(c.numlock_frames() * c.frame_ms, 496);
    }

    #[test]
    fn partial_json_keeps_official_defaults() {
        let c: Config = serde_json::from_str(r#"{"key_mode":"digits","grab":false}"#).unwrap();
        assert_eq!(c.key_mode, KeyMode::Digits);
        assert!(!c.grab);
        // 未给出的字段必须是官方默认
        assert_eq!(c.gesture_trig_um, 4838.0);
        assert_eq!(c.frame_ms, 8);
    }

    #[test]
    fn mixed_mode_accepts_bool_and_string() {
        let a: MixedMode = serde_json::from_str("true").unwrap();
        let b: MixedMode = serde_json::from_str("\"auto\"").unwrap();
        let c: MixedMode = serde_json::from_str("false").unwrap();
        assert_eq!(a, MixedMode::On);
        assert_eq!(b, MixedMode::Auto);
        assert_eq!(c, MixedMode::Off);
    }

    #[test]
    fn invalid_key_mode_falls_back_to_keypad() {
        let c: Config = serde_json::from_str(r#"{"key_mode":"numpad"}"#).unwrap();
        assert_eq!(c.key_mode, KeyMode::Keypad);
    }

    #[test]
    fn legacy_fields_are_ignored_not_fatal() {
        // `numlock_zone` / `hidraw_prefix` 已从实现里移除，读到就当没写：
        // 不能因为一份老配置让整份设置退回默认值。
        let mut c =
            Config::from_json(r#"{"numlock_zone":"icon","hidraw_prefix":[0,3,1],"frame_ms":8}"#)
                .unwrap();
        let notes = c.sanitize();
        assert_eq!(c.frame_ms, 8);
        assert!(notes.is_empty(), "unexpected notes: {notes:?}");
    }

    #[test]
    fn poll_interval_ms_folds_into_frame_ms() {
        // 旧版两个字段都写进配置文件；读到就折成一个，别让用户的帧长设置静默失效。
        let c = Config::from_json(r#"{"poll_interval_ms":12}"#).unwrap();
        assert_eq!(c.frame_ms, 12);
        // 两份都在时以 frame_ms 为准
        let c = Config::from_json(r#"{"frame_ms":6,"poll_interval_ms":12}"#).unwrap();
        assert_eq!(c.frame_ms, 6);
        // 写回只剩一个名字
        assert!(!c.to_json().contains("poll_interval_ms"));
    }

    #[test]
    fn brightness_levels_accept_int_and_hex_string() {
        let ints: Config = serde_json::from_str(r#"{"brightness_levels":[65,72]}"#).unwrap();
        assert_eq!(ints.brightness_levels, vec![0x41, 0x48]);
        // 旧版本写出的 "0x41" 字符串必须还能读（否则整份配置会悄悄退回默认）
        let hex: Config =
            serde_json::from_str(r#"{"brightness_levels":["0x41","0x45","0x48"]}"#).unwrap();
        assert_eq!(hex.brightness_levels, vec![0x41, 0x45, 0x48]);
        let dec: Config = serde_json::from_str(r#"{"brightness_levels":["65"]}"#).unwrap();
        assert_eq!(dec.brightness_levels, vec![0x41]);
        let missing: Config = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(missing.brightness_levels.len(), 8);
    }

    #[test]
    fn dump_config_roundtrips_through_load() {
        // to_json 写出的东西必须能被 load 原样读回（v5 曾因 "0x41" 字符串读不回来，
        // 导致 bright 命令写入后 follow/手动档全部丢失，退回默认值）
        let dir = std::env::temp_dir();
        let path = dir.join(format!("asus-numpad-roundtrip-{}.json", std::process::id()));
        let c = Config {
            follow_keyboard_brightness: false,
            manual_brightness_step: 2,
            ..Config::default()
        };
        std::fs::write(&path, c.to_json()).unwrap();

        let (back, notes) = Config::load(&path);
        let _ = std::fs::remove_file(&path);
        assert!(notes.is_empty(), "notes={notes:?}");
        assert!(!back.follow_keyboard_brightness);
        assert_eq!(back.manual_brightness_step, 2);
        assert_eq!(back.brightness_levels, c.brightness_levels);
        assert_eq!(back.level_for_step(1), Some(VAL_OFF));
        assert_eq!(back.level_for_step(2), Some(0x43));
        assert_eq!(back.level_for_step(4), Some(0x48));
    }

    #[test]
    fn bad_brightness_levels_reports_index() {
        let err =
            serde_json::from_str::<Config>(r#"{"brightness_levels":[65,"oops"]}"#).unwrap_err();
        assert!(err.to_string().contains("brightness_levels[1]"), "{err}");
    }

    #[test]
    fn zero_frame_ms_is_clamped() {
        let mut c: Config =
            serde_json::from_str(r#"{"frame_ms":0,"numlock_trigger_frames":0}"#).unwrap();
        c.sanitize();
        assert_eq!(c.frame_ms, 1);
        assert_eq!(c.numlock_trigger_frames, 1);
    }
}
