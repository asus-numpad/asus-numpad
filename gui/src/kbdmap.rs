//! 键盘 4 档亮度 → 触摸板亮度 level 的映射表。
//!
//! 键盘背光由 BIOS 固定 4 档；触摸板侧能写的只有 FEATURE `0x0d` 的 level 字节，
//! 硬件真实取值是 `0x00`（熄灭）与 `0x41..=0x48`（8 档），没有中间灰度。灯光页把
//! 这两条轴对上：键盘第 N 档对应哪一个触摸板档位，由这张 4 项表决定。
//!
//! 表结构与驱动配置里的 `manual_brightness_levels` 同构（长度即档数、取值受限、
//! 要求从暗到亮不递减），因此灯光页可以直接把它写进驱动配置，让后台服务与界面
//! 用同一份档位表，不会出现「界面上是这个亮度、拔插后变成另一个亮度」。

use numdeck::config::{manual_level_allowed, VAL_BRIGHTNESS_BASE, VAL_BRIGHTNESS_MAX, VAL_OFF};
use numdeck::led_proto;

/// 键盘亮度档数（BIOS 固定四档）。
pub const KBD_STEPS: usize = 4;

/// 硬件可选的触摸板档位：熄灭 + 8 个亮度档，供下拉框使用。
pub fn level_choices() -> Vec<(u8, String)> {
    let mut out = Vec::with_capacity(VAL_BRIGHTNESS_MAX as usize + 1);
    out.push((VAL_OFF, "熄灭（0x00）".to_string()));
    for step in 1..=VAL_BRIGHTNESS_MAX {
        let value = VAL_BRIGHTNESS_BASE + step;
        out.push((value, format!("{step} 档（0x{value:02x}）")));
    }
    out
}

/// 单个档位的说明文字，与 [`level_choices`] 的标签保持一致。
pub fn level_label(value: u8) -> String {
    if value == VAL_OFF {
        return "熄灭（0x00）".to_string();
    }
    if manual_level_allowed(value) {
        return format!("{} 档（0x{value:02x}）", value - VAL_BRIGHTNESS_BASE);
    }
    format!("非法值（0x{value:02x}）")
}

/// 默认映射：与官方四档观感一致——第 1 档熄灭，末档拉到最亮。
pub fn default_map() -> Vec<u8> {
    numdeck::config::DEFAULT_MANUAL_LEVELS.to_vec()
}

/// 把任意值夹进硬件真实档位：`0x00` 保持熄灭，其余夹到 `0x41..=0x48`。
pub fn clamp_level(value: u8) -> u8 {
    if value == VAL_OFF {
        return VAL_OFF;
    }
    let max = VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX;
    value.clamp(VAL_BRIGHTNESS_BASE + 1, max)
}

/// 修正整张表：长度必须等于 [`KBD_STEPS`]，非法值夹到最近的合法档位。
pub fn sanitize(map: &mut Vec<u8>) {
    if map.len() != KBD_STEPS {
        *map = default_map();
        return;
    }
    for value in map.iter_mut() {
        if !manual_level_allowed(*value) {
            *value = clamp_level(*value);
        }
    }
}

/// 是否满足「从暗到亮不递减」——驱动写配置时的硬性要求。
pub fn monotonic(map: &[u8]) -> bool {
    map.windows(2).all(|pair| pair[0] <= pair[1])
}

/// 键盘第 `step` 档（`1..=4`）对应的触摸板 level。
pub fn level_for(step: u8, map: &[u8]) -> Option<u8> {
    if step == 0 {
        return None;
    }
    map.get(step as usize - 1).copied()
}

/// 报文预览：`0d 14 03 43 ad`。与真正下发的字节同源（[`led_proto::encode`]）。
pub fn frame_preview(level: u8) -> String {
    match led_proto::encode(level) {
        Some(frame) => led_proto::hex(&frame),
        None => "—（该值没有对应报文）".to_string(),
    }
}

/// 整张表按报文形式展开，用于提示与诊断：`1→0d 14 03 00 ad …`。
pub fn frames_text(map: &[u8]) -> String {
    map.iter()
        .enumerate()
        .map(|(index, value)| format!("{}→{}", index + 1, frame_preview(*value)))
        .collect::<Vec<_>>()
        .join("  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_map_is_four_steps_from_off_to_max() {
        let map = default_map();
        assert_eq!(map.len(), KBD_STEPS);
        assert_eq!(map.first().copied(), Some(VAL_OFF));
        assert_eq!(
            map.last().copied(),
            Some(VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX)
        );
        assert!(monotonic(&map));
    }

    #[test]
    fn sanitize_restores_bad_length() {
        let mut map = vec![0x41, 0x48];
        sanitize(&mut map);
        assert_eq!(map, default_map());
    }

    #[test]
    fn sanitize_clamps_illegal_values() {
        let mut map = vec![0x10, 0x41, 0x7f, 0x99];
        sanitize(&mut map);
        // 0x10 不是 0 也不是 0x41..=0x48，夹到最暗的真实档；0x7f/0x99 夹到 0x48
        assert_eq!(map, vec![0x41, 0x41, 0x48, 0x48]);
    }

    #[test]
    fn keep_off_as_off() {
        let mut map = vec![VAL_OFF, VAL_OFF, 0x45, 0x48];
        sanitize(&mut map);
        assert_eq!(map[0], VAL_OFF);
        assert_eq!(map[1], VAL_OFF);
    }

    #[test]
    fn monotonic_detects_fall_back() {
        assert!(monotonic(&[0x00, 0x43, 0x45, 0x48]));
        assert!(monotonic(&[0x41, 0x41, 0x48, 0x48]));
        assert!(!monotonic(&[0x00, 0x48, 0x43, 0x48]));
    }

    #[test]
    fn level_lookup_matches_index() {
        let map = default_map();
        assert_eq!(level_for(1, &map), Some(map[0]));
        assert_eq!(level_for(KBD_STEPS as u8, &map), Some(map[3]));
        assert_eq!(level_for(0, &map), None);
        assert_eq!(level_for(5, &map), None);
    }

    #[test]
    fn frame_preview_matches_protocol() {
        assert_eq!(frame_preview(0x43), "0d 14 03 43 ad");
        assert_eq!(frame_preview(VAL_OFF), "0d 14 03 00 ad");
    }

    #[test]
    fn level_choices_cover_off_plus_eight_steps() {
        let choices = level_choices();
        assert_eq!(choices.len(), VAL_BRIGHTNESS_MAX as usize + 1);
        assert_eq!(choices[0].0, VAL_OFF);
        assert_eq!(choices[1].0, VAL_BRIGHTNESS_BASE + 1);
        assert_eq!(
            choices.last().map(|(v, _)| *v),
            Some(VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX)
        );
        assert_eq!(level_label(0x41), "1 档（0x41）");
        assert_eq!(level_label(VAL_OFF), "熄灭（0x00）");
    }
}
