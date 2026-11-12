//! Linux input 事件码常量（仅取值，不依赖 evdev crate）。
//!
//! 取值来自 Linux 内核 `include/uapi/linux/input-event-codes.h`，属稳定 ABI。
//! 之所以在本工程内自带一份，是为了让 `layout` / `engine` 这两个纯逻辑模块
//! 可以在非 Linux 平台编译与单元测试；Linux 侧的 `linux` 模块会再用 evdev
//! 的类型做一次交叉校验（见 `linux::dev::assert_codes_consistent`）。

// ---- 事件类型 ----
pub const EV_SYN: u16 = 0x00;
/// 内核事件缓冲溢出：此后的事件流不可信，客户端必须丢弃并重新同步
pub const SYN_DROPPED: u16 = 0x03;
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

// ---- 主键区与常用键 ----
// 自定义键表（`custom::key_code`）允许把任意格位映射到这些键，例如在没有镭雕的
// 触摸板上贴一张小键盘贴纸、把某一格设成 `pageup` 或 `F13`。取值仍然是
// `input-event-codes.h` 的稳定 ABI，不含任何厂商私有码。
pub const KEY_ESC: u16 = 1;
pub const KEY_MINUS: u16 = 12;
pub const KEY_EQUAL: u16 = 13;
pub const KEY_TAB: u16 = 15;
pub const KEY_LEFTCTRL: u16 = 29;
pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_LEFTALT: u16 = 56;
pub const KEY_SPACE: u16 = 57;
pub const KEY_CAPSLOCK: u16 = 58;
pub const KEY_COMMA: u16 = 51;
pub const KEY_SLASH: u16 = 53;
pub const KEY_ASTERISK: u16 = 55;
pub const KEY_ENTER: u16 = 28;
pub const KEY_KPEQUAL: u16 = 117;
pub const KEY_KPCOMMA: u16 = 121;
pub const KEY_RIGHTCTRL: u16 = 97;
pub const KEY_RIGHTSHIFT: u16 = 54;
pub const KEY_RIGHTALT: u16 = 100;
pub const KEY_LEFTMETA: u16 = 125;
pub const KEY_RIGHTMETA: u16 = 126;
pub const KEY_INSERT: u16 = 110;
pub const KEY_DELETE: u16 = 111;
pub const KEY_HOME: u16 = 102;
pub const KEY_END: u16 = 107;
pub const KEY_PAGEUP: u16 = 104;
pub const KEY_PAGEDOWN: u16 = 109;
pub const KEY_UP: u16 = 103;
pub const KEY_DOWN: u16 = 108;
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const KEY_SCROLLLOCK: u16 = 70;
pub const KEY_MUTE: u16 = 113;
pub const KEY_VOLUMEDOWN: u16 = 114;
pub const KEY_VOLUMEUP: u16 = 115;
pub const KEY_PLAYPAUSE: u16 = 164;
pub const KEY_NEXTSONG: u16 = 163;
pub const KEY_PREVIOUSSONG: u16 = 165;
pub const KEY_STOPCD: u16 = 166;
pub const KEY_MAIL: u16 = 155;
pub const KEY_HOMEPAGE: u16 = 172;
pub const KEY_BACK: u16 = 158;
pub const KEY_FORWARD: u16 = 159;
pub const KEY_REFRESH: u16 = 173;
pub const KEY_SEARCH: u16 = 217;
pub const KEY_BRIGHTNESSDOWN: u16 = 224;
pub const KEY_BRIGHTNESSUP: u16 = 225;

// ---- 字母区（KEY_A .. KEY_Z 连续） ----
pub const KEY_A: u16 = 30;
pub const KEY_B: u16 = 48;
pub const KEY_C: u16 = 46;
pub const KEY_D: u16 = 32;
pub const KEY_E: u16 = 18;
pub const KEY_F: u16 = 33;
pub const KEY_G: u16 = 34;
pub const KEY_H: u16 = 35;
pub const KEY_I: u16 = 23;
pub const KEY_J: u16 = 36;
pub const KEY_K: u16 = 37;
pub const KEY_L: u16 = 38;
pub const KEY_M: u16 = 50;
pub const KEY_N: u16 = 49;
pub const KEY_O: u16 = 24;
pub const KEY_P: u16 = 25;
pub const KEY_Q: u16 = 16;
pub const KEY_R: u16 = 19;
pub const KEY_S: u16 = 31;
pub const KEY_T: u16 = 20;
pub const KEY_U: u16 = 22;
pub const KEY_V: u16 = 47;
pub const KEY_W: u16 = 17;
pub const KEY_X: u16 = 45;
pub const KEY_Y: u16 = 21;
pub const KEY_Z: u16 = 44;

// ---- 功能键（F1..F10 连续，F11/F12 与 F13..F24 不连续） ----
pub const KEY_F1: u16 = 59;
pub const KEY_F2: u16 = 60;
pub const KEY_F3: u16 = 61;
pub const KEY_F4: u16 = 62;
pub const KEY_F5: u16 = 63;
pub const KEY_F6: u16 = 64;
pub const KEY_F7: u16 = 65;
pub const KEY_F8: u16 = 66;
pub const KEY_F9: u16 = 67;
pub const KEY_F10: u16 = 68;
pub const KEY_F11: u16 = 87;
pub const KEY_F12: u16 = 88;
pub const KEY_F13: u16 = 183;
pub const KEY_F14: u16 = 184;
pub const KEY_F15: u16 = 185;
pub const KEY_F16: u16 = 186;
pub const KEY_F17: u16 = 187;
pub const KEY_F18: u16 = 188;
pub const KEY_F19: u16 = 189;
pub const KEY_F20: u16 = 190;
pub const KEY_F21: u16 = 191;
pub const KEY_F22: u16 = 192;
pub const KEY_F23: u16 = 193;
pub const KEY_F24: u16 = 194;

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
