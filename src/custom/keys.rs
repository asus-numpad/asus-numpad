//! 键名 ↔ 键码：配置里写 `kp7` / `F13` / `0x64` 都能解析，回显走反查。

use crate::keycodes as kc;

/// 字母键：内核里 `KEY_A` 与 `KEY_B` 并不连续（30 之后是 48），只能查表。
const LETTERS: [(char, u16); 26] = [
    ('A', kc::KEY_A),
    ('B', kc::KEY_B),
    ('C', kc::KEY_C),
    ('D', kc::KEY_D),
    ('E', kc::KEY_E),
    ('F', kc::KEY_F),
    ('G', kc::KEY_G),
    ('H', kc::KEY_H),
    ('I', kc::KEY_I),
    ('J', kc::KEY_J),
    ('K', kc::KEY_K),
    ('L', kc::KEY_L),
    ('M', kc::KEY_M),
    ('N', kc::KEY_N),
    ('O', kc::KEY_O),
    ('P', kc::KEY_P),
    ('Q', kc::KEY_Q),
    ('R', kc::KEY_R),
    ('S', kc::KEY_S),
    ('T', kc::KEY_T),
    ('U', kc::KEY_U),
    ('V', kc::KEY_V),
    ('W', kc::KEY_W),
    ('X', kc::KEY_X),
    ('Y', kc::KEY_Y),
    ('Z', kc::KEY_Z),
];

/// `F1..F24` 的键码表（`F1..F10` 连续，`F11` 起不连续）。
const FUNCTIONS: [u16; 24] = [
    kc::KEY_F1,
    kc::KEY_F2,
    kc::KEY_F3,
    kc::KEY_F4,
    kc::KEY_F5,
    kc::KEY_F6,
    kc::KEY_F7,
    kc::KEY_F8,
    kc::KEY_F9,
    kc::KEY_F10,
    kc::KEY_F11,
    kc::KEY_F12,
    kc::KEY_F13,
    kc::KEY_F14,
    kc::KEY_F15,
    kc::KEY_F16,
    kc::KEY_F17,
    kc::KEY_F18,
    kc::KEY_F19,
    kc::KEY_F20,
    kc::KEY_F21,
    kc::KEY_F22,
    kc::KEY_F23,
    kc::KEY_F24,
];

/// 自定义键表里可以写的主键区数字键名（`0`..`9`）。
fn digit_code(d: u8) -> Option<u16> {
    Some(match d {
        0 => kc::KEY_0,
        1 => kc::KEY_1,
        2 => kc::KEY_2,
        3 => kc::KEY_3,
        4 => kc::KEY_4,
        5 => kc::KEY_5,
        6 => kc::KEY_6,
        7 => kc::KEY_7,
        8 => kc::KEY_8,
        9 => kc::KEY_9,
        _ => return None,
    })
}

/// 键名 → Linux 键码。
///
/// 名字大小写不敏感，`_` / `-` / 空格一律忽略；`KP` 前缀表示小键盘键（`kp1` 与
/// `kp_1` 等价）。除下表列出的别名外，还接受纯文本键名（见 [`key_label`]）。
pub fn key_code(name: &str) -> Option<u16> {
    let norm: String = name
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .collect::<String>()
        .to_ascii_uppercase();
    if norm.is_empty() {
        return None;
    }
    // 单字符：数字 / 字母
    let mut chars = norm.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if let Some(d) = c.to_digit(10) {
            return digit_code(d as u8);
        }
        if c.is_ascii_alphabetic() {
            return LETTERS
                .iter()
                .find(|(ch, _)| *ch == c.to_ascii_uppercase())
                .map(|(_, code)| *code);
        }
    }
    // 小键盘数字：KP0..KP9
    if let Some(rest) = norm.strip_prefix("KP") {
        if rest.len() == 1 {
            if let Some(d) = rest.chars().next().and_then(|c| c.to_digit(10)) {
                return Some(match d as u8 {
                    0 => kc::KEY_KP0,
                    1 => kc::KEY_KP1,
                    2 => kc::KEY_KP2,
                    3 => kc::KEY_KP3,
                    4 => kc::KEY_KP4,
                    5 => kc::KEY_KP5,
                    6 => kc::KEY_KP6,
                    7 => kc::KEY_KP7,
                    8 => kc::KEY_KP8,
                    9 => kc::KEY_KP9,
                    _ => return None,
                });
            }
        }
    }
    let code = match norm.as_str() {
        // 小键盘符号
        "KP" | "KPDOT" | "KPPERIOD" => kc::KEY_KPDOT,
        "KPSLASH" | "KPDIV" | "KPDIVIDE" => kc::KEY_KPSLASH,
        "KPASTERISK" | "KPSTAR" | "KPMUL" => kc::KEY_KPASTERISK,
        "KPMINUS" | "KPSUB" => kc::KEY_KPMINUS,
        "KPPLUS" | "KPADD" => kc::KEY_KPPLUS,
        "KPENTER" | "KPRETURN" => kc::KEY_KPENTER,
        "KPEQUAL" => kc::KEY_KPEQUAL,
        "KPCOMMA" => kc::KEY_KPCOMMA,
        // 主键区符号与常用编辑键
        "DOT" | "PERIOD" => kc::KEY_DOT,
        "COMMA" => kc::KEY_COMMA,
        "SLASH" | "DIV" => kc::KEY_SLASH,
        "ASTERISK" | "STAR" => kc::KEY_ASTERISK,
        "MINUS" => kc::KEY_MINUS,
        "PLUS" | "EQUAL" => kc::KEY_EQUAL,
        "SPACE" => kc::KEY_SPACE,
        "TAB" => kc::KEY_TAB,
        "BACKSPACE" | "BS" => kc::KEY_BACKSPACE,
        "ENTER" | "RETURN" | "RET" => kc::KEY_ENTER,
        "ESC" | "ESCAPE" => kc::KEY_ESC,
        "DELETE" | "DEL" => kc::KEY_DELETE,
        "INSERT" | "INS" => kc::KEY_INSERT,
        "HOME" => kc::KEY_HOME,
        "END" => kc::KEY_END,
        "PAGEUP" | "PGUP" => kc::KEY_PAGEUP,
        "PAGEDOWN" | "PGDN" => kc::KEY_PAGEDOWN,
        "UP" => kc::KEY_UP,
        "DOWN" => kc::KEY_DOWN,
        "LEFT" => kc::KEY_LEFT,
        "RIGHT" => kc::KEY_RIGHT,
        // 指示灯 / 锁定键
        "NUMLOCK" | "NUMLK" => kc::KEY_NUMLOCK,
        "CAPSLOCK" | "CAPSLK" => kc::KEY_CAPSLOCK,
        "SCROLLLOCK" | "SCROLLLK" => kc::KEY_SCROLLLOCK,
        // 计算器等
        "CALC" | "CALCULATOR" => kc::KEY_CALC,
        "MAIL" => kc::KEY_MAIL,
        "HOMEPAGE" | "WWW" => kc::KEY_HOMEPAGE,
        "BACK" => kc::KEY_BACK,
        "FORWARD" => kc::KEY_FORWARD,
        "REFRESH" => kc::KEY_REFRESH,
        "SEARCH" => kc::KEY_SEARCH,
        // 媒体键
        "MUTE" => kc::KEY_MUTE,
        "VOLUMEDOWN" | "VOLDOWN" => kc::KEY_VOLUMEDOWN,
        "VOLUMEUP" | "VOLUP" => kc::KEY_VOLUMEUP,
        "PLAYPAUSE" | "PLAY" => kc::KEY_PLAYPAUSE,
        "NEXTSONG" | "NEXT" => kc::KEY_NEXTSONG,
        "PREVIOUSSONG" | "PREV" => kc::KEY_PREVIOUSSONG,
        "STOPCD" | "STOP" => kc::KEY_STOPCD,
        "BRIGHTNESSDOWN" => kc::KEY_BRIGHTNESSDOWN,
        "BRIGHTNESSUP" => kc::KEY_BRIGHTNESSUP,
        // 修饰键（也可以出现在热键组合里）
        // 不带左右的名字默认取左侧修饰键，热键写法（`ctrl+alt+n`）才够自然
        "CTRL" | "CONTROL" | "LEFTCTRL" | "LCTRL" => kc::KEY_LEFTCTRL,
        "RIGHTCTRL" | "RCTRL" => kc::KEY_RIGHTCTRL,
        "ALT" | "LEFTALT" | "LALT" => kc::KEY_LEFTALT,
        "RIGHTALT" | "RALT" | "ALTGR" => kc::KEY_RIGHTALT,
        "SHIFT" | "LEFTSHIFT" | "LSHIFT" => kc::KEY_LEFTSHIFT,
        "RIGHTSHIFT" | "RSHIFT" => kc::KEY_RIGHTSHIFT,
        "META" | "LEFTMETA" | "LMETA" | "SUPER" | "WIN" => kc::KEY_LEFTMETA,
        "RIGHTMETA" | "RMETA" => kc::KEY_RIGHTMETA,
        _ => return function_or_raw(&norm),
    };
    Some(code)
}

/// `F1..F24` 与 `0x..` / 十进制原始键码。
fn function_or_raw(norm: &str) -> Option<u16> {
    if let Some(rest) = norm.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(FUNCTIONS[(n - 1) as usize]);
            }
        }
    }
    let lower = norm.to_ascii_lowercase();
    if let Some(hex) = lower.strip_prefix("0x") {
        return u16::from_str_radix(hex, 16).ok().filter(|c| *c > 0);
    }
    if norm.chars().all(|c| c.is_ascii_digit()) {
        return norm.parse::<u16>().ok().filter(|c| *c > 0);
    }
    None
}

/// 键码 → 便于人读的名字（写回配置时用得到，日志里也用它）。
pub fn key_label(code: u16) -> String {
    let named = [
        (kc::KEY_KP0, "kp0"),
        (kc::KEY_KP1, "kp1"),
        (kc::KEY_KP2, "kp2"),
        (kc::KEY_KP3, "kp3"),
        (kc::KEY_KP4, "kp4"),
        (kc::KEY_KP5, "kp5"),
        (kc::KEY_KP6, "kp6"),
        (kc::KEY_KP7, "kp7"),
        (kc::KEY_KP8, "kp8"),
        (kc::KEY_KP9, "kp9"),
        (kc::KEY_KPDOT, "kpdot"),
        (kc::KEY_KPSLASH, "kpslash"),
        (kc::KEY_KPASTERISK, "kpasterisk"),
        (kc::KEY_KPMINUS, "kpminus"),
        (kc::KEY_KPPLUS, "kpplus"),
        (kc::KEY_KPENTER, "kpenter"),
        (kc::KEY_BACKSPACE, "backspace"),
        (kc::KEY_ENTER, "enter"),
        (kc::KEY_DOT, "dot"),
        (kc::KEY_SPACE, "space"),
        (kc::KEY_TAB, "tab"),
        (kc::KEY_ESC, "esc"),
        (kc::KEY_DELETE, "delete"),
        (kc::KEY_NUMLOCK, "numlock"),
        (kc::KEY_CALC, "calc"),
    ];
    for (c, name) in named {
        if c == code {
            return name.to_string();
        }
    }
    if (kc::KEY_0..=kc::KEY_9).contains(&code) {
        return (code - kc::KEY_0).to_string();
    }
    if let Some((ch, _)) = LETTERS.iter().find(|(_, c)| *c == code) {
        return ch.to_ascii_lowercase().to_string();
    }
    if let Some(pos) = FUNCTIONS.iter().position(|c| *c == code) {
        return format!("F{}", pos + 1);
    }
    let ascii = kc::name_of(code);
    if ascii != "?" && ascii != "UNKNOWN" {
        return ascii.to_ascii_lowercase();
    }
    format!("0x{code:02x}")
}
