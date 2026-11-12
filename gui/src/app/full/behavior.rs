//! 行为开关：NumLock 联动、混合模式、计算器键位。

use super::*;

impl App {
    pub(super) fn draw_behavior(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("键盘行为")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "NumLock 联动",
                        Some("点数字键区时怎么处理系统 NumLock：sync 先补一次按键、toggle 无条件切换、off 完全不碰。"),
                    );
                    egui::ComboBox::from_id_salt("cfg_numlock")
                        .width(theme::ctrl_width())
                        .selected_text(numlock_label(self.driver.cfg.numlock))
                        .show_ui(ui, |ui| {
                            for (mode, label) in [
                                (NumLockMode::Sync, "sync：关闭时补一次按键再点灯"),
                                (NumLockMode::Toggle, "toggle：无条件切换 NumLock"),
                                (NumLockMode::Off, "off：完全不碰 NumLock"),
                            ] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.numlock,
                                    mode,
                                    label,
                                );
                            }
                        });
                });
                theme::note(
                    ui,
                    &p,
                    "驱动自己不模拟 NumLock 灯，这里决定的是「点数字键区时要不要顺手把系统 NumLock 打开」。",
                );

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "混合模式",
                        Some("混排模式：auto 检测到虚拟触摸板才开，force 总是开，off 就只剩纯数字键盘。"),
                    );
                    egui::ComboBox::from_id_salt("cfg_mixed")
                        .width(theme::ctrl_width())
                        .selected_text(mixed_label(self.driver.cfg.mixed_mode))
                        .show_ui(ui, |ui| {
                            for (mode, label) in [
                                (MixedMode::Auto, "auto：有虚拟触摸板就开（官方默认）"),
                                (MixedMode::On, "on：强制开启"),
                                (MixedMode::Off, "off：纯数字键盘，光标不可用"),
                            ] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.mixed_mode,
                                    mode,
                                    label,
                                );
                            }
                        });
                });
                theme::note(ui, &p, "关闭混合模式后触摸板只出数字、没有光标，等同官方 dwSupport_MixedMode = 0。",);

                ui.add_space(4.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.calc_gesture,
                    "计算器手势（长按左上图标区）",
                    Some("长按图标区弹出计算器；模组不支持时勾了也不生效。"),
                );
                if self.driver.cfg.calc_gesture {
                    ui.horizontal(|ui| {
                        theme::label_cell(
                            ui,
                            &p,
                            "手势模式",
                            Some("触摸手势的触发方式，取决于这套模组是否支持；改了之后要点「保存并重启服务」。"),
                        );
                        egui::ComboBox::from_id_salt("cfg_calc_mode")
                            .width(theme::ctrl_width())
                            .selected_text(match self.driver.cfg.calc_gesture_mode {
                                CalcMode::Long => "long：长按触发",
                                CalcMode::Off => "off：关闭",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.driver.cfg.calc_gesture_mode,
                                    CalcMode::Long,
                                    "long：长按触发",
                                );
                                ui.selectable_value(
                                    &mut self.driver.cfg.calc_gesture_mode,
                                    CalcMode::Off,
                                    "off：关闭",
                                );
                            });
                    });
                }

                ui.add_space(4.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.grab,
                    "独占触摸板（关掉后系统手势也能同时收到事件）",
                    Some("独占后系统手势拿不到事件；想同时用系统手势就取消勾选。"),
                );
            });
        });
    }
}
