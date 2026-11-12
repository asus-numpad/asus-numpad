//! Linux 后端：hidraw + `HIDIOCSFEATURE`。
//!
//! 探测直接用驱动侧的 [`numdeck::linux::probe_candidates`]（读 sysfs 里的
//! 报告描述符，判有没有 FEATURE `0x0d`），写入直接用驱动侧的
//! [`numdeck::linux::led::HidrawLed`]——和 `numdeck led` 命令行是同一条
//! 代码路径，界面不会变成「另一套实现」。

use anyhow::{anyhow, Result};

use numdeck::engine::LedSink;
use numdeck::led_proto::LED_FEATURE_LEN;
use numdeck::linux::led::{probe_candidates, HidrawLed};

use crate::backend::{mark_best, Backend, DeviceInfo};

/// Linux 平台后端。
pub struct LinuxHid;

impl Default for LinuxHid {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxHid {
    pub fn new() -> Self {
        Self
    }
}

impl Backend for LinuxHid {
    fn name(&self) -> &'static str {
        "Linux hidraw（FEATURE 0x0d）"
    }

    fn is_real(&self) -> bool {
        true
    }

    fn scan(&mut self) -> Result<Vec<DeviceInfo>> {
        let candidates = probe_candidates();
        let mut devices: Vec<DeviceInfo> = candidates
            .iter()
            .map(|c| DeviceInfo {
                path: c.dev.clone(),
                vid: u16::try_from(c.vid).unwrap_or(0),
                pid: u16::try_from(c.pid).unwrap_or(0),
                product: hid_name(&c.dev),
                has_feature: c.has_feature,
                // sysfs 里能读到完整报告长度的情况很少，带 FEATURE 即按报文长度算
                feature_len: if c.has_feature {
                    LED_FEATURE_LEN as u16
                } else {
                    0
                },
                rank: Some(c.rank),
                selected: false,
            })
            .collect();
        // 选中判据用同一份 led_policy::candidate_rank，避免和 Windows 侧选出不同接口
        mark_best(&mut devices);
        Ok(devices)
    }

    fn write(&mut self, device: &DeviceInfo, frame: [u8; LED_FEATURE_LEN]) -> Result<()> {
        // HidrawLed 只回 bool，先把「打不开」和「写入被拒」分开：前者是权限
        // （udev uaccess 没生效），后者是 EC 侧没接受这条报告，处理方向完全不同。
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device.path)
            .map_err(|err| {
                anyhow!(
                    "打不开 {}：{err}。hidraw 背光通道的权限来自 udev 的 uaccess，\
                     需在 seat 内的当前会话运行（远程 ssh / cron 里没有 ACL）",
                    device.path
                )
            })?;

        // 报文第 4 字节就是 level；HidrawLed 会把它重新拼成同一帧再下发
        let value = frame[3];
        if HidrawLed::new(&device.path).send(value) {
            Ok(())
        } else {
            Err(anyhow!(
                "{} 拒绝了 FEATURE 0x0d 写入（报告不被接受或 EC 无响应）",
                device.path
            ))
        }
    }

    fn numlock(&mut self) -> Option<bool> {
        // 触摸板自身的 NumLock LED 就是内核维护的那一份；hidraw 通道读不到，
        // 从 input 子系统的 led 类节点读最简单，也不需要独占 event 设备。
        let entries = std::fs::read_dir("/sys/class/leds").ok()?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.ends_with("::numlock") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(entry.path().join("brightness")) {
                return Some(text.trim() != "0");
            }
        }
        None
    }
}

/// hidraw 节点的 HID 名（`ASUE1416:00 04F3:3275 Touchpad`），读不到时返回空串。
///
/// sysfs 的 `uevent` 是纯文本，比再开一次设备读 `HIDIOCGRAWNAME` 便宜，也不用权限。
fn hid_name(dev: &str) -> String {
    let Some(node) = dev.rsplit('/').next() else {
        return String::new();
    };
    let Ok(text) = std::fs::read_to_string(format!("/sys/class/hidraw/{node}/device/uevent"))
    else {
        return String::new();
    };
    text.lines()
        .find_map(|line| line.strip_prefix("HID_NAME="))
        .map(|name| name.trim().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hid_name_tolerates_missing_node() {
        assert_eq!(hid_name("/dev/hidraw-does-not-exist"), "");
    }

    #[test]
    fn scan_never_panics_without_hardware() {
        let mut backend = LinuxHid::new();
        let devices = backend.scan().expect("扫描不应失败");
        // 容器 / 无触摸板的机器上候选集为空，这里只验证不 panic、且选中标记自洽
        assert!(devices.iter().filter(|d| d.selected).count() <= 1);
    }
}
