//! 硬件探测里「纯数据」的那一半：HID 报告描述符、`uevent`、ioctl 返回的位图。
//!
//! 这些函数决定的是**自动识别**的正确性：选哪个 hidraw 接口点灯、认出哪台机型、
//! 认为触摸板支持哪些事件。真机不可能随时在手边，所以判定逻辑必须能在任意平台
//! 上按样本跑单测，`libc`/`ioctl` 只留在 `linux` 模块里负责把字节取回来。
//!
//! 与 [`crate::led_policy`] 同样的分工：跨平台模块给结论，Linux 侧给数据。

/// 报告描述符探测结果。
///
/// 必须把「确实没有 FEATURE 报告」与「没权限看」分开：后者是
/// udev 的 `TAG+="uaccess"` 没生效（规则未装 / 未 trigger / 非 seat 会话），
/// 报成“没有 FEATURE 0x0d”会把排障方向带到硬件上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureProbe {
    /// 描述符里有 FEATURE 报告 `report_id`
    Has,
    /// 描述符里确实没有该 FEATURE 报告
    Missing,
    /// 节点打不开（权限不足，多为 uaccess 未生效）
    NoAccess,
    /// 打得开但读描述符失败（节点刚消失 / 驱动异常）
    Unknown,
}

impl FeatureProbe {
    /// 是否可作为“描述符证据成立”使用。
    pub fn has(self) -> bool {
        matches!(self, FeatureProbe::Has)
    }
}

/// 在 HID 报告描述符字节里找 `report_id` 的 FEATURE 主项。
///
/// 一次导出的 hidraw 里既有做鼠标/键盘的接口，也有带厂商 FEATURE 的接口；
/// 只有带 `0x0d` FEATURE 的那个能点灯，选错接口时 ioctl 会返回 EINVAL。
/// 所以这段解析解析错一个字节，就会去点一个永远点不亮的节点。
///
/// 只回答「有 / 确实没有」；「打不开」「读不到」属于 IO 层，由调用方区分。
pub fn feature_in_descriptor(desc: &[u8], report_id: u8) -> FeatureProbe {
    let len = desc.len();
    let mut i = 0usize;
    let mut in_target = false;
    while i < len {
        let item = desc[i];
        // 长项：0xFE, bDataSize, bLongItemTag, data[bDataSize]。整块跳过，
        // 否则里头的 0x85 / 0xb1 字节会被当成主项误判成「有这个 FEATURE」。
        if item == 0xfe {
            let size = desc.get(i + 1).copied().unwrap_or(0) as usize;
            i += 3 + size;
            continue;
        }
        if item == 0x85 {
            // Report ID 主项（1 字节数据）
            in_target = desc.get(i + 1).copied() == Some(report_id);
            i += 2;
            continue;
        }
        if item == 0xb1 && in_target {
            // Feature 主项
            return FeatureProbe::Has;
        }
        let payload = match item & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 4,
        };
        i += 1 + payload;
    }
    // 描述符读全了也没找到该 FEATURE 报告：这是真的没有。
    FeatureProbe::Missing
}

/// 从 hidraw 的 `uevent` 文本里取 `(vid, pid)`。
///
/// `HID_ID` 的格式是 `bus:VID:PID`，三段都是十六进制。不同内核/驱动写出来的
/// 大小写、前导零并不一致，解析错就认不出机型，只能退到「未知设备」上火候
/// 更粗的兜底逻辑。
pub fn hid_id_from_uevent(text: &str) -> Option<(u32, u32)> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("HID_ID=") else {
            continue;
        };
        let mut it = rest.trim().split(':');
        let _bus = it.next()?;
        let vid = u32::from_str_radix(it.next()?.trim(), 16).ok()?;
        let pid = u32::from_str_radix(it.next()?.trim(), 16).ok()?;
        return Some((vid, pid));
    }
    None
}

/// ioctl 位图 → 置位的编号列表（`EVIOCGBIT` / `EVIOCGLED` 的返回格式）。
///
/// 内核按小端字节序逐位排，第 n 位表示「支持第 n 号事件码」。自动判定
/// 「这台设备的触摸板支不支持压力 / 边缘滑动」全靠它。
pub fn bits(buf: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, byte) in buf.iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                out.push(i * 8 + bit);
            }
        }
    }
    out
}

/// `EVIOCGLED` 返回的位图里 NumLock 指示灯是否亮着（`LED_NUML` = bit 0）。
///
/// 用灯状态反推 NumLock，比解析 `/sys/class/leds` 稳：不依赖节点命名，也不受
/// 键盘厂商驱动影响。读到空位图按「未知」处理，不能默认按关处理，否则
/// NumLock 明明开着却不点灯。
pub fn numlock_led_on(buf: &[u8]) -> Option<bool> {
    buf.first().map(|byte| byte & 0x01 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实 ELAN 触摸板描述符的骨架：鼠标集合在前，FEATURE 0x0d 在后。
    fn elan_like_descriptor() -> Vec<u8> {
        vec![
            0x05, 0x01, // Usage Page (Generic Desktop)
            0x09, 0x02, // Usage (Mouse)
            0xa1, 0x01, // Collection (Application)
            0x85, 0x0c, // Report ID 0x0c
            0x09, 0x01, //   Usage (Pointer)
            0x81, 0x02, //   Input (Data,Var,Abs)
            0xc0, // End Collection
            0x06, 0x00, 0xff, // Usage Page (Vendor)
            0x09, 0x01, // Usage (Vendor 1)
            0xa1, 0x01, // Collection (Application)
            0x85, 0x0d, // Report ID 0x0d
            0x09, 0x01, //   Usage (Vendor 1)
            0x15, 0x00, //   Logical Minimum (0)
            0x26, 0xff, 0x00, // Logical Maximum (255)
            0x75, 0x08, //   Report Size (8)
            0x95, 0x05, //   Report Count (5)
            0xb1, 0x02, //   Feature (Data,Var,Abs)
            0xc0, // End Collection
        ]
    }

    #[test]
    fn feature_found_for_target_report_id() {
        assert_eq!(
            feature_in_descriptor(&elan_like_descriptor(), 0x0d),
            FeatureProbe::Has
        );
    }

    #[test]
    fn input_item_is_not_a_feature() {
        // 0x0c 那一组只有 Input 主项，不能因为它存在就认为能点灯
        assert_eq!(
            feature_in_descriptor(&elan_like_descriptor(), 0x0c),
            FeatureProbe::Missing
        );
    }

    #[test]
    fn absent_report_id_is_missing() {
        assert_eq!(
            feature_in_descriptor(&elan_like_descriptor(), 0x07),
            FeatureProbe::Missing
        );
        assert_eq!(feature_in_descriptor(&[], 0x0d), FeatureProbe::Missing);
    }

    #[test]
    fn long_item_is_skipped_whole() {
        // 长项内部恰好带着 0x85 0x0d 0xb1 三个字节：整块跳过才不会误判
        let desc = vec![
            0xfe, 0x05, 0x10, // long item, 5 字节数据, tag 0x10
            0x85, 0x0d, 0xb1, 0x00,
            0x00, // 长项数据（看起来像 report id + feature）
            0x85, 0x0c, // 真正的 Report ID 0x0c
            0x81, 0x02, // Input
        ];
        assert_eq!(feature_in_descriptor(&desc, 0x0d), FeatureProbe::Missing);
    }

    #[test]
    fn truncated_descriptor_never_panics() {
        let full = elan_like_descriptor();
        for cut in 0..=full.len() {
            let _ = feature_in_descriptor(&full[..cut], 0x0d);
        }
        // 只有报头没有后续字节
        assert_eq!(feature_in_descriptor(&[0x85], 0x0d), FeatureProbe::Missing);
        assert_eq!(feature_in_descriptor(&[0xfe], 0x0d), FeatureProbe::Missing);
    }

    #[test]
    fn four_byte_payload_item_is_stepped_over() {
        // 0x17 = 主项 + 4 字节数据；后面紧跟真正的 0x85 0x0d 0xb1
        let desc = vec![0x17, 1, 2, 3, 4, 0x85, 0x0d, 0xb1, 0x02];
        assert_eq!(feature_in_descriptor(&desc, 0x0d), FeatureProbe::Has);
    }

    #[test]
    fn uevent_vid_pid_is_parsed() {
        let text = "DRIVER=hid-generic\nHID_ID=0018:000004F3:0000320B\nHID_NAME=ASUE1416:00 04F3:320B Touchpad\n";
        assert_eq!(hid_id_from_uevent(text), Some((0x04f3, 0x320b)));
    }

    #[test]
    fn uevent_lowercase_hex_is_parsed() {
        assert_eq!(
            hid_id_from_uevent("HID_ID=0018:00000b05:00001a2f\n"),
            Some((0x0b05, 0x1a2f))
        );
    }

    #[test]
    fn uevent_without_hid_id_is_none() {
        assert_eq!(hid_id_from_uevent("HID_NAME=keyboard\n"), None);
        assert_eq!(hid_id_from_uevent(""), None);
    }

    #[test]
    fn uevent_with_broken_hex_is_none() {
        // 少一段 / 非十六进制都当没有，不能装作认出来
        assert_eq!(hid_id_from_uevent("HID_ID=0018:000004F3\n"), None);
        assert_eq!(hid_id_from_uevent("HID_ID=0018:00ZZ:0001\n"), None);
    }

    #[test]
    fn bits_lists_set_positions() {
        assert_eq!(bits(&[0x00]), Vec::<usize>::new());
        assert_eq!(bits(&[0b0000_0001]), vec![0]);
        assert_eq!(bits(&[0b1000_0000]), vec![7]);
        assert_eq!(bits(&[0x01, 0x80]), vec![0, 15]);
        // EV_KEY 普遍在 1 号位：触摸板事件位图里 EV_ABS(3)/EV_KEY(1) 都要能读出来
        assert_eq!(bits(&[0b0000_1010]), vec![1, 3]);
    }

    #[test]
    fn bits_handles_full_width() {
        let all = [0xffu8; 12];
        let list = bits(&all);
        assert_eq!(list.len(), 96);
        assert_eq!(list.first().copied(), Some(0));
        assert_eq!(list.last().copied(), Some(95));
    }

    #[test]
    fn numlock_led_bit0_is_on() {
        assert_eq!(numlock_led_on(&[0x01]), Some(true));
        assert_eq!(numlock_led_on(&[0x00]), Some(false));
        // 其他灯亮着不算 NumLock（CapsLock 是 bit 1、ScrollLock 是 bit 2）
        assert_eq!(numlock_led_on(&[0x06]), Some(false));
        // 读不到位图 ≠ NumLock 关
        assert_eq!(numlock_led_on(&[]), None);
    }
}
