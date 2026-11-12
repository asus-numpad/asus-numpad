//! 平台后端抽象。
//!
//! 灯光路径只有两件事：探测哪些接口能点灯、往选中的那个写 FEATURE 报文。
//! Windows 用系统自带的 `setupapi` + `hid.dll`（见 [`crate::win_hid`]）；Linux 走
//! hidraw 的 `HIDIOCSFEATURE`，探测与写入都复用驱动侧的 `linux::led`（见
//! [`crate::linux_hid`]）；其余平台用 [`crate::mock`]，让界面与档位逻辑在任意
//! 开发机上都能跑起来调试。

use anyhow::Result;

use numdeck::led_policy::{ASUS_VID, ELAN_VID, PIXART_VID};

/// 一个 HID 接口的探测结果。
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    /// 平台设备路径：Windows 是 `\\?\hid#vid_04f3&pid_3275...` 接口路径，
    /// Linux 是 `/dev/hidrawN`
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    /// 产品字符串；取不到时为空
    pub product: String,
    /// 是否暴露 FEATURE 报告 `0x0d`——只有这种接口能点灯
    pub has_feature: bool,
    /// `FeatureReportByteLength`（含 report id）；0 表示 HID 层拿不到能力
    pub feature_len: u16,
    /// [`numdeck::led_policy::candidate_rank`] 给出的优先级，越小越优先
    pub rank: Option<u8>,
    /// 是否被选为当前灯光目标
    pub selected: bool,
}

impl DeviceInfo {
    /// `04f3:3275` 形式。
    pub fn vid_pid(&self) -> String {
        format!("{:04x}:{:04x}", self.vid, self.pid)
    }

    /// 厂商归属，用于界面上区分 ELAN / PixArt / ASUS 三家的 NumberPad。
    pub fn vendor(&self) -> &'static str {
        match u32::from(self.vid) {
            ELAN_VID => "ELAN",
            PIXART_VID => "PixArt",
            ASUS_VID => "ASUS",
            _ => "未知厂商",
        }
    }

    /// 路径尾部（`col01` 这类集合名），界面里当短标签用。
    pub fn tail(&self) -> &str {
        self.path
            .rsplit(['#', '\\', '/'])
            .find(|s| !s.is_empty())
            .unwrap_or(self.path.as_str())
    }

    /// 是否能作为灯光目标：带 FEATURE 且报告长度够放下 5 字节报文。
    pub fn usable(&self) -> bool {
        self.has_feature && self.feature_len as usize >= numdeck::led_proto::LED_FEATURE_LEN
    }
}

/// 平台后端。
pub trait Backend {
    /// 界面上显示的后端名，例如 `Windows HID`。
    fn name(&self) -> &'static str;

    /// 是否真的连着硬件（模拟后端返回 false，界面据此提示）。
    fn is_real(&self) -> bool;

    /// 重新枚举所有 HID 接口。
    fn scan(&mut self) -> Result<Vec<DeviceInfo>>;

    /// 向指定接口写入一帧 FEATURE 报文。
    fn write(
        &mut self,
        device: &DeviceInfo,
        frame: [u8; numdeck::led_proto::LED_FEATURE_LEN],
    ) -> Result<()>;

    /// 读 NumLock 状态；平台读不到时返回 `None`。
    fn numlock(&mut self) -> Option<bool>;

    /// 仅模拟后端实现：界面里手动改 NumLock 状态，用来演示联动。
    fn set_numlock(&mut self, _on: bool) {}
}

/// 按当前平台挑一个后端。
pub fn create() -> Box<dyn Backend> {
    #[cfg(windows)]
    {
        Box::new(crate::win_hid::WindowsHid::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(crate::linux_hid::LinuxHid::new())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Box::new(crate::mock::MockBackend::new())
    }
}

/// 在探测结果里按 [`numdeck::led_policy`] 的优先级挑出灯光目标，并把
/// `selected` 标记写到对应条目上。
///
/// 判据与 Linux 侧完全一致（同一份 `candidate_rank`），避免两个平台在
/// 「多接口里选哪个」上出现分歧。
pub fn mark_best(devices: &mut [DeviceInfo]) -> Option<usize> {
    let tuples: Vec<(u32, u32, bool)> = devices
        .iter()
        .map(|d| (u32::from(d.vid), u32::from(d.pid), d.has_feature))
        .collect();
    let best = numdeck::led_policy::pick_best(&tuples);

    for d in devices.iter_mut() {
        d.rank = numdeck::led_policy::candidate_rank(
            u32::from(d.vid),
            u32::from(d.pid),
            d.has_feature,
        );
        d.selected = false;
    }
    if let Some(i) = best {
        devices[i].selected = true;
    }
    best
}

/// 档位表：在 `0x00` 与 `0x41..=0x48` 组成的轴上等分取 `steps` 个点。
///
/// 与 Linux 侧 `manual_brightness_levels` 用的是同一个
/// [`numdeck::brightness::linear_levels_from_off`]，所以 4 档组必然给出
/// `0x00 / 0x43 / 0x45 / 0x48`——两个平台的「第 2 档」不会一个亮一个暗。
pub fn level_table(steps: u8) -> Vec<u8> {
    numdeck::brightness::linear_levels_from_off(
        &numdeck::led_proto::brightness_levels(),
        steps as usize,
    )
}

/// 第 `step` 档（从 1 起）对应的原始命令字；越界返回 `None`。
pub fn level_for_step(steps: u8, step: u8) -> Option<u8> {
    if step == 0 {
        return None;
    }
    level_table(steps).get((step - 1) as usize).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(vid: u16, pid: u16, has_feature: bool) -> DeviceInfo {
        DeviceInfo {
            path: format!("\\\\?\\hid#vid_{vid:04x}&pid_{pid:04x}#col01"),
            vid,
            pid,
            product: String::new(),
            has_feature,
            feature_len: 5,
            rank: None,
            selected: false,
        }
    }

    #[test]
    fn elan_interface_with_feature_wins() {
        let mut devices = vec![
            dev(0x093a, 0x0001, false),
            dev(0x04f3, 0x3275, true),
            dev(0x04f3, 0x3275, false),
        ];
        let best = mark_best(&mut devices).expect("应当选中一个接口");
        assert_eq!(best, 1);
        assert!(devices[1].selected);
        assert!(!devices[0].selected);
        assert!(!devices[2].selected);
    }

    #[test]
    fn no_candidate_when_nothing_matches() {
        let mut devices = vec![dev(0x1234, 0x5678, false)];
        assert!(mark_best(&mut devices).is_none());
    }

    #[test]
    fn usable_requires_full_feature_report() {
        let mut d = dev(0x04f3, 0x3275, true);
        assert!(d.usable());
        d.feature_len = 4;
        assert!(!d.usable());
    }
}
