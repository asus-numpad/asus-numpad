//! `numdeck-gui`：NumberPad 背光控制的图形界面 + 命令行（Windows / Linux 共用）。
//!
//! 两个平台的界面、档位、报文完全一致，差别只在后端：Windows 走 `setupapi` +
//! `hid.dll`，Linux 走 hidraw 的 `HIDIOCSFEATURE`。
//!
//! 不带参数启动界面；带参数时按命令行执行，便于脚本、开机自启与排障。
//!
//! ```text
//! numdeck-gui                      打开界面
//! numdeck-gui --selftest           只探测接口并打印档位报文，不写硬件
//! numdeck-gui --on                 按内置档位点亮
//! numdeck-gui --off                熄灭
//! numdeck-gui --level 6            点灯并设到第 6 档（也接受 0x45 这类原始值）
//! numdeck-gui --apply-startup      按配置文件下发一次（供开机自启用）
//! ```

mod app;
mod autostart;
mod backend;
mod cli_bridge;
mod cmdform;
mod kbdmap;
mod settings;
mod shot;
mod sysinfo;
mod theme;
mod ui_font;

#[cfg(target_os = "linux")]
mod linux_hid;
#[cfg(not(any(windows, target_os = "linux")))]
mod mock;
#[cfg(windows)]
mod win_hid;

use anyhow::{anyhow, bail, Context, Result};

use numdeck::led_proto;

use crate::backend::{mark_best, Backend, DeviceInfo};

const USAGE: &str = "\
ASUS NumberPad 背光控制（图形界面 + 命令行）

用法：
  numdeck-gui                    打开图形界面
  numdeck-gui --selftest         探测 HID 接口并打印报文，不写硬件
  numdeck-gui --on               点亮（配置文件里的档位）
  numdeck-gui --off              熄灭
  numdeck-gui --level <1-8|0xNN> 点灯并设到指定档
  numdeck-gui --apply-startup    按配置文件下发一次（开机自启调用）
  numdeck-gui --help             显示本说明

说明：
  本程序只写背光 FEATURE 报告 0x0d，不接管任何输入。
  Windows：数字键、手势与触摸板行为由 ASUS 官方驱动负责；
  Linux：数字键与手势由 systemd 用户单元 numdeck 负责。";

fn main() -> Result<()> {
    // GUI 也读 NUMDECK_GUI_* 环境变量，先把更名前的 ASUS_NUMPAD_GUI_* 继承过来
    numdeck::compat::adopt_legacy_env();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        None => {
            run_gui()?;
            0
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            0
        }
        Some("--selftest") => selftest()?,
        Some("--apply-startup") => {
            apply_startup()?;
            0
        }
        Some("--on") => {
            send_value(fallback_level())?;
            0
        }
        Some("--off") => {
            send_value(led_proto::LED_OFF)?;
            0
        }
        Some("--level") => {
            let raw = args
                .get(1)
                .ok_or_else(|| anyhow!("--level 需要一个参数，例如 --level 6"))?;
            send_value(parse_level(raw)?)?;
            0
        }
        Some(other) => {
            eprintln!("未知参数：{other}\n\n{USAGE}");
            2
        }
    };
    std::process::exit(code);
}

/// `--on` 用：按配置档位取原始值，取不到就退回点亮。
fn fallback_level() -> u8 {
    let settings = settings::load();
    backend::level_for_step(settings.steps, settings.step).unwrap_or(led_proto::LED_ON)
}

/// 解析 `6` / `0x45` / `0` 三种写法。
fn parse_level(text: &str) -> Result<u8> {
    let value = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).with_context(|| format!("无法解析档位：{text}"))?
    } else {
        let n: u8 = text
            .parse()
            .with_context(|| format!("无法解析档位：{text}"))?;
        match n {
            0 => led_proto::LED_OFF,
            1..=8 => led_proto::level_value(n).context("档位换算失败")?,
            _ => bail!("档位取值应为 0、1-8 或 0xNN，收到 {n}"),
        }
    };
    Ok(value)
}

/// 挑出灯光目标：优先配置文件里记住的接口，其次按 `led_policy` 的优先级。
fn pick_target(backend: &mut dyn Backend, devices: &mut [DeviceInfo]) -> Option<usize> {
    let _ = backend;
    let saved = settings::load().device_path;
    if let Some(path) = saved {
        if let Some(i) = devices.iter().position(|d| d.path == path && d.usable()) {
            for d in devices.iter_mut() {
                d.selected = false;
            }
            devices[i].selected = true;
            return Some(i);
        }
    }
    mark_best(devices)
}

/// 下发一个原始值；CLI 三个命令共用。
fn send_value(value: u8) -> Result<()> {
    let mut backend = backend::create();
    let mut devices = backend.scan().context("枚举 HID 接口失败")?;
    let Some(index) = pick_target(&mut *backend, &mut devices) else {
        // 两个平台「找不到接口」的原因完全不同，提示分开写，别让人按错的文档去查
        let hint = if cfg!(windows) {
            "请确认 ASUS 官方驱动已安装，且没有其它程序独占设备。"
        } else {
            "请确认触摸板描述符带 FEATURE 0x0d、hidraw 已加载，且 udev 规则已生效。"
        };
        bail!("没有找到可点灯的 NumberPad 接口。{hint}");
    };
    let device = devices[index].clone();
    let frame = led_proto::encode(value).ok_or_else(|| anyhow!("非法档位值：0x{value:02x}"))?;
    backend
        .write(&device, frame)
        .with_context(|| format!("写入 {} 失败", device.vid_pid()))?;
    println!(
        "已向 {}（{}）写入 {}",
        device.vid_pid(),
        device.vendor(),
        led_proto::hex(&frame)
    );
    Ok(())
}

/// 开机自启入口：静默按配置下发一次。
fn apply_startup() -> Result<()> {
    let settings = settings::load();
    if !settings.on {
        return Ok(());
    }
    let level = backend::level_for_step(settings.steps, settings.step)
        .ok_or_else(|| anyhow!("配置里的档位 {} 越界", settings.step))?;
    send_value(level)
}

/// 只读自检：打印探测结果与各档报文，不写硬件。
fn selftest() -> Result<i32> {
    let mut backend = backend::create();
    println!(
        "后端：{}（真实硬件：{}）",
        backend.name(),
        backend.is_real()
    );

    // 平台状态放到自检最前面：灯不亮时先分清是没权限、没接口还是没这块硬件
    let state = sysinfo::snapshot();
    println!("{}", state.subtitle);
    for (name, value) in &state.rows {
        println!("  {name}：{value}");
    }
    for hint in &state.hints {
        println!("  提示：{hint}");
    }
    if let Some(service) = &state.service {
        println!("  后台服务：{} / {}", service.active, service.enabled);
    }

    let mut devices = backend.scan().context("枚举 HID 接口失败")?;
    if devices.is_empty() {
        println!("未枚举到任何 HID 接口。");
        return Ok(1);
    }
    mark_best(&mut devices);
    println!("共 {} 个接口：", devices.len());
    for d in &devices {
        println!(
            "  {:<10} {:<8} feature={:<5} len={:<3} rank={:?} selected={}",
            d.vid_pid(),
            d.vendor(),
            d.has_feature,
            d.feature_len,
            d.rank,
            d.selected
        );
        if !d.product.is_empty() {
            println!("             product={}", d.product);
        }
        println!("             path={}", d.path);
    }

    println!("\n8 档报文：");
    for (i, level) in led_proto::brightness_levels().iter().enumerate() {
        if let Some(frame) = led_proto::encode(*level) {
            println!(
                "  {} 档 0x{:02x} -> {}",
                i + 1,
                level,
                led_proto::hex(&frame)
            );
        }
    }
    println!(
        "\n熄灭报文：{}",
        led_proto::hex(&led_proto::encode(led_proto::LED_OFF).unwrap())
    );
    println!(
        "普通点亮：{}",
        led_proto::hex(&led_proto::encode(led_proto::LED_ON).unwrap())
    );

    for steps in [4u8, 8u8] {
        let table = backend::level_table(steps);
        let text = table
            .iter()
            .map(|v| format!("0x{v:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!("{steps} 档组映射：{text}");
    }

    Ok(if devices.iter().any(|d| d.usable()) {
        0
    } else {
        1
    })
}

fn run_gui() -> Result<()> {
    #[cfg(windows)]
    hide_console();

    // Linux 面板多得多（驱动配置 + 系统状态 + 命令行），窗口开大一点；Windows 只有灯光。
    let (size, min_size) = if app::ui_full() {
        ([1180.0, 900.0], [960.0, 660.0])
    } else {
        ([980.0, 780.0], [820.0, 620.0])
    };
    // `NUMDECK_GUI_SIZE=1180x1400` 覆盖初始窗口尺寸：文档截图要把整页拍全，
    // 而窗口一旦被系统记住大小（eframe 的持久化），改代码里的默认值就没用了。
    let size = std::env::var("NUMDECK_GUI_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some([w.trim().parse().ok()?, h.trim().parse().ok()?])
        })
        .unwrap_or(size);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(size)
            .with_min_inner_size(min_size)
            .with_title(app::window_title())
            // 与官网 favicon 同一枚标记：窗口标题栏、任务栏、Dock 都有图标，
            // 不再是一枚通用占位方块。
            .with_icon(crate::theme::app_icon(128)),
        ..Default::default()
    };
    eframe::run_native(
        "numdeck-gui",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
    .map_err(|err| anyhow!("启动界面失败：{err}"))
}

/// GUI 模式隐藏控制台窗口；CLI 模式保留，方便看输出。
#[cfg(windows)]
fn hide_console() {
    use windows::Win32::System::Console::GetConsoleWindow;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};

    unsafe {
        let console = GetConsoleWindow();
        if !console.is_invalid() {
            let _ = ShowWindow(console, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn parse_level_accepts_number_and_hex() {
        assert_eq!(parse_level("0").unwrap(), led_proto::LED_OFF);
        assert_eq!(parse_level("1").unwrap(), 0x41);
        assert_eq!(parse_level("8").unwrap(), 0x48);
        assert_eq!(parse_level("0x45").unwrap(), 0x45);
        assert_eq!(parse_level("0X41").unwrap(), 0x41);
    }

    #[test]
    fn parse_level_rejects_out_of_range() {
        assert!(parse_level("9").is_err());
        assert!(parse_level("abc").is_err());
        assert!(parse_level("0xzz").is_err());
    }

    #[test]
    fn step_mapping_matches_linux_defaults() {
        // 4 档组的默认值与 Linux 侧 linux/led.rs 的 manual_brightness_levels 一致
        let four = backend::level_table(4);
        assert_eq!(four, vec![0x00, 0x43, 0x45, 0x48]);
        let eight = backend::level_table(8);
        assert_eq!(eight.len(), 8);
        assert_eq!(eight[0], 0x00);
        assert_eq!(*eight.last().unwrap(), 0x48);
    }

    #[test]
    fn settings_default_is_usable() {
        let s = Settings::default();
        assert!(backend::level_for_step(s.steps, s.step).is_some());
    }
}
