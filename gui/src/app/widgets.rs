//! 页面共用的小零件与纯函数工具。
//!
//! 只有「多个页面都要、且与 App 状态无关」的东西才住在这里：选中态的按钮、组合框、
//! 表单行、十六进制表渲染，以及时间戳/路径这类纯函数。任何需要读 `App` 字段的
//! 逻辑都留在各自页面模块里，别往这里塞。

use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;

use crate::theme;

/// 是否画 Linux 全套面板。
///
/// 默认按平台走：Linux 全套、Windows 只灯光。环境变量 `NUMDECK_GUI_FULL=1/0`
/// 可以强制覆盖，用于在另一平台上预览对面那套界面（真机默认行为不受影响）。
pub fn ui_full() -> bool {
    let Ok(value) = std::env::var("NUMDECK_GUI_FULL") else {
        return cfg!(target_os = "linux");
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "full" | "yes" => true,
        "0" | "off" | "false" | "lite" | "no" => false,
        _ => cfg!(target_os = "linux"),
    }
}

pub fn window_title() -> &'static str {
    if ui_full() {
        "ASUS NumberPad 控制中心"
    } else {
        "ASUS NumberPad 灯光控制"
    }
}

pub(in crate::app) fn small(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into()).small().weak()
}

/// 档位表展开成 `0x00 0x43 0x45 0x48`，用于把配置里的表直接摊在界面上。
pub(in crate::app) fn hex_table(table: &[u8]) -> String {
    table
        .iter()
        .map(|value| format!("0x{value:02x}"))
        .collect::<Vec<_>>()
        .join("  ")
}

/// 下拉框：`items` 里第一项通常就是默认值，找不到匹配时显示空串而不是 panic。
pub(in crate::app) fn combo<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    items: &[(T, &'static str)],
) {
    let current = items
        .iter()
        .find(|(candidate, _)| candidate == value)
        .map(|(_, label)| *label)
        .unwrap_or("");
    egui::ComboBox::from_id_salt(id)
        .width(crate::theme::ctrl_width())
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (candidate, label) in items {
                ui.selectable_value(value, *candidate, *label);
            }
        });
}

/// 一行「标签 + 输入框」。空串一律视为「不传这个参数」，由 cmdform 判空。
///
/// 标签走统一的左列、输入框从控件列起，`help` 挂在标签右侧的 `i` 上悬浮显示。
pub(in crate::app) fn field(
    ui: &mut egui::Ui,
    p: &crate::theme::Palette,
    label: &str,
    value: &mut String,
    hint: &str,
    help: Option<&str>,
) -> egui::Response {
    theme::row(ui, p, label, help, |ui| theme::text_field(ui, value, hint))
}

/// 系统状态每一行的悬浮说明：字段名对用户不直观，停一下鼠标给出含义与排查方向。
pub(in crate::app) fn state_help(name: &str) -> Option<&'static str> {
    Some(match name {
        "平台" => "当前跑在哪个平台，以及数字键与手势由谁处理。",
        "配置文件" => "驱动读取的配置文件路径；发现改不生效时先看这里指的是哪一份。",
        "机型" => "驱动识别出的华硕机型 ID，它决定了默认布局与键位。",
        "布局" => "数字键区当前使用的键位布局；认错机型时这里通常也跟着错。",
        "背光通道" => "灯不亮先看这一行：能否找到带 FEATURE 0x0d 的 HID 接口。",
        "数字键 / 手势" => "数字键与手势事件的实现方；Linux 下应为本程序的后台服务。",
        _ => return None,
    })
}

/// 日志时间戳（UTC，`HH:MM:SS`）。只用于看报文先后，不做展示级时区换算。
pub(in crate::app) fn now_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// 取一段文本的末尾若干行：`(是否被裁剪, 裁剪后的文本, 原始行数)`。
///
/// 按 `\n` 切，保留原行序；尾部为空行也照旧留着，免得把命令最后的空行吃掉。
pub(in crate::app) fn tail_lines(text: &str, max: usize) -> (bool, String, usize) {
    let total = text.lines().count();
    if total <= max {
        return (false, text.to_string(), total);
    }
    let start = total - max;
    let tail: Vec<&str> = text.lines().skip(start).collect();
    (true, tail.join("\n"), total)
}

/// 用系统文件管理器打开一个目录。只读操作，失败就如实报出来。
pub(in crate::app) fn open_dir(dir: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    let opener = "xdg-open";
    #[cfg(target_os = "macos")]
    let opener = "open";
    #[cfg(windows)]
    let opener = "explorer";
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    let opener = "xdg-open";
    std::process::Command::new(opener)
        .arg(dir)
        .spawn()
        .map(|_| ())
}

/// 诊断报告文件名用的时间戳（Unix 秒，够排序了）。
pub(in crate::app) fn stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod output_tail_tests {
    use super::tail_lines;

    #[test]
    fn short_output_untouched() {
        let text = "第一行\n第二行";
        let (cut, body, total) = tail_lines(text, 10);
        assert!(!cut);
        assert_eq!(body, text);
        assert_eq!(total, 2);
    }

    #[test]
    fn keeps_tail_in_order() {
        let text = "a\nb\nc\nd\ne";
        let (cut, body, total) = tail_lines(text, 3);
        assert!(cut);
        assert_eq!(body, "c\nd\ne");
        assert_eq!(total, 5);
    }

    #[test]
    fn exact_fit_not_cut() {
        let text = "1\n2\n3";
        let (cut, body, total) = tail_lines(text, 3);
        assert!(!cut);
        assert_eq!(body, text);
        assert_eq!(total, 3);
    }

    #[test]
    fn keeps_trailing_blank_line() {
        let text = "x\ny\n";
        let (cut, body, total) = tail_lines(text, 5);
        assert!(!cut);
        assert_eq!(body, "x\ny\n");
        assert_eq!(total, 2);
    }
}
