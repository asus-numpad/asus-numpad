//! 全局热键：组合键输入与下拉选择。

use eframe::egui;

use numdeck::custom::{Hotkey, HotkeyAction, HotkeySpec};

use super::WARN;
use crate::theme;

/// 组合键的比对键：能解析就用规整写法，解析不出来退回去按原文小写比。
///
/// 判断「两条热键是不是同一个组合键」不能直接比字符串——`ctrl+alt+n` 与
/// `Alt+Ctrl+N` 在驱动里是同一个，界面上却是两串不同的字。
pub(crate) fn hotkey_key(gesture: &str) -> String {
    match Hotkey::parse(gesture) {
        Ok(hotkey) => hotkey.label(),
        Err(_) => gesture.trim().to_ascii_lowercase(),
    }
}

/// 热键动作的界面文案。
pub(crate) fn action_label(action: HotkeyAction) -> &'static str {
    match action {
        HotkeyAction::Toggle => "切换开关",
        HotkeyAction::On => "强制打开",
        HotkeyAction::Off => "强制关闭",
    }
}

/// 组合键校验：写得对就显示规整写法，写得不对就把驱动会报的错原样摆出来。
pub(crate) fn draw_hotkey_check(ui: &mut egui::Ui, p: &theme::Palette, spec: &mut HotkeySpec) {
    if spec.gesture.trim().is_empty() {
        ui.label(
            egui::RichText::new("未填组合键，这条不会生效")
                .size(12.0)
                .color(WARN),
        );
        return;
    }
    match Hotkey::parse(&spec.gesture) {
        Ok(hotkey) => {
            let canonical = hotkey.label();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("识别为 {canonical}"))
                        .size(12.0)
                        .color(p.ok),
                );
                if canonical != spec.gesture.trim() {
                    let resp = theme::secondary(ui, p, "规整写法");
                    let resp = theme::tip(
                        resp,
                        "把 `Ctrl+Alt+N`、`ctrl + alt + n` 这类写法统一成配置里的规范形式。",
                    );
                    if resp.clicked() {
                        spec.gesture = canonical;
                    }
                }
            });
        }
        Err(err) => {
            ui.label(
                egui::RichText::new(format!("解析失败：{err}"))
                    .size(12.0)
                    .color(WARN),
            );
        }
    }
}
