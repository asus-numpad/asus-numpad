//! 页尾：界面语言、配置 JSON 预览与保存/重置动作。

use super::*;

impl App {
    pub(super) fn draw_language(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("驱动语言")
                .default_open(false)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        theme::label_cell(
                            ui,
                            &p,
                            "语言",
                            Some("界面与日志的语言，auto 跟系统语言走。"),
                        );
                        egui::ComboBox::from_id_salt("cfg_lang")
                            .width(theme::ctrl_width())
                            .selected_text(lang_label(&self.driver.cfg.language))
                            .show_ui(ui, |ui| {
                                for code in ["auto", "zh-CN", "zh-TW", "en"] {
                                    ui.selectable_value(
                                        &mut self.driver.cfg.language,
                                        code.to_string(),
                                        lang_label(code),
                                    );
                                }
                            });
                    });
                    theme::note(
                        ui,
                        &p,
                        "驱动与 CLI 的输出文案跟着这一项走，界面自身语言不受影响。",
                    );
                });
        });
    }

    pub(super) fn draw_cfg_json(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("配置文件内容（JSON）")
                .default_open(false)
                .show(ui, |ui| {
                    let text = self.driver.json();
                    ui.horizontal(|ui| {
                        if theme::secondary(ui, &p, "复制").clicked() {
                            ui.ctx().copy_text(text.clone());
                        }
                        theme::note(ui, &p, "与 numdeck --dump-config 输出一致");
                    });
                    let mut shown = text;
                    ui.add(
                        egui::TextEdit::multiline(&mut shown)
                            .code_editor()
                            .desired_rows(14)
                            .desired_width(f32::INFINITY),
                    );
                });
        });
    }

    pub(super) fn draw_cfg_actions(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            ui.add_space(6.0);
            let picked = theme::action_row(
                ui,
                &p,
                "收尾",
                Some("和顶部的两颗保存按钮等价，滚到页底时不用再滚回去。"),
                &[
                    ("保存配置", false, "只落盘，不重启后台服务。"),
                    (
                        "保存并重启",
                        true,
                        "写回配置文件并重启后台服务，让新配置生效。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SaveDriver),
                Some(1) => *action = Some(Action::SaveDriverAndRestart),
                _ => {}
            }
        });
    }
}
