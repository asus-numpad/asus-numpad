//! Linux 全套控制面板。
//!
//! 这张面板把驱动配置文件里的每一项都做成控件：机型、布局、键码风格、NumLock 联动、
//! 混合模式、手势阈值、背光通道、跟灯参数、时序参数、语言。改完点保存即落盘，
//! 「保存并重启服务」再多做一步 `systemctl --user restart`，让后台服务读到新配置。
//!
//! 做法是直接读改写 [`numdeck::config::Config`] 的 JSON，而不是拼命令行参数：
//! 命令行只覆盖了其中一部分（`sku` / `layout` / `lang` / `bright`），而阈值与通道这
//! 类参数只有配置文件一条路；界面里两者落在同一份文件上，不会出现「界面一套、CLI 一套」。

use eframe::egui;

use numdeck::config::{CalcMode, MixedMode, NumLockMode};
use numdeck::custom;
use numdeck::layout::{KeyMode, LayoutSupport, LAYOUTS};
use numdeck::sku::{self, SKUS};

use super::{field, small, Action, App, WARN};
use crate::theme;

mod behavior;
mod channel;
mod gestures;
mod identity;
mod labels;
mod levels;
mod tail;
mod timing;

use labels::{lang_label, mixed_label, numlock_label, source_label, support_label};
/// 机型下拉里「自动识别」的取值（配置里 `sku: null`）。
const SKU_AUTO: &str = "";

impl App {
    /// Linux 侧的全部调节项。画完统一比对一次，有差异就标脏。
    pub(super) fn draw_full_controls(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.draw_cfg_header(ui, action);
        ui.add_space(10.0);
        self.draw_identity(ui);
        ui.add_space(10.0);
        self.draw_behavior(ui);
        ui.add_space(10.0);
        self.draw_channel(ui, action);
        ui.add_space(10.0);
        self.draw_gestures(ui);
        ui.add_space(10.0);
        self.draw_timing(ui);
        ui.add_space(10.0);
        self.draw_language(ui);
        ui.add_space(10.0);
        self.draw_cfg_json(ui);
        ui.add_space(10.0);
        self.draw_cfg_actions(ui, action);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    /// 驱动配置页的抬头：写在哪、有没有改动、以及四个落盘动作。
    ///
    /// 四个按钮按「动词轻重」排：主按钮是保存并重启（改完参数的常规动作），
    /// 只保存和重新载入是次级，恢复默认放最后且用次级样式，避免误点。
    fn draw_cfg_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let dirty = self.driver.dirty;
        let write_path = self.driver.write_path.display().to_string();
        let read_path = self.driver.path.display().to_string();
        let same_path = self.driver.path == self.driver.write_path;
        let existed = self.driver.existed;
        let notes = self.driver.notes.clone();

        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row(ui, &p, "驱动配置");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dirty {
                        theme::pill(ui, "有改动未保存", p.warn)
                            .on_hover_text("点「保存」或「保存并重启服务」落盘");
                    } else {
                        theme::pill(ui, "已与磁盘一致", p.ok);
                    }
                });
            });
            ui.add_space(2.0);
            theme::note(ui, &p, format!("写入：{write_path}"));
            if !same_path {
                theme::note(ui, &p, format!("读取自：{read_path}"));
            }
            if !existed {
                theme::note(ui, &p, "磁盘上还没有配置文件，保存后按官方默认值创建。");
            }
            for note in notes {
                ui.label(egui::RichText::new(note).small().color(p.warn));
            }

            ui.add_space(8.0);
            // 四颗按钮各自统一宽度，从控件列起排；右边缘不会因为文字长短而参差
            let picked = theme::action_row(
                ui,
                &p,
                "写入",
                Some("保存＝只落盘；保存并重启＝写回配置文件后 systemctl --user restart。"),
                &[
                    (
                        "保存并重启",
                        true,
                        "写回配置文件并 systemctl --user restart，改机型/布局/阈值后必须走这个。",
                    ),
                    (
                        "保存",
                        false,
                        "只落盘，不重启后台服务；正在用的触摸板不中断。",
                    ),
                    (
                        "重新载入",
                        false,
                        "丢掉界面上的改动，重新从磁盘读一遍配置。",
                    ),
                    (
                        "恢复默认",
                        false,
                        "把驱动配置改回官方默认值；需要再保存才落盘。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SaveDriverAndRestart),
                Some(1) => *action = Some(Action::SaveDriver),
                Some(2) => *action = Some(Action::ReloadDriver),
                Some(3) => *action = Some(Action::ResetDriver),
                _ => {}
            }
            theme::note(
                ui,
                &p,
                "只点保存不会打断正在用的触摸板；改机型/布局/阈值这类参数要点「保存并重启服务」才生效。",
            );
        });
    }
}
