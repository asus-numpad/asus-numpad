//! 手势：滚动、应用切换、双击窗口与位移阈值。

use super::*;

impl App {
    pub(super) fn draw_gestures(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("手势与阈值（改错会影响手感）")
                .default_open(false)
                .show(ui, |ui| {
                    let rows: [(&str, &mut f64, &str, &str); 5] = [
                        (
                            "应用手势触发距离",
                            &mut self.driver.cfg.app_gesture_trig_um,
                            "μm",
                            "从触摸板边缘往中间划出这段距离才算「应用手势」，调小更容易触发。",
                        ),
                        (
                            "手势触发距离",
                            &mut self.driver.cfg.gesture_trig_um,
                            "μm",
                            "普通手势的位移阈值，小于这个距离的移动当作无意的抖动丢掉。",
                        ),
                        (
                            "轻触阈值",
                            &mut self.driver.cfg.tap_threshold_mm,
                            "mm",
                            "手指按下后移动不超过这段距离才算轻触，超过则判定成滑动。",
                        ),
                        (
                            "双指滚动距离",
                            &mut self.driver.cfg.scroll_finger_dist_mm,
                            "mm",
                            "两指间距超过这个值才开始算双指滚动，避免误判成点击。",
                        ),
                        (
                            "连击最小间隔",
                            &mut self.driver.cfg.tap_interval_min_ms,
                            "ms",
                            "两次轻触之间至少隔这么久才算新的一击，否则合并成连击。",
                        ),
                    ];
                    for (label, value, unit, help) in rows {
                        theme::row(ui, &p, label, Some(help), |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(value)
                                    .speed(1.0)
                                    .suffix(format!(" {unit}")),
                            );
                        });
                    }
                    theme::row(
                        ui,
                        &p,
                        "触摸板尺寸",
                        Some("触摸板物理尺寸（mm），手势阈值按它换算。"),
                        |ui| {
                            // 两个数字框 + 中间的「×」与单位「mm」，加起来正好一列宽
                            let side = (theme::ctrl_width() - 52.0) / 2.0;
                            let dim = egui::Vec2::new(side, theme::CTRL_H);
                            let res = ui.add_sized(
                                dim,
                                egui::DragValue::new(&mut self.driver.cfg.touchpad_width_mm),
                            );
                            theme::tip(res, "触摸板宽（mm），手势阈值按它换算。");
                            ui.label(egui::RichText::new("×").color(p.text_weak));
                            let res = ui.add_sized(
                                dim,
                                egui::DragValue::new(&mut self.driver.cfg.touchpad_height_mm),
                            );
                            theme::tip(res, "触摸板高（mm），手势阈值按它换算。");
                            ui.label(egui::RichText::new("mm").color(p.text_weak));
                        },
                    );
                });
        });
    }
}
