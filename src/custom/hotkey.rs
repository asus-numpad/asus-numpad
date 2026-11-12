//! 全局热键：物理键盘上的组合键 → 开 / 关 / 切换。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::keycodes as kc;

use super::*;

pub const MOD_CTRL: u8 = 1;
pub const MOD_ALT: u8 = 2;
pub const MOD_SHIFT: u8 = 4;
pub const MOD_SUPER: u8 = 8;

fn mod_of(code: u16) -> Option<u8> {
    Some(match code {
        kc::KEY_LEFTCTRL | kc::KEY_RIGHTCTRL => MOD_CTRL,
        kc::KEY_LEFTALT | kc::KEY_RIGHTALT => MOD_ALT,
        kc::KEY_LEFTSHIFT | kc::KEY_RIGHTSHIFT => MOD_SHIFT,
        kc::KEY_LEFTMETA | kc::KEY_RIGHTMETA => MOD_SUPER,
        _ => return None,
    })
}

/// 一条组合键：修饰键掩码 + 主键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub mods: u8,
    pub code: u16,
}

impl Hotkey {
    /// 解析 `ctrl+alt+n`、`super+F13`、`shift+kpenter` 这类写法。
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut mods = 0u8;
        let mut main: Option<u16> = None;
        for part in text.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let Some(code) = key_code(part) else {
                return Err(format!("unknown key `{part}`"));
            };
            if let Some(m) = mod_of(code) {
                mods |= m;
                continue;
            }
            if let Some(prev) = main {
                if prev != code {
                    return Err(format!("`{text}` has two main keys"));
                }
            }
            main = Some(code);
        }
        let Some(code) = main else {
            return Err(format!("`{text}` has no main key"));
        };
        Ok(Hotkey { mods, code })
    }

    /// 规整写法（`ctrl+alt+n` 形式），写回配置时用。
    pub fn label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.mods & MOD_CTRL != 0 {
            parts.push("ctrl");
        }
        if self.mods & MOD_ALT != 0 {
            parts.push("alt");
        }
        if self.mods & MOD_SHIFT != 0 {
            parts.push("shift");
        }
        if self.mods & MOD_SUPER != 0 {
            parts.push("super");
        }
        let mut out = parts.join("+");
        if !out.is_empty() {
            out.push('+');
        }
        out.push_str(&key_label(self.code));
        out
    }
}

/// 热键要执行的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    /// 切换数字键盘开关
    Toggle,
    /// 强制开启（不动键盘 LED 状态时也点亮）
    On,
    /// 强制关闭
    Off,
}

impl HotkeyAction {
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim().to_ascii_lowercase().as_str() {
            "" | "toggle" => Ok(HotkeyAction::Toggle),
            "on" | "enable" => Ok(HotkeyAction::On),
            "off" | "disable" => Ok(HotkeyAction::Off),
            other => Err(format!("unknown action `{other}`")),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HotkeyAction::Toggle => "toggle",
            HotkeyAction::On => "on",
            HotkeyAction::Off => "off",
        }
    }
}

/// 配置里的一条热键绑定。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct HotkeySpec {
    /// 组合键写法，如 `ctrl+alt+n`。
    pub gesture: String,
    /// 动作：`toggle`（默认）/ `on` / `off`。
    pub action: String,
}

impl Default for HotkeySpec {
    fn default() -> Self {
        Self {
            gesture: "ctrl+alt+n".to_string(),
            action: "toggle".to_string(),
        }
    }
}

/// 热键匹配器：喂物理键盘的 `EV_KEY` 事件，命中时给出动作名。
#[derive(Debug, Default)]
pub struct HotkeyState {
    binds: Vec<(Hotkey, HotkeyAction)>,
    down: HashSet<u16>,
}

impl HotkeyState {
    /// 从配置构造；解析失败的行返回提示并跳过。
    pub fn new(specs: &[HotkeySpec]) -> (Self, Vec<String>) {
        let mut binds = Vec::new();
        let mut notes = Vec::new();
        for spec in specs {
            if spec.gesture.trim().is_empty() {
                continue;
            }
            match Hotkey::parse(&spec.gesture) {
                Ok(hk) => match HotkeyAction::parse(&spec.action) {
                    Ok(action) => binds.push((hk, action)),
                    Err(err) => notes.push(crate::i18n::tf(
                        "note.hotkey_bad",
                        &[("spec", spec.gesture.trim()), ("err", &err)],
                    )),
                },
                Err(err) => notes.push(crate::i18n::tf(
                    "note.hotkey_bad",
                    &[("spec", spec.gesture.trim()), ("err", &err)],
                )),
            }
        }
        (
            Self {
                binds,
                down: HashSet::new(),
            },
            notes,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.binds.is_empty()
    }

    /// 绑定一览（日志/GUI 用）。
    pub fn describe(&self) -> Vec<String> {
        self.binds
            .iter()
            .map(|(hk, action)| format!("{} -> {}", hk.label(), action.as_str()))
            .collect()
    }

    /// 喂一个按键事件（`value`：1 按下 / 0 抬起 / 2 长按重复）。
    ///
    /// 只在「按下」那一刻判定一次；主键按住不放不会重复触发，必须抬起再按。
    pub fn feed(&mut self, code: u16, value: i32) -> Option<HotkeyAction> {
        if value == 0 {
            self.down.remove(&code);
            return None;
        }
        if value != 1 {
            return None;
        }
        self.down.insert(code);
        let mods_now = self
            .down
            .iter()
            .filter_map(|c| mod_of(*c))
            .fold(0u8, |a, b| a | b);
        for (hk, action) in &self.binds {
            if hk.code == code && hk.mods == mods_now {
                return Some(*action);
            }
        }
        None
    }

    /// 键状态复位（例如切 tty 后残留按下态）。
    pub fn clear(&mut self) {
        self.down.clear();
    }
}
