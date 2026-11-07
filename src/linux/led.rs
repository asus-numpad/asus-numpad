//! 背光通道：hidraw / i2c / 空实现。
//!
//! ## hidraw 通道
//!
//! 触摸板自己暴露的 HID 接口里有一个 **FEATURE 报告 ID `0x0d`**：
//!
//! ```text
//! 报告描述符片段：85 0d 09 c4 a1 01 ... 95 04 b1 02
//!                 ^^ Report ID 0x0d       ^^^^^ 4 字节 Feature（Usage Page ff00 / Usage 00c4）
//! ```
//!
//! 下发报文（5 字节，首字节是 report id）：
//!
//! ```text
//! [0x0d, 0x14, 0x03, <level>, 0xad]
//!   id    固定   固定    亮度     固定
//! ```
//!
//! `<level>` 语义：
//!   `0x01` 点亮、`0x00` 熄灭、`0x41..=0x48` 对应亮度 1..8 档。
//!
//! 写入方式必须是 **`HIDIOCSFEATURE`（SET_REPORT / Feature）**。
//! 早前用 `write()` 走的是 OUTPUT 报告（report id 0x14，键盘 LED 通道），
//! 内核照样返回成功、灯却不动，属于假成功——这是 v3 点不亮灯的根因。
//!
//! ## i2c 通道（官方 ACPI 路径的等效实现，默认不启用）
//!
//! 官方 INF 的 LED 区给出命令字：解锁 `0x60`、亮 `0x01`、灭 `0x00`。
//! 这一组是「先解锁再写」的两段式语义，只在 i2c 直写 EC 时才需要；
//! hidraw FEATURE 通道不需要 `0x60`（见 `LedSink::needs_unlock`）。
//!
//! ## 节点选择
//!
//! 背光节点挂在触摸板 `04F3:3275` 上，**不是** ASUS vendor `0B05`
//! （`0B05:1ACE` 是 OMNI RECEIVER 无线接收器，往里写 FEATURE 报文会打到接收器）。

use std::ffi::CString;
use std::io;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::process::Command;

use crate::config::{Config, VAL_UNLOCK};
use crate::engine::LedSink;

fn open_rw(path: &str) -> io::Result<RawFd> {
    let c =
        CString::new(path).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad path"))?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDWR) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

fn open_ro(path: &str) -> io::Result<RawFd> {
    let c =
        CString::new(path).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad path"))?;
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

/// 直接走 `syscall(SYS_ioctl)`：musl 的 `ioctl()` 请求码是 `int`，
/// 高位请求（`0xC0...`）经它传递有截断歧义，syscall 的 u64 参数则两边一致。
fn do_ioctl(fd: RawFd, req: libc::c_ulong, arg: *mut libc::c_void) -> libc::c_int {
    unsafe { libc::syscall(libc::SYS_ioctl, fd, req, arg) as libc::c_int }
}

/// `_IOWR` / `_IOR` 编码（Linux 全架构通用布局）。
const fn ioc(dir: u64, ty: u64, nr: u64, size: u64) -> libc::c_ulong {
    ((dir << 30) | (size << 16) | (ty << 8) | nr) as libc::c_ulong
}

const IOC_READ: u64 = 2;
const IOC_READ_WRITE: u64 = 3;
const HID_TYPE: u64 = b'H' as u64;

/// `HIDIOCGRDESCSIZE`：读报告描述符长度。
const HIDIOCGRDESCSIZE: libc::c_ulong = ioc(
    IOC_READ,
    HID_TYPE,
    0x01,
    std::mem::size_of::<libc::c_int>() as u64,
);
/// `HIDIOCGRDESC`：读报告描述符本体。
const HIDIOCGRDESC: libc::c_ulong = ioc(
    IOC_READ,
    HID_TYPE,
    0x02,
    std::mem::size_of::<HidrawReportDescriptor>() as u64,
);
/// `HIDIOCSFEATURE(len)`：写 Feature 报告。
const fn hid_iocsfeature(len: u64) -> libc::c_ulong {
    ioc(IOC_READ_WRITE, HID_TYPE, 0x06, len)
}

const HID_MAX_DESCRIPTOR_SIZE: usize = 4096;

#[repr(C)]
struct HidrawReportDescriptor {
    size: u32,
    value: [u8; HID_MAX_DESCRIPTOR_SIZE],
}

// --------------------------------------------------------------------------- //
// hidraw
// --------------------------------------------------------------------------- //

/// 背光 FEATURE 报告 ID（报告描述符里 `85 0d ... b1 02`）。
pub const LED_FEATURE_REPORT_ID: u8 = 0x0d;
/// 报文第 2/3 字节：固定 0x14 / 0x03（对应 usage ff00.00c4 的写法）。
const LED_FEATURE_BODY: [u8; 2] = [0x14, 0x03];
/// 报文末字节：固定 0xad。
const LED_FEATURE_TAIL: u8 = 0xad;
/// FEATURE 报告总长（含 report id）。
const LED_FEATURE_LEN: usize = 5;

/// 把亮度命令字映射成 FEATURE 报文里的 level 字节。
///
/// `0x01` 亮 / `0x00` 灭原样透传；`0x60`（i2c 解锁字）在 hidraw 通道无意义，直接忽略。
pub fn level_byte(value: u8) -> Option<u8> {
    match value {
        VAL_UNLOCK => None,
        v => Some(v),
    }
}

pub struct HidrawLed {
    pub path: String,
    history: Vec<u8>,
}

impl HidrawLed {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            history: Vec::new(),
        }
    }
}

impl LedSink for HidrawLed {
    fn send(&mut self, value: u8) -> bool {
        let Some(level) = level_byte(value) else {
            // 0x60 是 i2c 的解锁字，hidraw FEATURE 通道没有这个前置动作，静默跳过
            return true;
        };
        let mut buf = [
            LED_FEATURE_REPORT_ID,
            LED_FEATURE_BODY[0],
            LED_FEATURE_BODY[1],
            level,
            LED_FEATURE_TAIL,
        ];
        debug_assert_eq!(buf.len(), LED_FEATURE_LEN);

        // 每次写都重新 open/close：驱动占着 hidraw 句柄会让 EC 通路在其他工具里不可用
        let fd = match open_rw(&self.path) {
            Ok(fd) => fd,
            Err(err) => {
                eprintln!(
                    "{}",
                    crate::i18n::tf(
                        "led.warn.open",
                        &[("path", &self.path), ("err", &err.to_string())]
                    )
                );
                return false;
            }
        };
        // 必须走 SET_REPORT / Feature；write() 走的是 OUTPUT 报告，会假成功
        let rc = do_ioctl(
            fd,
            hid_iocsfeature(LED_FEATURE_LEN as u64),
            buf.as_mut_ptr() as *mut libc::c_void,
        );
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        if rc < 0 {
            eprintln!(
                "{}",
                crate::i18n::tf(
                    "led.warn.feature",
                    &[
                        ("path", &self.path),
                        ("rid", &format!("{LED_FEATURE_REPORT_ID:02x}")),
                        ("level", &format!("{level:02x}")),
                        ("err", &err.to_string()),
                    ]
                )
            );
            return false;
        }
        self.history.push(value);
        true
    }

    fn channel(&self) -> String {
        format!(
            "hidraw:{}（FEATURE 0x{LED_FEATURE_REPORT_ID:02x}）",
            self.path
        )
    }

    fn history(&self) -> Vec<u8> {
        self.history.clone()
    }

    fn needs_unlock(&self) -> bool {
        // FEATURE 通道一步到位，不需要先发 0x60
        false
    }
}

// --------------------------------------------------------------------------- //
// i2c（通过 i2ctransfer 命令行）
// --------------------------------------------------------------------------- //

pub struct I2cLed {
    pub tool: String,
    pub bus: i32,
    pub addr: u8,
    /// 与解锁字节一起写入的载荷前缀，默认 `0x00`
    pub payload: String,
    history: Vec<u8>,
}

impl I2cLed {
    pub fn new(tool: &str, bus: i32, addr: u8, payload: String) -> Self {
        Self {
            tool: tool.to_string(),
            bus,
            addr,
            payload,
            history: Vec::new(),
        }
    }

    fn write_byte(&mut self, value: u8) -> bool {
        let out = Command::new(&self.tool)
            .arg("-f")
            .arg("-y")
            .arg(self.bus.to_string())
            .arg(format!("w2@0x{:02x}", self.addr))
            .arg(&self.payload)
            .arg(format!("0x{value:02x}"))
            .output();
        match out {
            Ok(o) if o.status.success() => {
                self.history.push(value);
                true
            }
            Ok(o) => {
                eprintln!(
                    "{}",
                    crate::i18n::tf(
                        "led.warn.tool",
                        &[
                            ("tool", &self.tool),
                            ("value", &format!("{value:02x}")),
                            ("err", String::from_utf8_lossy(&o.stderr).trim()),
                        ]
                    )
                );
                false
            }
            Err(err) => {
                eprintln!(
                    "{}",
                    crate::i18n::tf(
                        "led.warn.exec",
                        &[("tool", &self.tool), ("err", &err.to_string())]
                    )
                );
                false
            }
        }
    }
}

impl LedSink for I2cLed {
    fn send(&mut self, value: u8) -> bool {
        // 官方两态点灯是「先解锁 0x60 再 0x01/0x00」，与上游一致逐字节写
        self.write_byte(value)
    }

    fn channel(&self) -> String {
        format!(
            "i2c:bus={} addr=0x{:02x} via {}",
            self.bus, self.addr, self.tool
        )
    }

    fn history(&self) -> Vec<u8> {
        self.history.clone()
    }
}

// --------------------------------------------------------------------------- //
// 空实现
// --------------------------------------------------------------------------- //

pub struct NullLed {
    reason: String,
    history: Vec<u8>,
}

impl NullLed {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: reason.to_string(),
            history: Vec::new(),
        }
    }
}

impl LedSink for NullLed {
    fn send(&mut self, value: u8) -> bool {
        // 返回 true：让状态机认为点灯成功，避免 `--led none` 时 lit 永远 false 反复重试
        self.history.push(value);
        true
    }

    fn channel(&self) -> String {
        format!("none（{}）", self.reason)
    }

    fn history(&self) -> Vec<u8> {
        self.history.clone()
    }
}

// --------------------------------------------------------------------------- //
// 探测
// --------------------------------------------------------------------------- //

use crate::led_policy::OMNI_PID;
/// ELAN / ASUS vendor 与候选打分统一由跨平台模块 [`crate::led_policy`] 提供，
/// 避免 Linux 侧与测试侧出现两套逻辑。
pub use crate::led_policy::{
    candidate_rank, pick_best, presence_from, NumpadPresence, NumpadProbe, ASUS_VID, ELAN_VID,
    KNOWN_NUMPAD_PIDS, PIXART_VID, RANK_WARN_ASUS_FALLBACK, RANK_WARN_NO_FEATURE,
};
/// OMNI RECEIVER 无线接收器 vendor。
const OMNI_VID: u32 = ASUS_VID;

/// 解析 `/sys/class/hidraw/hidrawN/device/uevent` 里的 `HID_ID=bus:vid:pid`。
fn hid_id(entry: &Path) -> Option<(u32, u32)> {
    let text = std::fs::read_to_string(entry.join("device/uevent")).ok()?;
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("HID_ID=") else {
            continue;
        };
        let mut it = rest.trim().split(':');
        let _bus = it.next()?;
        let vid = u32::from_str_radix(it.next()?, 16).ok()?;
        let pid = u32::from_str_radix(it.next()?, 16).ok()?;
        return Some((vid, pid));
    }
    None
}

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

/// 读报告描述符，判断该节点是否真的带 FEATURE 报告 `report_id`。
///
/// 触摸板一次会导出多个 hidraw 接口，只有带 `0x0d` Feature 的那个能点灯；
/// 选错接口时 ioctl 会返回 EINVAL/ENOENT，所以这里先按描述符筛一遍。
///
/// 返回值区分「有 / 确实没有 / 打不开」三种情况：权限不足被打包成「没有」，
/// 会把排查方向带偏到「机型不对」，所以这里不用 bool。
pub fn probe_feature_report(path: &str, report_id: u8) -> FeatureProbe {
    let fd = match open_ro(path) {
        Ok(fd) => fd,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return FeatureProbe::NoAccess,
        Err(_) => return FeatureProbe::Unknown,
    };
    let mut size: libc::c_int = 0;
    let rc = do_ioctl(
        fd,
        HIDIOCGRDESCSIZE,
        &mut size as *mut libc::c_int as *mut libc::c_void,
    );
    if rc < 0 || size <= 0 {
        unsafe { libc::close(fd) };
        return FeatureProbe::Unknown;
    }
    let mut desc = Box::new(HidrawReportDescriptor {
        size: size as u32,
        value: [0u8; HID_MAX_DESCRIPTOR_SIZE],
    });
    let rc = do_ioctl(
        fd,
        HIDIOCGRDESC,
        desc.as_mut() as *mut HidrawReportDescriptor as *mut libc::c_void,
    );
    unsafe { libc::close(fd) };
    if rc < 0 {
        return FeatureProbe::Unknown;
    }
    let len = (desc.size as usize).min(HID_MAX_DESCRIPTOR_SIZE);
    let data = &desc.value[..len];
    let mut i = 0usize;
    let mut in_target = false;
    while i < len {
        let item = data[i];
        if item == 0x85 {
            // Report ID 主项（1 字节数据）
            in_target = data.get(i + 1).copied() == Some(report_id);
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

/// 一个背光候选节点的探测结果（`asus-numpad check` 会逐条列出）。
#[derive(Debug, Clone)]
pub struct LedCandidate {
    pub dev: String,
    pub vid: u32,
    pub pid: u32,
    pub has_feature: bool,
    /// 描述符探测细粒度结果：`NoAccess` 表示没权限看，而不是没有该 FEATURE。
    pub probe: FeatureProbe,
    /// 见 [`crate::led_policy::candidate_rank`]，越小越优先。
    pub rank: u8,
}

impl LedCandidate {
    /// 是否会被 [`find_hidraw`] 选中（即当前候选集里的最优项）。
    pub fn is_selected(&self, all: &[LedCandidate]) -> bool {
        all.iter()
            .min_by_key(|c| c.rank)
            .map(|best| best.dev == self.dev)
            .unwrap_or(false)
    }
}

/// 列出全部 hidraw 候选并打分（跨机型自检用）。
///
/// 与 [`find_hidraw`] 使用同一套判定，只是不做短路返回，便于在别的机型上
/// 一眼看出“到底哪个节点带 FEATURE 0x0d、驱动会选谁”。
pub fn probe_candidates() -> Vec<LedCandidate> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/hidraw") else {
        return out;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let dev = format!("/dev/{name}");
        if !Path::new(&dev).exists() {
            continue;
        }
        let Some((vid, pid)) = hid_id(&entry.path()) else {
            continue;
        };
        if vid != ELAN_VID && vid != PIXART_VID && vid != ASUS_VID {
            continue;
        }
        if vid == OMNI_VID && pid == OMNI_PID {
            // 无线接收器不参与打分，但仍列出（rank 用 u8::MAX 标记）
            out.push(LedCandidate {
                dev,
                vid,
                pid,
                has_feature: false,
                probe: FeatureProbe::Missing,
                rank: u8::MAX,
            });
            continue;
        }
        let probe = probe_feature_report(&dev, LED_FEATURE_REPORT_ID);
        let has_feature = probe.has();
        let Some(rank) = candidate_rank(vid, pid, has_feature) else {
            continue;
        };
        out.push(LedCandidate {
            dev,
            vid,
            pid,
            has_feature,
            probe,
            rank,
        });
    }
    out.sort_by_key(|c| c.rank);
    out
}

/// 扫描 `/sys/class/hidraw`，判断本机有没有 NumberPad 硬件。
///
/// 与 [`find_hidraw`] 同一套粗筛口径，区别只在于这里**不需要**选出可用节点，
/// 只要有一个节点带 FEATURE `0x0d` 就算有硬件。权限不足单独归为
/// [`NumpadPresence::NoAccess`]——那种情况不能当成「没有这块硬件」。
pub fn probe_numpad_presence() -> NumpadPresence {
    let Ok(rd) = std::fs::read_dir("/sys/class/hidraw") else {
        return NumpadPresence::Unknown;
    };
    let mut probes: Vec<NumpadProbe> = Vec::new();
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let dev = format!("/dev/{name}");
        if !Path::new(&dev).exists() {
            continue;
        }
        let Some((vid, pid)) = hid_id(&entry.path()) else {
            continue;
        };
        if vid != ELAN_VID && vid != PIXART_VID && vid != ASUS_VID {
            continue;
        }
        if vid == OMNI_VID && pid == OMNI_PID {
            continue;
        }
        probes.push(match probe_feature_report(&dev, LED_FEATURE_REPORT_ID) {
            FeatureProbe::Has => NumpadProbe::Has,
            FeatureProbe::Missing => NumpadProbe::Missing,
            FeatureProbe::NoAccess => NumpadProbe::NoAccess,
            FeatureProbe::Unknown => NumpadProbe::Unknown,
        });
    }
    presence_from(&probes)
}

/// 背光 i2c 从地址探测（0x38 优先，其次 0x15），供 self-check 使用。
pub fn probe_i2c() -> Option<(i32, u8)> {
    find_i2c()
}

/// 在 /sys/class/hidraw 下锁定触摸板的背光 FEATURE 节点。
///
/// **跨机型通用策略**：不再绑定某个固定 `vid:pid`，而是按
/// [`candidate_rank`] 打分——只要节点是 ELAN(`04f3`) / PixArt(`093a`) 或 ASUS(`0b05`，排除
/// OMNI RECEIVER `0B05:1ACE`)，且报告描述符里确有 FEATURE `0x0d`，就认为命中。
/// 不同机型触摸板 PID 各不相同（G634JY=3275，其它机型可能是别的值），
/// 只有描述符特征才是稳定的判据。
pub fn find_hidraw() -> Option<String> {
    let rd = std::fs::read_dir("/sys/class/hidraw").ok()?;
    // 先收集候选：(dev, vid, pid, has_feature)，打分交给 led_policy::pick_best
    let mut cands: Vec<(u32, u32, bool)> = Vec::new();
    let mut devs: Vec<String> = Vec::new();
    let mut probes: Vec<FeatureProbe> = Vec::new();

    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let dev = format!("/dev/{name}");
        if !Path::new(&dev).exists() {
            continue;
        }
        let Some((vid, pid)) = hid_id(&entry.path()) else {
            continue;
        };

        // 先按 vid 粗筛，避免对无关节点做 ioctl
        if vid != ELAN_VID && vid != PIXART_VID && vid != ASUS_VID {
            continue;
        }
        if vid == OMNI_VID && pid == OMNI_PID {
            continue;
        }
        let probe = probe_feature_report(&dev, LED_FEATURE_REPORT_ID);
        let has_feature = probe.has();
        if candidate_rank(vid, pid, has_feature).is_none() {
            continue;
        }
        cands.push((vid, pid, has_feature));
        devs.push(dev);
        probes.push(probe);
    }

    let idx = pick_best(&cands)?;
    let (vid, pid, _) = cands[idx];
    let dev = devs[idx].clone();
    let probe = probes[idx];
    let rank = candidate_rank(vid, pid, cands[idx].2).unwrap_or(RANK_WARN_ASUS_FALLBACK);

    if rank >= RANK_WARN_NO_FEATURE {
        // 区分「没权限看」与「确实没有」：前者要查 udev，别去怀疑硬件。
        if probe == FeatureProbe::NoAccess {
            eprintln!(
                "{}",
                crate::i18n::tf(
                    "led.warn.no_access",
                    &[
                        ("dev", &dev),
                        ("rid", &format!("{LED_FEATURE_REPORT_ID:02x}")),
                    ]
                )
            );
        } else {
            eprintln!(
                "{}",
                crate::i18n::tf(
                    "led.warn.no_feature",
                    &[
                        ("dev", &dev),
                        ("vid", &format!("{vid:04x}")),
                        ("pid", &format!("{pid:04x}")),
                        ("rid", &format!("{LED_FEATURE_REPORT_ID:02x}")),
                    ]
                )
            );
        }
    }
    if rank >= RANK_WARN_ASUS_FALLBACK {
        eprintln!(
            "{}",
            crate::i18n::tf(
                "led.warn.fallback_asus",
                &[
                    ("vid", &format!("{vid:04x}")),
                    ("pid", &format!("{pid:04x}"))
                ]
            )
        );
    }
    Some(dev)
}

/// 在 /sys/bus/i2c/devices 下找背光从设备地址（0x38 或 0x15），并解析其总线号。
///
/// 上游 asus-numberpad-driver 两种地址都用过：0x38 为主，部分机型落在 0x15。
pub fn find_i2c() -> Option<(i32, u8)> {
    let rd = std::fs::read_dir("/sys/bus/i2c/devices").ok()?;
    let mut hit_38: Option<(i32, u8)> = None;
    let mut hit_15: Option<(i32, u8)> = None;
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // 形如 i2c-3-0038
        if let Some((bus_part, addr_part)) = name.rsplit_once('-') {
            let Ok(addr) = u8::from_str_radix(addr_part, 16) else {
                continue;
            };
            let bus = bus_part
                .trim_start_matches("i2c-")
                .parse::<i32>()
                .unwrap_or(0);
            if addr == 0x38 {
                hit_38.get_or_insert((bus, addr));
            } else if addr == 0x15 {
                hit_15.get_or_insert((bus, addr));
            }
        }
    }
    hit_38.or(hit_15)
}

/// 按配置选择通道。
pub fn build_led(cfg: &Config) -> Box<dyn LedSink> {
    match cfg.led_kind.as_str() {
        "none" => Box::new(NullLed::new(&crate::i18n::t("led.none.explicit"))),
        "hidraw" => {
            let path = cfg.hidraw.clone().or_else(find_hidraw);
            match path {
                Some(p) => Box::new(HidrawLed::new(&p)),
                None => Box::new(NullLed::new(&crate::i18n::t("led.none.no_hidraw"))),
            }
        }
        "i2c" => {
            let (bus, addr) = match (cfg.i2c_bus, cfg.i2c_addr) {
                (Some(b), Some(a)) => (b, a),
                _ => find_i2c().unwrap_or((0, 0x38)),
            };
            if bus == 0 {
                return Box::new(NullLed::new(&crate::i18n::t("led.none.no_i2c")));
            }
            Box::new(I2cLed::new(
                &cfg.i2c_transfer,
                bus,
                addr,
                cfg.i2c_payload.clone(),
            ))
        }
        other => {
            // auto 或未知值：先 hidraw 再 i2c
            if other != "auto" {
                eprintln!("{}", crate::i18n::tf("led.warn.bad_kind", &[("v", other)]));
            }
            if let Some(p) = cfg.hidraw.clone().or_else(find_hidraw) {
                return Box::new(HidrawLed::new(&p));
            }
            if let Some((bus, addr)) = find_i2c() {
                return Box::new(I2cLed::new(
                    &cfg.i2c_transfer,
                    bus,
                    addr,
                    cfg.i2c_payload.clone(),
                ));
            }
            Box::new(NullLed::new(&crate::i18n::t("led.none.any")))
        }
    }
}
