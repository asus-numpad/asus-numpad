//! 平台状态：后台服务、机型与布局识别、NumberPad 背光通道是否存在。
//!
//! 界面上这些只读信息的作用是排障：灯不亮时先看这一区，能分清是「选错接口」「没权限」
//! 还是「这台机器本来就没有背光通道」，不用让用户回去敲命令行。

/// 平台快照。界面按行渲染 `rows`，`hints` 是配置里冒出来的问题，`service` 只有
/// Linux 才有。
pub struct Snapshot {
    /// 头部副标题：这台机器上谁负责数字键与手势
    pub subtitle: String,
    /// 只读信息行（标题, 值）
    pub rows: Vec<(String, String)>,
    /// 配置加载时产生的提示（字段非法、被夹回默认值之类）
    pub hints: Vec<String>,
    /// systemd 用户单元状态
    pub service: Option<ServiceState>,
}

/// 后台服务状态。
pub struct ServiceState {
    /// `systemctl --user is-active` 的结果，如 `active` / `inactive`
    pub active: String,
    /// `systemctl --user is-enabled` 的结果，如 `enabled` / `disabled`
    pub enabled: String,
}

impl ServiceState {
    pub fn is_active(&self) -> bool {
        self.active == "active"
    }
}

/// 界面里能操作的动作。
#[derive(Clone, Copy)]
pub enum ServiceVerb {
    Start,
    Stop,
    Restart,
}

/// 该平台在界面里是否提供后台服务管理。
pub fn has_service() -> bool {
    cfg!(target_os = "linux")
}

#[cfg(not(target_os = "linux"))]
pub fn snapshot() -> Snapshot {
    Snapshot {
        subtitle: "数字键与手势由 ASUS 官方驱动处理，本程序只写背光 FEATURE 报文".to_string(),
        rows: vec![
            (
                "平台".to_string(),
                if cfg!(windows) {
                    "Windows".to_string()
                } else {
                    "非 Windows（模拟后端，仅用于预览界面）".to_string()
                },
            ),
            (
                "数字键 / 手势".to_string(),
                "ASUS 官方驱动（AsusNUMPADFilter）".to_string(),
            ),
            (
                "背光通道".to_string(),
                "HID FEATURE 0x0d（setupapi + hid.dll）".to_string(),
            ),
        ],
        hints: Vec::new(),
        service: None,
    }
}

#[cfg(not(target_os = "linux"))]
pub fn service_run(_verb: ServiceVerb) -> anyhow::Result<String> {
    anyhow::bail!("当前平台没有可管理的后台服务")
}

#[cfg(not(target_os = "linux"))]
pub fn service_autostart(_on: bool) -> anyhow::Result<String> {
    anyhow::bail!("当前平台没有可管理的后台服务")
}

#[cfg(target_os = "linux")]
pub use linux_impl::{service_autostart, service_run, snapshot};

#[cfg(target_os = "linux")]
mod linux_impl {
    use anyhow::{anyhow, Result};

    use numdeck::config::Config;
    use numdeck::layout;
    use numdeck::linux::{probe_numpad_presence, resolve_layout, NumpadPresence};
    use numdeck::sku::{self, SkuSource};

    use super::{ServiceState, ServiceVerb, Snapshot};

    /// 用户单元名，与 `packaging/common/numdeck.service` 一致。
    const UNIT: &str = "numdeck";

    pub fn snapshot() -> Snapshot {
        let path = Config::default_path();
        let (cfg, notes) = Config::load(&path);
        let rows = vec![
            ("配置文件".to_string(), path.display().to_string()),
            ("机型".to_string(), sku_text(&cfg)),
            ("布局".to_string(), layout_text(&cfg)),
            ("背光通道".to_string(), presence_text()),
        ];

        Snapshot {
            subtitle:
                "数字键与手势由 systemd 用户单元 numdeck 处理，本程序只写背光 FEATURE 报文"
                    .to_string(),
            rows,
            hints: notes,
            service: Some(ServiceState {
                active: systemctl("is-active"),
                enabled: systemctl("is-enabled"),
            }),
        }
    }

    fn sku_text(cfg: &Config) -> String {
        match sku::current(cfg.sku.as_deref()) {
            Ok(resolved) => {
                let how = match resolved.source {
                    SkuSource::Manual => "手动指定",
                    SkuSource::Dmi => "DMI 型号命中",
                    SkuSource::Hardware => "触摸板模组命中",
                    // ARM64 构建不带机型表，机型恒为自定义布局
                    SkuSource::Custom => "自定义布局（无机型表）",
                };
                match &resolved.detail {
                    Some(detail) => format!("{}（{how}：{detail}）", resolved.id()),
                    None => format!("{}（{how}）", resolved.id()),
                }
            }
            Err(err) => format!("未识别：{err}"),
        }
    }

    fn layout_text(cfg: &Config) -> String {
        let Some(name) = touchpad_name() else {
            return format!("未找到触摸板 event 节点（配置 layout={}）", cfg.layout_name);
        };
        let resolved = resolve_layout(cfg, &name);
        let how = match resolved.source {
            "fixed" => "配置写死",
            "auto" => "按 HID ID 识别",
            _ => "未识别，回落默认",
        };
        match &resolved.hid {
            Some(hid) => format!("{}（{hid}，{how}）", resolved.entry.name),
            None => format!("{}（{how}）", resolved.entry.name),
        }
    }

    /// 触摸板设备名：从 sysfs 读，不需要打开 event 节点，普通用户可读。
    fn touchpad_name() -> Option<String> {
        let entries = std::fs::read_dir("/sys/class/input").ok()?;
        for entry in entries.flatten() {
            let node = entry.file_name().to_string_lossy().to_string();
            if !node.starts_with("event") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path().join("device/name")) else {
                continue;
            };
            let name = text.trim().to_string();
            // 用布局层的同一个判据认触摸板，避免界面上「认出来的」和驱动用的不是一块
            if layout::hid_id_of(&name).is_some() {
                return Some(name);
            }
        }
        None
    }

    fn presence_text() -> String {
        match probe_numpad_presence() {
            NumpadPresence::Present => "存在（描述符带 FEATURE 0x0d）".to_string(),
            NumpadPresence::Absent => "不存在（同模组的减配档，没有数字区背光）".to_string(),
            NumpadPresence::NoAccess => "读不到（权限不足，检查 udev 规则是否生效）".to_string(),
            NumpadPresence::Unknown => {
                "未扫到候选节点（非 ELAN / PixArt / ASUS 触摸板，或 hidraw 未加载）".to_string()
            }
        }
    }

    fn systemctl(verb: &str) -> String {
        match std::process::Command::new("systemctl")
            .args(["--user", verb, UNIT])
            .output()
        {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if text.is_empty() {
                    String::from_utf8_lossy(&out.stderr).trim().to_string()
                } else {
                    text
                }
            }
            Err(err) => format!("无法执行 systemctl（{err}）"),
        }
    }

    /// 启动 / 停止 / 重启用户单元。
    ///
    /// 用户单元不需要 root，所以这里不用 pkexec——界面里点按钮和在终端敲
    /// `systemctl --user` 是同一条路径。
    pub fn service_run(verb: ServiceVerb) -> Result<String> {
        let arg = match verb {
            ServiceVerb::Start => "start",
            ServiceVerb::Stop => "stop",
            ServiceVerb::Restart => "restart",
        };
        run_systemctl(arg)
    }

    /// 开机自启（`systemctl --user enable/disable`）。
    pub fn service_autostart(on: bool) -> Result<String> {
        run_systemctl(if on { "enable" } else { "disable" })
    }

    fn run_systemctl(arg: &str) -> Result<String> {
        let out = std::process::Command::new("systemctl")
            .args(["--user", arg, UNIT])
            .output()
            .map_err(|err| anyhow!("执行 systemctl 失败：{err}"))?;
        if out.status.success() {
            return Ok(format!("systemctl --user {arg} {UNIT} 已执行"));
        }
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(anyhow!("systemctl --user {arg} {UNIT} 失败：{stderr}"))
    }
}
