//! NumberPad 背光 FEATURE 报告的跨平台编码。
//!
//! 报告描述符里带 `85 0d … b1 02` 的 FEATURE 报告只有这一个：报告 ID `0x0d`，
//! 总长 5 字节（含 ID），内容固定为
//!
//! ```text
//! [0x0d, 0x14, 0x03, <level>, 0xad]
//! ```
//!
//! `<level>` 取值：`0x00` 熄灭、`0x01` 点亮（保持当前档）、`0x41..=0x48` 对应亮度 1..8 档。
//!
//! 这份定义原先只存在于 `linux::led`。Windows 侧要写的是同一份报文，于是上移到
//! 这里作为单一真源：任何一侧的字节改动都必须同时作用于另一侧，避免两个平台
//! 的灯光行为出现漂移。

use crate::config::{VAL_BRIGHTNESS_BASE, VAL_BRIGHTNESS_MAX, VAL_OFF, VAL_ON, VAL_UNLOCK};

/// 背光 FEATURE 报告 ID（报告描述符里 `85 0d ... b1 02`）。
pub const LED_FEATURE_REPORT_ID: u8 = 0x0d;
/// 报文第 2/3 字节：固定 0x14 / 0x03（对应 usage ff00.00c4 的写法）。
pub const LED_FEATURE_BODY: [u8; 2] = [0x14, 0x03];
/// 报文末字节：固定 0xad。
pub const LED_FEATURE_TAIL: u8 = 0xad;
/// FEATURE 报告总长（含 report id）。
pub const LED_FEATURE_LEN: usize = 5;

/// 把亮度命令字映射成 FEATURE 报文里的 level 字节。
///
/// `0x01` 亮 / `0x00` 灭原样透传；`0x60`（i2c 解锁字）在 FEATURE 通道无意义，直接忽略。
pub fn level_byte(value: u8) -> Option<u8> {
    match value {
        VAL_UNLOCK => None,
        v => Some(v),
    }
}

/// 生成待写入的 FEATURE 报文；`0x60` 无对应报文。
pub fn encode(value: u8) -> Option<[u8; LED_FEATURE_LEN]> {
    let level = level_byte(value)?;
    Some([
        LED_FEATURE_REPORT_ID,
        LED_FEATURE_BODY[0],
        LED_FEATURE_BODY[1],
        level,
        LED_FEATURE_TAIL,
    ])
}

/// 亮度档位（1..=8）对应的 level 字节；越界返回 `None`。
pub fn level_value(step: u8) -> Option<u8> {
    if (1..=VAL_BRIGHTNESS_MAX).contains(&step) {
        Some(VAL_BRIGHTNESS_BASE + step)
    } else {
        None
    }
}

/// 全部亮度档位的 level 字节，从暗到亮（`0x41..=0x48`）。
pub fn brightness_levels() -> Vec<u8> {
    (1..=VAL_BRIGHTNESS_MAX).filter_map(level_value).collect()
}

/// 熄灭 / 点亮（保持当前档）两个开关键值。
pub const LED_OFF: u8 = VAL_OFF;
pub const LED_ON: u8 = VAL_ON;

/// 把报文字节格式化成 `0d 14 03 41 ad` 形式，供日志与诊断输出使用。
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_layout_matches_descriptor() {
        let buf = encode(0x43).expect("0x43 是合法档位");
        assert_eq!(buf.len(), LED_FEATURE_LEN);
        assert_eq!(buf[0], 0x0d);
        assert_eq!(&buf[1..3], &[0x14, 0x03]);
        assert_eq!(buf[3], 0x43);
        assert_eq!(buf[4], 0xad);
    }

    #[test]
    fn unlock_byte_has_no_report() {
        assert!(encode(VAL_UNLOCK).is_none());
    }

    #[test]
    fn off_and_on_pass_through() {
        assert_eq!(encode(LED_OFF).unwrap()[3], 0x00);
        assert_eq!(encode(LED_ON).unwrap()[3], 0x01);
    }

    #[test]
    fn brightness_steps_cover_full_range() {
        let levels = brightness_levels();
        assert_eq!(levels.len(), VAL_BRIGHTNESS_MAX as usize);
        assert_eq!(levels.first().copied(), Some(0x41));
        assert_eq!(levels.last().copied(), Some(0x48));
        assert!(level_value(0).is_none());
        assert!(level_value(VAL_BRIGHTNESS_MAX + 1).is_none());
    }

    #[test]
    fn hex_format_is_lowercase_pairs() {
        assert_eq!(hex(&[0x0d, 0x14, 0x03, 0x41, 0xad]), "0d 14 03 41 ad");
    }
}
