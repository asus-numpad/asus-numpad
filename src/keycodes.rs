//! Linux input 事件码常量（仅取值，不依赖 evdev crate）。
//!
//! 取值来自 Linux 内核 `include/uapi/linux/input-event-codes.h`，属稳定 ABI。
//! 之所以在本工程内自带一份，是为了让 `layout` / `engine` 这两个纯逻辑模块
//! 可以在非 Linux 平台编译与单元测试；Linux 侧的 `linux` 模块会再用 evdev
//! 的类型做一次交叉校验（见 `linux::dev::assert_codes_consistent`）。

// ---- 事件类型 ----
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_LED: u16 = 0x11;

// ---- 绝对轴 ----
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_PRESSURE: u16 = 0x18;
pub const ABS_MT_SLOT: u16 = 0x2f;
pub const ABS_MT_TOUCH_MAJOR: u16 = 0x30;
pub const ABS_MT_WIDTH_MAJOR: u16 = 0x32;
pub const ABS_MT_POSITION_X: u16 = 0x35;
pub const ABS_MT_POSITION_Y: u16 = 0x36;
pub const ABS_MT_TRACKING_ID: u16 = 0x39;
pub const ABS_MT_PRESSURE: u16 = 0x3a;

// ---- 按键/按钮 ----
pub const BTN_TOUCH: u16 = 0x14a;
pub const BTN_TOOL_FINGER: u16 = 0x145;
pub const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
pub const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
pub const BTN_TOOL_QUADTAP: u16 = 0x14f;

// ---- 指示灯 ----
pub const LED_NUML: u16 = 0x00;

// ---- 数字小键盘键码 ----
pub const KEY_0: u16 = 2;
pub const KEY_1: u16 = 3;
pub const KEY_2: u16 = 4;
pub const KEY_3: u16 = 5;
pub const KEY_4: u16 = 6;
pub const KEY_5: u16 = 7;
pub const KEY_6: u16 = 8;
pub const KEY_7: u16 = 9;
pub const KEY_8: u16 = 10;
pub const KEY_9: u16 = 11;
pub const KEY_DOT: u16 = 52;
pub const KEY_BACKSPACE: u16 = 14;
pub const KEY_KPASTERISK: u16 = 55;
pub const KEY_KP7: u16 = 71;
pub const KEY_KP8: u16 = 72;
pub const KEY_KP9: u16 = 73;
pub const KEY_KPMINUS: u16 = 74;
pub const KEY_KP4: u16 = 75;
pub const KEY_KP5: u16 = 76;
pub const KEY_KP6: u16 = 77;
pub const KEY_KPPLUS: u16 = 78;
pub const KEY_KP1: u16 = 79;
pub const KEY_KP2: u16 = 80;
pub const KEY_KP3: u16 = 81;
pub const KEY_KP0: u16 = 82;
pub const KEY_KPDOT: u16 = 83;
pub const KEY_KPENTER: u16 = 96;
pub const KEY_KPSLASH: u16 = 98;
pub const KEY_NUMLOCK: u16 = 69;
pub const KEY_CALC: u16 = 140;

/// 需要向虚拟键盘声明支持的键位集合（与上游 Python 版 ALL_KEYS 一致）。
pub const ALL_KEYS: [u16; 30] = [
    KEY_KP0,
    KEY_KP1,
    KEY_KP2,
    KEY_KP3,
    KEY_KP4,
    KEY_KP5,
    KEY_KP6,
    KEY_KP7,
    KEY_KP8,
    KEY_KP9,
    KEY_KPDOT,
    KEY_KPENTER,
    KEY_KPPLUS,
    KEY_KPMINUS,
    KEY_KPASTERISK,
    KEY_KPSLASH,
    KEY_BACKSPACE,
    KEY_NUMLOCK,
    KEY_CALC,
    KEY_0,
    KEY_1,
    KEY_2,
    KEY_3,
    KEY_4,
    KEY_5,
    KEY_6,
    KEY_7,
    KEY_8,
    KEY_9,
    KEY_DOT,
];

/// 透传时允许放行给桌面的 EV_KEY 码（多指工具键）。
pub const FORWARD_KEY_CODES: [u16; 5] = [
    BTN_TOUCH,
    BTN_TOOL_FINGER,
    BTN_TOOL_DOUBLETAP,
    BTN_TOOL_TRIPLETAP,
    BTN_TOOL_QUADTAP,
];

/// 透传时允许放行给桌面的 EV_ABS 码。
pub const FORWARD_ABS_CODES: [u16; 7] = [
    ABS_MT_SLOT,
    ABS_MT_TRACKING_ID,
    ABS_MT_POSITION_X,
    ABS_MT_POSITION_Y,
    ABS_MT_PRESSURE,
    ABS_MT_TOUCH_MAJOR,
    ABS_MT_WIDTH_MAJOR,
];

/// 键码的可读名（`--dump` 与日志用）。
pub fn name_of(code: u16) -> &'static str {
    match code {
        KEY_KP0 => "KP0",
        KEY_KP1 => "KP1",
        KEY_KP2 => "KP2",
        KEY_KP3 => "KP3",
        KEY_KP4 => "KP4",
        KEY_KP5 => "KP5",
        KEY_KP6 => "KP6",
        KEY_KP7 => "KP7",
        KEY_KP8 => "KP8",
        KEY_KP9 => "KP9",
        KEY_KPDOT => "KPDOT",
        KEY_KPENTER => "KPENTER",
        KEY_KPPLUS => "KPPLUS",
        KEY_KPMINUS => "KPMINUS",
        KEY_KPASTERISK => "KPASTERISK",
        KEY_KPSLASH => "KPSLASH",
        KEY_BACKSPACE => "BACKSPACE",
        KEY_NUMLOCK => "NUMLOCK",
        KEY_CALC => "CALC",
        KEY_0 => "0",
        KEY_1 => "1",
        KEY_2 => "2",
        KEY_3 => "3",
        KEY_4 => "4",
        KEY_5 => "5",
        KEY_6 => "6",
        KEY_7 => "7",
        KEY_8 => "8",
        KEY_9 => "9",
        KEY_DOT => "DOT",
        _ => "UNKNOWN",
    }
}
