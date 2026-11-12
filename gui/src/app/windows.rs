//! Windows 侧的界面：只做灯光。
//!
//! Windows 上键位映射、手势、NumLock 行为全部由 ASUS 官方驱动负责，我们一个字节都不碰
//! FEATURE 0x0d 之外的报告——官方驱动保持原有的手感和行为，这里只补它没做的两件事：
//! 把亮度调到想要的档、在 NumLock 关闭时把灯灭掉。
//!
//! 所以这一侧没有 Linux 那套驱动配置面板：那套参数在 Windows 上属于官方驱动的注册表，
//! 改了只会让两边行为对不上。

use eframe::egui;

use super::{small, Action, App};
use crate::theme;

impl App {
    pub(super) fn draw_lighting_note(&mut self, ui: &mut egui::Ui, _action: &mut Option<Action>) {
        let p = self.palette;
        theme::section(ui, &p, "说明", |ui| {
            ui.label(small(
            "Windows 版只负责灯光：键位、手势、NumLock 联动逻辑沿用 ASUS 官方驱动，界面不改动、不接管输入。",
        ));
            ui.label(small(
            "下发的是与 Linux 侧同一份 FEATURE 0x0d 报文（[0d 14 03 <档位> ad]），亮度档位映射也同源，所以两边亮度看起来一致。",
        ));
            ui.label(small(
            "需要调机型、布局、阈值这些驱动参数时，请用 Linux 版或命令行：Windows 上这些由官方驱动的注册表项决定。",
        ));
        });
    }
}
