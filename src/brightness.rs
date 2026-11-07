//! 数字区背光亮度跟随键盘背光。
//!
//! 数字区背光与键盘背光是两条独立通路：
//!
//! * 键盘背光由 `asus-wmi` 暴露成 sysfs `/sys/class/leds/asus::kbd_backlight/`
//!   （`brightness` 0..=`max_brightness`），可读可写；
//! * 数字区背光只能通过 FEATURE 报告 `0x0d 0x14 0x03 <level> 0xad` 写，
//!   硬件**不提供读回**当前亮度的途径（上游 asus-numberpad-driver 同样只能写，
//!   靠内存里记住上一次的档位来做「循环加亮」）。
//!
//! 所以「跟随」只能是单向的：轮询键盘背光的 sysfs，换算成数字区亮度档再写下去。
//!
//! 映射规则：键盘档位 `1..=max` 线性铺满 `levels` 全档（暗 → 亮）。
//! 键盘背光关闭（`kbd == 0`）时**保持最低档**（档位 1，即 `levels[0]`）：
//! 1.0.0-19/20 曾把 0 映射成中间档，结果键盘背光一路调到 0 时数字区反而变亮，
//! 观感是「亮度回弹」，比「灭着不动」更突兀（1.0.0-21 改回最低档）。
//!
//! 但 `max_brightness` 读不到（`max_kbd == 0`）时仍取中间档而不是最亮档 ——
//! 那会让灯突然全功率，与键盘当前状态完全脱节；中间档等 sysfs 起来后再校正。

use std::path::{Path, PathBuf};

use crate::config::{VAL_OFF, VAL_ON};

/// 键盘背光 sysfs 目录（asus-wmi 注册的 LED 类设备）。
pub const DEFAULT_DIR: &str = "/sys/class/leds/asus::kbd_backlight";

/// 读取 sysfs 里的十进制值。
fn read_u32(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

/// 键盘档位 → 数字区亮度命令字。纯函数，便于离线用例。
///
/// * `levels` 为空表示跟随功能关闭，返回普通点亮值 `VAL_ON`；
/// * `max_kbd` = 0 表示读不到 `max_brightness`，取**中间档**（不再盲取最亮）；
/// * `kbd` = 0 表示键盘背光关着，保持**最低档** `levels[0]`（1.0.0-21 起；不再回弹到中间档）。
pub fn map_kbd_level(kbd: u32, max_kbd: u32, levels: &[u8]) -> u8 {
    if levels.is_empty() {
        return VAL_ON;
    }
    let last = levels.len() - 1;
    // 中间档：8 档表 → 索引 3（0x44）。只用于「读不到 max_brightness」的兜底。
    let middle = levels[last / 2];
    // 键盘背光关着 → 数字区保持最暗档（档位 1），亮度只降不升，不会「回弹」。
    // 这一判断必须放在 max 兜底之前：`brightness` 读到 0 本身就已说明灯是关的，
    // 不需要 `max_brightness` 参与；否则「max 读不到 + kbd=0」两条兜底叠加，
    // 又会跑回中间档，同步模式下表现为键盘已经全灭、数字区反而变亮。
    if kbd == 0 {
        return levels[0];
    }
    if max_kbd == 0 {
        return middle;
    }
    let k = kbd.min(max_kbd);
    let idx = ((k as f64 / max_kbd as f64) * last as f64).round() as usize;
    levels[idx.min(last)]
}

/// 从全档表里线性取 `n` 个点（含首尾两端），用于生成手动档位表。
///
/// 语义是「从最暗到最亮等分」：`levels.len() == 8`、`n == 4` 时取
/// 索引 `0 / 2.33 / 4.67 / 7` 四舍五入 → `0x41 / 0x43 / 0x46 / 0x48`。
///
/// 与 [`map_kbd_level`] 的区别：那边是「键盘有 max 档、按比例换算」，
/// 首档从 `1/max` 起算；这里首档固定落在最暗档、末档固定落在最亮档，
/// 更适合「手动四档从 0（最暗）线性拉到最大」的直觉。
pub fn linear_levels(levels: &[u8], n: usize) -> Vec<u8> {
    if levels.is_empty() || n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![levels[levels.len() - 1]];
    }
    let last = levels.len() - 1;
    (0..n)
        .map(|i| {
            let pos = (i as f64 / (n - 1) as f64) * last as f64;
            levels[(pos.round() as usize).min(last)]
        })
        .collect()
}

/// 从「熄灭」到「最亮」在**硬件可达档位轴**上线性取 `n` 个点。
///
/// 硬件不是连续可调的：只有 `0x00`（灭）与 `0x41..=0x48`（8 档）。把可用值排成一条轴
/// `[00, 41, 42, ..., 48]`（9 点），在上面等分取点，就等于「从 0 到最大线性」。
///
/// 默认 8 档 + 4 档 → 索引 `0 / 2.67 / 5.33 / 8` 四舍五入 → `0x00 / 0x43 / 0x45 / 0x48`。
/// 首位固定落 `0x00`（灭），末位固定落最亮档。
///
/// 与 [`linear_levels`] 的差别：那个只在 `levels` 内部等分（首档是最暗的**亮**档），
/// 拉不到熄灭；这个把 `0x00` 一起算进轴里。
pub fn linear_levels_from_off(levels: &[u8], n: usize) -> Vec<u8> {
    if levels.is_empty() || n == 0 {
        return Vec::new();
    }
    // 只有一档时没有「两端」可言，取最亮——「1 档」总比「1 档就是黑的」符合直觉
    if n == 1 {
        return vec![levels[levels.len() - 1]];
    }
    // 自定义表里可能已经手写了 0x00，避免轴上出现两个零点
    let mut axis: Vec<u8> = Vec::with_capacity(levels.len() + 1);
    if levels[0] != VAL_OFF {
        axis.push(VAL_OFF);
    }
    axis.extend_from_slice(levels);
    linear_levels(&axis, n)
}

/// 跟随器：持有 sysfs 路径、档位表与上一次读到的值。
pub struct BrightnessFollower {
    /// 配置里给的原始路径，重解析时用它做起点（不能拿已解析结果去解析）
    configured: String,
    dir: PathBuf,
    levels: Vec<u8>,
    /// `max_brightness`，0 表示尚未读到
    max_kbd: u32,
    /// 上一次读到的键盘档位
    pub last_kbd: Option<u32>,
    last_poll_ms: f64,
    current: Option<u8>,
    /// 是否已经打过「读不到键盘背光」的提示，避免刷屏
    warned: bool,
}

/// 解析实际可用的键盘背光目录。
///
/// 配置里给的是标准路径 `asus::kbd_backlight`，但不同机型/内核暴露的名字不完全一样
/// （有的分支叫 `asus_wmi::kbd_backlight`），所以配置路径不存在时退化为按名字扫描，
/// 扫不到就原样返回，让上层走「读不到 → 固定点亮」的兜底。
pub fn resolve_dir(configured: &str) -> PathBuf {
    let configured_path = PathBuf::from(configured);
    if configured_path.join("brightness").exists() {
        return configured_path;
    }
    if let Ok(entries) = std::fs::read_dir("/sys/class/leds") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.contains("kbd_backlight") {
                let candidate = entry.path();
                if candidate.join("brightness").exists() {
                    return candidate;
                }
            }
        }
    }
    configured_path
}

impl BrightnessFollower {
    pub fn new(dir: &str, levels: Vec<u8>) -> Self {
        let resolved = resolve_dir(dir);
        let max_kbd = read_u32(&resolved.join("max_brightness")).unwrap_or(0);
        Self {
            configured: dir.to_string(),
            dir: resolved,
            levels,
            max_kbd,
            last_kbd: None,
            last_poll_ms: f64::NEG_INFINITY,
            current: None,
            warned: false,
        }
    }

    pub fn enabled(&self) -> bool {
        !self.levels.is_empty()
    }

    fn read_kbd(&mut self) -> Option<u32> {
        if let Some(v) = read_u32(&self.dir.join("brightness")) {
            if self.max_kbd == 0 {
                self.max_kbd = read_u32(&self.dir.join("max_brightness"))
                    .unwrap_or(3)
                    .max(1);
            }
            return Some(v);
        }
        // 读不到不代表这台机器没有键盘背光：systemd --user 服务往往比 asus-wmi 注册
        // LED 类设备更早起来，1.0.0-18 在 new() 里只解析一次，把「还没出现」永久固化成
        // 「不存在」，之后再也不重试 —— 表现为服务日志只有一条 warning，灯从头到尾不亮。
        // 这里每次读失败都重新解析一次，驱动起来后即可自动接上。
        let fresh = resolve_dir(&self.configured);
        if fresh != self.dir {
            if let Some(v) = read_u32(&fresh.join("brightness")) {
                self.dir = fresh;
                self.max_kbd = read_u32(&self.dir.join("max_brightness")).unwrap_or(0);
                if self.max_kbd == 0 {
                    self.max_kbd = 3;
                }
                return Some(v);
            }
        }
        None
    }

    /// 点灯时下发的命令字：能读到键盘背光就用跟随档位，否则退回普通点亮值。
    pub fn on_value(&mut self) -> u8 {
        if !self.enabled() {
            return VAL_ON;
        }
        let kbd = match self.read_kbd() {
            Some(v) => v,
            None => {
                if !self.warned {
                    self.warned = true;
                    eprintln!(
                        "{}",
                        crate::i18n::tf(
                            "bright.warn.no_kbd_dir",
                            &[
                                ("dir", &self.dir.display().to_string()),
                                ("level", &format!("{VAL_ON:02x}")),
                            ]
                        )
                    );
                }
                return VAL_ON;
            }
        };
        self.last_kbd = Some(kbd);
        let level = map_kbd_level(kbd, self.max_kbd, &self.levels);
        self.current = Some(level);
        level
    }

    /// 节流轮询键盘背光；档位变化时返回新的数字区命令字。
    ///
    /// 主循环是几十赫兹的帧循环，sysfs 读取开销虽小也不该每帧都读，
    /// 因此按 `interval_ms` 节流；键盘背光变化是秒级操作，500 ms 足够跟上。
    pub fn poll(&mut self, now_ms: f64, interval_ms: f64) -> Option<u8> {
        if !self.enabled() {
            return None;
        }
        if now_ms - self.last_poll_ms < interval_ms {
            return None;
        }
        self.last_poll_ms = now_ms;
        let kbd = match self.read_kbd() {
            Some(v) => v,
            None => {
                // 提示由 `on_value` 负责，这里只记下状态供日志使用
                self.warned = true;
                return None;
            }
        };
        if self.last_kbd == Some(kbd) {
            return None;
        }
        self.last_kbd = Some(kbd);
        let level = map_kbd_level(kbd, self.max_kbd, &self.levels);
        if self.current == Some(level) {
            return None;
        }
        self.current = Some(level);
        Some(level)
    }

    pub fn levels(&self) -> &[u8] {
        &self.levels
    }

    /// 实际使用的 sysfs 目录（可能与配置值不同，见 [`resolve_dir`]）。
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 读到的 `max_brightness`，0 表示还没读到。
    pub fn max_kbd(&self) -> u32 {
        self.max_kbd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> Vec<u8> {
        (0x41..=0x48).collect()
    }

    #[test]
    fn zero_keeps_dimmest_level() {
        // 1.0.0-21：键盘背光关着时保持最低档 0x41。
        // 1.0.0-19/20 曾取中间档 0x44，键盘一路调暗到 0 时数字区反而变亮，观感是「回弹」。
        assert_eq!(map_kbd_level(0, 3, &levels()), 0x41);
        assert_eq!(map_kbd_level(0, 8, &levels()), 0x41);
    }

    #[test]
    fn full_scale_uses_brightest_level() {
        assert_eq!(map_kbd_level(3, 3, &levels()), 0x48);
        assert_eq!(map_kbd_level(4, 3, &levels()), 0x48); // 越界夹紧
    }

    #[test]
    fn middle_levels_are_monotonic() {
        // 1.0.0-21 起 kbd = 0 也落在序列起点（最低档），整段必须单调不减。
        let l = levels();
        let mut prev = map_kbd_level(0, 3, &l);
        assert_eq!(prev, 0x41);
        for k in 1..=3 {
            let cur = map_kbd_level(k, 3, &l);
            assert!(cur >= prev, "step {k} got darker instead of brighter");
            prev = cur;
        }
    }

    #[test]
    fn four_step_keyboard_maps_across_all_levels() {
        let l = levels();
        assert_eq!(map_kbd_level(0, 4, &l), 0x41); // 关闭 → 保持最低档
        assert_eq!(map_kbd_level(1, 4, &l), 0x43);
        assert_eq!(map_kbd_level(4, 4, &l), 0x48);
    }

    #[test]
    fn unknown_max_uses_middle_level() {
        // max_brightness 读不到时不再赌「最亮」，否则灯会突然全功率且无法解释
        assert_eq!(map_kbd_level(1, 0, &levels()), 0x44);
    }

    #[test]
    fn zero_kbd_wins_over_unknown_max() {
        // max 读不到 + 键盘背光关着：两条兜底叠加时，仍必须保持最低档，不能回弹到中间档
        assert_eq!(map_kbd_level(0, 0, &levels()), 0x41);
    }

    #[test]
    fn empty_levels_disable_following() {
        assert_eq!(map_kbd_level(2, 3, &[]), VAL_ON);
    }
}
