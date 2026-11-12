//! 亮度档位：官方四档手动表、线性表、自定义表，以及档位值的解析与收敛。

use serde::de::{self, Deserializer};
use serde::Deserialize;

use super::*;

/// 默认档位表：0x41..=0x48（8 档）
pub(crate) fn default_brightness_levels() -> Vec<u8> {
    (1..=VAL_BRIGHTNESS_MAX)
        .map(|i| VAL_BRIGHTNESS_BASE + i)
        .collect()
}

/// 宽松解析亮度档位表：兼容整数与 `"0x41"` / `"65"` 字符串。
/// 任何一项解析不了都直接报错（不要静默丢档位），错误信息带下标，方便定位手改错的配置。
pub(crate) fn de_brightness_levels<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<serde_json::Value>::deserialize(deserializer)?;
    parse_level_list::<D::Error>(&raw)
}

/// 同上，但允许整项缺失 / 显式 `null`（`manual_brightness_levels` 用）。
pub(crate) fn de_opt_brightness_levels<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
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
pub(crate) fn parse_level_list<E: de::Error>(raw: &[serde_json::Value]) -> Result<Vec<u8>, E> {
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

/// `manual_brightness_levels` 的默认值（配置里不写这一项时用）。
pub(crate) fn default_manual_brightness_levels() -> Option<Vec<u8>> {
    Some(DEFAULT_MANUAL_LEVELS.to_vec())
}

/// 自定义档位是否合法：`0`（熄灭）或硬件亮度档 `0x41..=0x48`。
///
/// 允许 0 是为了让「从 0 到最大线性四档」能写成 `[0, 0x43, 0x46, 0x48]`，
/// 与 `bright 0` 的熄灭语义一致；其余值一律视为手写配置错误。
pub fn manual_level_allowed(v: u8) -> bool {
    v == VAL_OFF
        || (VAL_BRIGHTNESS_BASE + 1..=VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX).contains(&v)
}

impl Config {
    pub fn effective_levels(&self) -> Vec<u8> {
        if self.follow_keyboard_brightness {
            self.brightness_levels.clone()
        } else {
            Vec::new()
        }
    }

    pub fn manual_steps(&self) -> u8 {
        match &self.manual_brightness_levels {
            Some(v) if !v.is_empty() => v.len().min(MANUAL_BRIGHTNESS_MAX_STEPS) as u8,
            _ => MANUAL_BRIGHTNESS_STEPS,
        }
    }

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

    pub fn numlock_frames(&self) -> u32 {
        // 按「实际生效的布局」取长按帧数：auto 会解析成具体布局名，
        // 解析不到时回落 GH3XLNDZ 的 62 帧（≈ 496 ms）。
        match self.layout_entry().name {
            "GH3XLNDZ" => NUMLOCK_TRIGGER_FRAMES_GH3XLNDZ,
            "GH2XLNDZ" | "GH4XLNDZ" => NUMLOCK_TRIGGER_FRAMES_GH2XLNDZ,
            _ => self.numlock_trigger_frames,
        }
    }
}
