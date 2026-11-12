//! 配置：默认值即官方 AlgoParameters 原值，改动即偏离官方。

use crate::layout::KeyMode;
use serde::de::{self, Deserializer, Visitor};
use serde::Deserialize;
use std::fmt;

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

/// 手动调光的默认档数（`numdeck bright 1|2|3|4`，另有 0 = 熄灭）。
///
/// 四档在「熄灭 + 硬件全档」这条轴上**线性等分**（`0x00` 与 `0x41..=0x48`，共 9 点），
/// 首档就是熄灭、末档是最亮：默认 8 档表 → `0x00 / 0x43 / 0x45 / 0x48`。
///
/// 这只是默认值：`manual_brightness_levels` 给了自定义表时，档数以该表长度为准，
/// 亮度直接取表内写死的值（见 [`Config::manual_steps`]）。
pub const MANUAL_BRIGHTNESS_STEPS: u8 = 4;

/// 自定义手动档位表的长度上限（受硬件 0x41..=0x48 八个物理档限制）。
pub const MANUAL_BRIGHTNESS_MAX_STEPS: usize = 8;

/// 默认手动档位表：四档「从熄灭线性拉到最亮」= `0x00 / 0x43 / 0x45 / 0x48`。
///
/// 与 1.2.2 之前「不配置时现算线性等分」的结果逐字节相同，区别只是**写进默认配置**：
/// 用户打开 JSON 就能看到每一档的字节值，按档改成任意硬件支持的亮度
/// （`0x00` 或 `0x41..=0x48`）即可，不必再自己推算线性关系。
pub const DEFAULT_MANUAL_LEVELS: [u8; 4] = [VAL_OFF, 0x43, 0x45, 0x48];

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
pub const CONFIG_PATH: &str = "/etc/numdeck/numdeck.json";
/// 历史版本的配置路径（都平铺在 `/etc` 下），按新→旧顺序探测。
///
/// * `/etc/numdeck.json` —— 改名后中间版本的路径；
/// * `/etc/asus-numpad/asus-numpad.json` —— 更名前（1.0.x）的正式路径，
///   升级上来的机器大量存在这一份，必须照旧读得到；
/// * `/etc/asus-numpad.json` —— 更名前的中间版本路径；
/// * `/etc/rog-numpad.json`  —— 更早 rog-numpad 时代的路径。
///
/// 仅用于一次性迁移与只读兼容，新写入一律走 [`CONFIG_PATH`]。
pub const LEGACY_CONFIG_PATHS: &[&str] = &[
    "/etc/numdeck.json",
    "/etc/asus-numpad/asus-numpad.json",
    "/etc/asus-numpad.json",
    "/etc/rog-numpad.json",
];
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
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    /// 也可写死某套布局（`GH3XLNDZ` / `GH2XLNDZ` / `GH4XLNDZ` 等，见 `numdeck layout`）。
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
    /// `numdeck sku auto` 可清空本字段、回到自动识别。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
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
    /// 优先级：环境变量 `NUMDECK_LANG` > 命令行 `--lang` > 本字段 > 系统 locale。
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
    /// 仅在「不跟随键盘背光」时作为常亮档使用；`numdeck bright N` 会写入这里。
    pub manual_brightness_step: u8,
    /// 手动档位表（从暗到亮）：**默认显式给出四档** `[0, 0x43, 0x45, 0x48]`
    /// （0x00 灭 → 0x48 最亮，线性等分）。
    ///
    /// 每一项都可以换成任意一个硬件支持的亮度：`0x00` 或 `0x41..=0x48`（共 8 个亮档）；
    /// 档数由长度决定（1..=8），所以 `bright 1|2|…|N` 的 N 跟着表长走。
    /// 改单档可以直接用 `numdeck levels <N> <VALUE>`，也可以手改这里，
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
    // ---- 自定义扩展（见 `custom` 模块） ----
    /// 自定义布局：几何参数 + 键表 + 可选绑定的触摸板 HID 模组 ID。
    ///
    /// 用来覆盖官方表里还没有的机型，以及在**没有镭雕的普通触摸板**上贴一张
    /// 小键盘贴纸——贴纸怎么排，键表就怎么写，`columns` 跟着键表列数走。
    pub custom_layouts: Vec<crate::custom::CustomLayoutSpec>,
    /// 自定义点亮报文（默认就是 NumberPad 的 `0d 14 03 ff ad`）。
    pub led_proto: crate::custom::LedProtoSpec,
    /// 压感触摸板「用力按 = 切换」。
    pub pressure_switch: crate::custom::PressureSwitch,
    /// 物理键盘上的全局组合键。
    pub hotkeys: Vec<crate::custom::HotkeySpec>,
    /// 非原生机型的网格划分（列行数、非均匀列宽、网格矩形、NumLock 格）。
    pub partition: crate::partition::PartitionSpec,
    /// 触摸板轴的绝对范围与单位换算（内核上报不准时手动校正）。
    pub calibration: crate::partition::CalibrationSpec,
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
            custom_layouts: Vec::new(),
            led_proto: crate::custom::LedProtoSpec::default(),
            pressure_switch: crate::custom::PressureSwitch::default(),
            hotkeys: Vec::new(),
            partition: crate::partition::PartitionSpec::default(),
            calibration: crate::partition::CalibrationSpec::default(),
        }
    }
}

mod levels;
pub use levels::*;
mod geometry;
mod io;
mod params;
mod sanitize;
pub use params::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn user_config_path_uses_xdg_layout() {
        // 不论 HOME/XDG 是否设置，末尾必须是 <dir>/numdeck/numdeck.json
        let p = Config::user_config_path();
        assert!(
            p.ends_with("numdeck/numdeck.json"),
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
        let _registry_guard = crate::test_lock::registry();
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
        let _registry_guard = crate::test_lock::registry();
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
        let _registry_guard = crate::test_lock::registry();
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
        let _registry_guard = crate::test_lock::registry();
        let mut c: Config = serde_json::from_str(r#"{"manual_brightness_step":9}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(c.manual_brightness_step, MANUAL_BRIGHTNESS_STEPS);
        assert!(notes.iter().any(|n| n.contains("manual_brightness_step")));
    }

    #[test]
    fn manual_levels_use_custom_table() {
        let _registry_guard = crate::test_lock::registry();
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
        let _registry_guard = crate::test_lock::registry();
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
        let _registry_guard = crate::test_lock::registry();
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
        let _registry_guard = crate::test_lock::registry();
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
        let path = dir.join(format!("numdeck-roundtrip-{}.json", std::process::id()));
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
    fn extension_config_roundtrips_through_disk() {
        // 四项扩展必须能写出去再读回来：GUI 里改完保存、后台服务重启，
        // 少一个字段就等于界面上的设置被静默丢弃。
        let _registry_guard = crate::test_lock::registry();
        let dir = std::env::temp_dir();
        let path = dir.join(format!("numdeck-ext-{}.json", std::process::id()));
        let c = Config {
            custom_layouts: vec![crate::custom::CustomLayoutSpec {
                name: "STICKER-4X4".to_string(),
                base: "GH3XLNDZ".to_string(),
                hid_ids: vec!["ASUE1416".to_string()],
                keys: vec![
                    "kp7 kp8 kp9 slash".to_string(),
                    "kp4 kp5 kp6 asterisk".to_string(),
                    "kp1 kp2 kp3 minus".to_string(),
                    "kp0 period plus enter".to_string(),
                ],
                patch: crate::custom::LayoutPatch {
                    columns: Some(4),
                    left: Some(20),
                    ..Default::default()
                },
            }],
            led_proto: crate::custom::LedProtoSpec {
                template: vec![0x0a, 0x01, crate::custom::LEVEL_MARK, 0x00],
            },
            pressure_switch: crate::custom::PressureSwitch {
                enabled: true,
                percent: 65,
                frames: 4,
                cooldown_ms: 250.0,
            },
            hotkeys: vec![crate::custom::HotkeySpec {
                gesture: "ctrl+alt+n".to_string(),
                action: "on".to_string(),
            }],
            ..Config::default()
        };
        std::fs::write(&path, c.to_json()).unwrap();
        let (back, notes) = Config::load(&path);
        let _ = std::fs::remove_file(&path);
        // 读回时允许「已生效」类提示（抢占内置 HID、压感开启），但不许有任何回落
        assert!(
            notes
                .iter()
                .all(|n| !n.contains("ignored") && !n.contains("falling back")),
            "notes={notes:?}"
        );

        assert_eq!(back.custom_layouts, c.custom_layouts);
        assert_eq!(back.led_proto, c.led_proto);
        assert_eq!(back.pressure_switch, c.pressure_switch);
        assert_eq!(back.hotkeys, c.hotkeys);
        // 布局真的装上了，而且键表按自定义的四列走
        assert!(crate::custom::entry_by_name("STICKER-4X4").is_some());
        assert_eq!(crate::custom::table_for("STICKER-4X4").cols(), 4);
        crate::custom::install_layouts(&[]);
    }

    #[test]
    fn extension_defaults_are_inert() {
        // 默认配置不能有任何副作用：不开压感、报文就是内置报文、没有热键
        let c = Config::default();
        assert!(c.custom_layouts.is_empty());
        assert!(c.hotkeys.is_empty());
        assert!(!c.pressure_switch.enabled);
        assert!(c.led_proto.is_builtin());
        assert!(c.led_proto.validate().is_ok());
        // 空的扩展段不该写进配置文件，免得用户看不出哪些是默认
        let json = c.to_json();
        assert!(!json.contains("STICKER"));
        assert!(serde_json::from_str::<Config>(&json).is_ok());
    }

    /// 坏配置的提示必须**每条只说一次**：`check` 曾经把同一句印两遍
    /// （load 一遍 + CLI 覆盖后复核一遍），用户会以为配了两份坏配置。
    #[test]
    fn duplicate_notes_collapse_to_one_each() {
        let _guard = crate::test_lock::registry();
        let text = r#"{
            "custom_layouts": [
                { "name": "BROKEN", "keys": ["not-a-key q w e", "a s d f", "z x c v", "1 2 3 4"] }
            ],
            "led_proto": { "template": [1, 2, 3] },
            "hotkeys": [{ "gesture": "hyper+q", "action": "on" }]
        }"#;
        let mut cfg = Config::from_json(text).expect("bad-but-parsable");
        let first = cfg.sanitize();
        assert!(!first.is_empty(), "坏配置应当产提示");
        let mut merged = first.clone();
        // 第二次只报还没被修掉的项（LED 报文已被回落成内置，不再重复报），
        // 所以两次的并集一定包含重复——这正是 check 印两遍的来源。
        merged.extend(cfg.sanitize());
        let before = merged.len();
        Config::dedup_notes(&mut merged);
        assert!(merged.len() < before, "去重应当真的删掉了重复项");
        assert_eq!(merged, first, "去重结果应当与单份一致且顺序不变");
    }

    /// LED 报文的提示里必须带真正的错因，不能漏出未替换的 `{err}` 占位符。
    #[test]
    fn led_proto_note_has_no_unreplaced_placeholder() {
        let _guard = crate::test_lock::registry();
        let mut cfg = Config::from_json(r#"{ "led_proto": { "template": [1, 2, 3] } }"#).unwrap();
        let notes = cfg.sanitize();
        let note = notes
            .iter()
            .find(|n| n.contains("255"))
            .expect("应当有一条说明模板少了 255 占位");
        assert!(!note.contains("{err}"), "占位符没替换: {note}");
        assert!(!note.contains('{'), "还有没替换的占位符: {note}");
    }

    #[test]
    fn bad_extension_values_fall_back_with_notes() {
        let _registry_guard = crate::test_lock::registry();
        let mut c: Config = serde_json::from_str(
            r#"{
                "led_proto": { "template": [1, 2, 3] },
                "pressure_switch": { "enabled": true, "percent": 0, "frames": 0 },
                "hotkeys": [ { "gesture": "ctrl+alt+n", "action": "toggle" } ],
                "custom_layouts": [ { "name": "GH3XLNDZ", "keys": [] } ]
            }"#,
        )
        .expect("坏报文不该让整份配置读不出来");
        let notes = c.sanitize();
        // 报文没有占位符 → 回落内置并说明；布局撞了内置名 → 跳过并说明
        assert!(c.led_proto.is_builtin());
        assert!(notes.iter().any(|n| n.contains("placeholder")), "{notes:?}");
        assert!(notes.iter().any(|n| n.contains("GH3XLNDZ")), "{notes:?}");
        // 压感阈值被夹到合法区间，而不是让状态机拿到 0
        assert_eq!(c.pressure_switch.percent, 1);
        assert_eq!(c.pressure_switch.frames, 1);
        crate::custom::install_layouts(&[]);
    }

    /// 分区里与面板无关的坏值必须当场点名并丢掉：这些校验原本只在 `build_geometry`
    /// 里做，而 `check` 不打印那一步的 notes，用户改坏配置后跑 check 什么都看不到。
    #[test]
    fn partition_sanitize_names_bad_values() {
        let _registry_guard = crate::test_lock::registry();

        // 列数越界
        let mut c: Config = serde_json::from_str(r#"{"partition":{"cols":9}}"#).unwrap();
        let notes = c.sanitize();
        assert!(
            notes.iter().any(|n| n.contains("9") && n.contains("1..=8")),
            "{notes:?}"
        );
        assert_eq!(c.partition.cols, None);
        assert!(c.partition.is_unset());

        // 反矩形：整个 rect 丢掉，不再出现「已启用」的假象
        let mut c: Config =
            serde_json::from_str(r#"{"partition":{"rect":[3000,2000,400,500]}}"#).unwrap();
        let notes = c.sanitize();
        assert!(
            notes
                .iter()
                .any(|n| n.contains("rect") && n.contains("3000")),
            "{notes:?}"
        );
        assert_eq!(c.partition.rect, None);
        assert!(
            !notes.iter().any(|n| n.contains("custom partition active")),
            "{notes:?}"
        );

        // 权重长度不符
        let mut c: Config =
            serde_json::from_str(r#"{"partition":{"cols":5,"col_ratios":[1,1]}}"#).unwrap();
        let notes = c.sanitize();
        assert!(notes.iter().any(|n| n.contains("col_ratios")), "{notes:?}");
        assert!(c.partition.col_ratios.is_empty());
        assert_eq!(c.partition.cols, Some(5), "列数本身是好的，不该一起丢掉");

        // 权重里有 0 / 负数
        let mut c: Config =
            serde_json::from_str(r#"{"partition":{"cols":5,"rows":4,"col_ratios":[1,-1,1,1,1]}}"#)
                .unwrap();
        let notes = c.sanitize();
        assert!(
            notes
                .iter()
                .any(|n| n.contains("col_ratios") && n.contains("non-positive")),
            "{notes:?}"
        );
        assert!(c.partition.col_ratios.is_empty());

        // NumLock 格越界 → 回默认格，行列数保留
        let mut c: Config = serde_json::from_str(
            r#"{"partition":{"cols":5,"rows":4,"numlock_cell":[9,9],"layout":"auto"}}"#,
        )
        .unwrap();
        let notes = c.sanitize();
        assert!(notes.iter().any(|n| n.contains("NumLock")), "{notes:?}");
        assert_eq!(c.partition.numlock_cell, None);
        assert_eq!(c.partition.cols, Some(5));
        assert!(
            notes.iter().any(|n| n.contains("layout: auto")),
            "{notes:?}"
        );

        // layout 留空归一成 auto，提示里不再出现空括号
        let mut c: Config =
            serde_json::from_str(r#"{"partition":{"cols":5,"layout":"  "}}"#).unwrap();
        let notes = c.sanitize();
        assert_eq!(c.partition.layout, "auto");
        assert!(
            notes.iter().any(|n| n.contains("layout: auto")),
            "{notes:?}"
        );

        // 好配置一条坏提示都不该有
        let mut c: Config = serde_json::from_str(
            r#"{"partition":{"layout":"auto","cols":5,"rows":4,"col_ratios":[1,1,1,1,1]}}"#,
        )
        .unwrap();
        let notes = c.sanitize();
        assert_eq!(notes.len(), 1, "只该有「已启用」一条: {notes:?}");
    }

    #[test]
    fn bad_brightness_levels_reports_index() {
        let err =
            serde_json::from_str::<Config>(r#"{"brightness_levels":[65,"oops"]}"#).unwrap_err();
        assert!(err.to_string().contains("brightness_levels[1]"), "{err}");
    }

    #[test]
    fn zero_frame_ms_is_clamped() {
        let _registry_guard = crate::test_lock::registry();
        let mut c: Config =
            serde_json::from_str(r#"{"frame_ms":0,"numlock_trigger_frames":0}"#).unwrap();
        c.sanitize();
        assert_eq!(c.frame_ms, 1);
        assert_eq!(c.numlock_trigger_frames, 1);
    }
}
