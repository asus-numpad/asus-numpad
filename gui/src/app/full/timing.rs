//! 时序：帧长、长按重复、抬指延时等时间参数。

use super::*;

impl App {
    pub(super) fn draw_timing(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("时序参数（固件帧计数，一般不用动）")
                .default_open(false)
                .show(ui, |ui| {
                    let rows: [(&str, &mut u32, &str); 6] = [
                        (
                            "帧间隔",
                            &mut self.driver.cfg.frame_ms,
                            "固件上报的帧间隔，下面这些阈值都以它为计数单位。",
                        ),
                        (
                            "NumLock 触发帧",
                            &mut self.driver.cfg.numlock_trigger_frames,
                            "按下 NumLock 键需连续保持的帧数，太小容易被误触。",
                        ),
                        (
                            "SENDKEY 连续触发帧",
                            &mut self.driver.cfg.cont_sendkey_trigger_frames,
                            "SENDKEY 手势连续触发所需的帧数，越大越不容易误发。",
                        ),
                        (
                            "图标区长按帧",
                            &mut self.driver.cfg.numadv_trigger_frames,
                            "按住左上角图标区多少帧即切到计算器模式。",
                        ),
                        (
                            "按键保持帧",
                            &mut self.driver.cfg.touch_sendkey_frames,
                            "发键后保持电平的帧数，够驱动侧采到就行。",
                        ),
                        (
                            "最长工作时间",
                            &mut self.driver.cfg.max_work_time_ms,
                            "单次手势会话的最长持续时间，超时后重新开始计时。",
                        ),
                    ];
                    for (label, value, help) in rows {
                        theme::row(ui, &p, label, Some(help), |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(value),
                            );
                        });
                    }
                    theme::row(
                        ui,
                        &p,
                        "SENDKEY 判定比例",
                        Some("手势里判定为 SENDKEY 的比例阈值，越小越灵敏。"),
                        |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(&mut self.driver.cfg.touch_sendkey_ratio),
                            );
                        },
                    );
                });
        });
    }
}
