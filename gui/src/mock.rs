//! 非 Windows 平台的模拟后端。
//!
//! 存在的意义只有两个：让 `cargo test` 与界面预览在 macOS / Linux 上跑得起来，
//! 以及在 CI 里做不需要硬件的回归。它不碰任何真实设备。

use anyhow::Result;

use crate::backend::{mark_best, Backend, DeviceInfo};

/// 记录写入历史、NumLock 状态可手动切换的假后端。
pub struct MockBackend {
    numlock: bool,
    /// 已写入的报文，`--selftest` 与界面上都会显示
    pub sent: Vec<[u8; numdeck::led_proto::LED_FEATURE_LEN]>,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            numlock: true,
            sent: Vec::new(),
        }
    }
}

impl Backend for MockBackend {
    fn name(&self) -> &'static str {
        "模拟后端（未接硬件）"
    }

    fn is_real(&self) -> bool {
        false
    }

    fn scan(&mut self) -> Result<Vec<DeviceInfo>> {
        // 两个接口：只有带 FEATURE 的那个能点灯，与真机上「触摸板会导出多个集合」
        // 的形态一致，方便验证选中逻辑。
        let mut devices = vec![
            DeviceInfo {
                path: "mock://vid_04f3&pid_3275&col01".to_string(),
                vid: 0x04f3,
                pid: 0x3275,
                product: "ASUS NumberPad (模拟)".to_string(),
                has_feature: true,
                feature_len: 5,
                rank: None,
                selected: false,
            },
            DeviceInfo {
                path: "mock://vid_04f3&pid_3275&col02".to_string(),
                vid: 0x04f3,
                pid: 0x3275,
                product: "ASUS NumberPad Touchpad (模拟)".to_string(),
                has_feature: false,
                feature_len: 0,
                rank: None,
                selected: false,
            },
        ];
        mark_best(&mut devices);
        Ok(devices)
    }

    fn write(
        &mut self,
        _device: &DeviceInfo,
        frame: [u8; numdeck::led_proto::LED_FEATURE_LEN],
    ) -> Result<()> {
        self.sent.push(frame);
        Ok(())
    }

    fn numlock(&mut self) -> Option<bool> {
        Some(self.numlock)
    }

    fn set_numlock(&mut self, on: bool) {
        self.numlock = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_selects_the_feature_interface() {
        let mut backend = MockBackend::new();
        let devices = backend.scan().unwrap();
        let selected = devices.iter().position(|d| d.selected).unwrap();
        assert!(devices[selected].has_feature);
    }

    #[test]
    fn mock_records_writes() {
        let mut backend = MockBackend::new();
        let devices = backend.scan().unwrap();
        let frame = numdeck::led_proto::encode(0x45).unwrap();
        backend.write(&devices[0], frame).unwrap();
        assert_eq!(backend.sent, vec![frame]);
    }
}
