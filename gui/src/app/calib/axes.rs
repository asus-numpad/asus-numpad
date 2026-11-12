//! 面板坐标轴：绝对原点、单位换算与分辨率。

use super::*;

impl App {
    /// 卡二：轴范围与分辨率。
    pub(super) fn draw_calib_axes(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "手动校准",
                Some(
                    "内核报的 ABS_MT_POSITION_* 范围或分辨率不准时，用这里覆盖。只做线性映射，\
                     不交换轴也不翻转——真机上极少见，改错了比不改更糟。",
                ),
            );
            ui.add_space(4.0);

            // X 轴
            let mut x_on = !self.driver.cfg.calibration.x.is_unset();
            let resp = theme::check_row(
                ui,
                &p,
                &mut x_on,
                "覆盖 X 轴",
                Some("不勾就沿用内核上报的范围与分辨率。"),
            );
            if resp.changed() {
                self.driver.cfg.calibration.x = if x_on {
                    numdeck::partition::AxisCal {
                        min: Some(0),
                        max: Some(3600),
                        units_per_mm: None,
                    }
                } else {
                    Default::default()
                };
            }
            if !self.driver.cfg.calibration.x.is_unset() {
                let axis = &mut self.driver.cfg.calibration.x;
                theme::row_hint(
                    ui,
                    &p,
                    "X 范围",
                    Some("ABS_MT_POSITION_X 的最小值与最大值。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, axis.min.get_or_insert(0), -100000..=100000),
                            |ui| {
                                theme::pair_num(ui, axis.max.get_or_insert(3600), -100000..=100000)
                            },
                        )
                    },
                );
                theme::row_hint(
                    ui,
                    &p,
                    "X 分辨率",
                    Some("每毫米多少个单位，等同 evdev 的 resolution；留空沿用内核值。"),
                    "units/mm",
                    |ui| {
                        let mut on = axis.units_per_mm.is_some();
                        let resp = theme::switch(ui, &p, &mut on);
                        if resp.changed() {
                            axis.units_per_mm = if on { Some(40.0) } else { None };
                        }
                        if let Some(units) = axis.units_per_mm.as_mut() {
                            theme::pair_num(ui, units, 1.0..=400.0);
                        }
                        resp
                    },
                );
            }

            ui.add_space(4.0);

            // Y 轴
            let mut y_on = !self.driver.cfg.calibration.y.is_unset();
            let resp = theme::check_row(
                ui,
                &p,
                &mut y_on,
                "覆盖 Y 轴",
                Some("与 X 轴同理，两轴可以只覆盖一个。"),
            );
            if resp.changed() {
                self.driver.cfg.calibration.y = if y_on {
                    numdeck::partition::AxisCal {
                        min: Some(0),
                        max: Some(2200),
                        units_per_mm: None,
                    }
                } else {
                    Default::default()
                };
            }
            if !self.driver.cfg.calibration.y.is_unset() {
                let axis = &mut self.driver.cfg.calibration.y;
                theme::row_hint(
                    ui,
                    &p,
                    "Y 范围",
                    Some("ABS_MT_POSITION_Y 的最小值与最大值。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, axis.min.get_or_insert(0), -100000..=100000),
                            |ui| {
                                theme::pair_num(ui, axis.max.get_or_insert(2200), -100000..=100000)
                            },
                        )
                    },
                );
                theme::row_hint(
                    ui,
                    &p,
                    "Y 分辨率",
                    Some("每毫米多少个单位；留空沿用内核值。"),
                    "units/mm",
                    |ui| {
                        let mut on = axis.units_per_mm.is_some();
                        let resp = theme::switch(ui, &p, &mut on);
                        if resp.changed() {
                            axis.units_per_mm = if on { Some(40.0) } else { None };
                        }
                        if let Some(units) = axis.units_per_mm.as_mut() {
                            theme::pair_num(ui, units, 1.0..=400.0);
                        }
                        resp
                    },
                );
            }

            if !self.driver.cfg.calibration.is_unset() {
                ui.add_space(4.0);
                theme::note(
                    ui,
                    &p,
                    "下界必须小于上界；写反了或被填成 0 以下的分辨率，加载时会丢掉那一轴并在提示里点名。",
                );
            }
        });
    }
}
