//! 分区：网格切几列几行、边界各占多少权重，含解算入口与逐行列权重输入。

use super::*;

impl App {
    /// 卡一：分区矩形与行列。
    pub(super) fn draw_calib_partition(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "分区",
                Some(
                    "直接用边界把网格切成任意行列。坐标是触摸板的原始值，与 evtest 里看到的一致。",
                ),
            );
            ui.add_space(4.0);

            theme::row_hint(
                ui,
                &p,
                "生效布局",
                Some("auto = 当前生效布局；也可以填具体布局名或自定义布局名。"),
                "auto / 布局名",
                |ui| theme::text_field(ui, &mut self.driver.cfg.partition.layout, "auto"),
            );

            // 矩形：勾上才参与，四值按 x0 y0 x1 y1 排
            let mut has_rect = self.driver.cfg.partition.rect.is_some();
            let resp = theme::check_row(
                ui,
                &p,
                &mut has_rect,
                "自定义矩形",
                Some("不勾就用官方 index0/index4/level0/level3 推出来的网格。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.rect = if has_rect {
                    Some([0.0, 0.0, 3600.0, 2200.0])
                } else {
                    None
                };
            }
            if let Some(rect) = self.driver.cfg.partition.rect {
                // 四个角拆成独立变量再写回：同一张卡里两个闭包同时借一个数组的下标，
                // borrow checker 会判定成整体借用。
                let (mut x0, mut y0, mut x1, mut y1) = (rect[0], rect[1], rect[2], rect[3]);
                theme::row_hint(
                    ui,
                    &p,
                    "左上 x / y",
                    Some("网格左上角在触摸板坐标系里的位置。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, &mut x0, -100000.0..=100000.0),
                            |ui| theme::pair_num(ui, &mut y0, -100000.0..=100000.0),
                        )
                    },
                );
                theme::row_hint(
                    ui,
                    &p,
                    "右下 x / y",
                    Some("网格右下角；必须比左上角大，否则这条配置会被拒绝。"),
                    "原始值",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, &mut x1, -100000.0..=100000.0),
                            |ui| theme::pair_num(ui, &mut y1, -100000.0..=100000.0),
                        )
                    },
                );
                if [x0, y0, x1, y1] != rect {
                    self.driver.cfg.partition.rect = Some([x0, y0, x1, y1]);
                }
            }

            // 预览：把当前配置直接画出来。四个角、行列数、权重、NumLock 格填完长什么样，
            // 只有画出来才能判断对不对——数字全对而网格偏一格，是这页最容易犯的错。
            theme::block(
                ui,
                &p,
                "预览",
                Some(
                    "按当前配置画出的网格，鼠标移上去读每格中心坐标；样本点也画在这张图上。\
                     底下一整块是触摸板，网格之外就是光标区。",
                ),
                |ui| {
                    let (hover, _err) =
                        calib_view::plate(ui, &p, &self.driver.cfg, &self.calib_probes);
                    if !self.calib_probes.is_empty() {
                        let at = self.calib_probes.len();
                        ui.label(
                            egui::RichText::new(format!("已画出 {at} 个样本点"))
                                .size(11.5)
                                .color(p.text_weak),
                        );
                    }
                    // 读数放在图下面一行：整宽铺开，悬停时换内容而不改行高，图不会跟着跳
                    let grid = calib_view::resolve(&self.driver.cfg).ok();
                    let (text, color) = calib_view::readout(&p, grid.as_ref(), hover.as_ref());
                    ui.label(egui::RichText::new(text).size(11.5).color(color));
                },
            );

            // 行列：留空跟随键表
            let mut follow_rows = self.driver.cfg.partition.rows.is_none();
            let resp = theme::check_row(
                ui,
                &p,
                &mut follow_rows,
                "行列跟随键表",
                Some("键表本身就有行列（4 行、1..=8 列），一般不用在这里另填。"),
            );
            if resp.changed() {
                let (rows, cols) = if follow_rows {
                    (None, None)
                } else {
                    (Some(4usize), Some(5usize))
                };
                self.driver.cfg.partition.rows = rows;
                self.driver.cfg.partition.cols = cols;
            }
            if let (Some(rows), Some(cols)) = (
                self.driver.cfg.partition.rows.as_mut(),
                self.driver.cfg.partition.cols.as_mut(),
            ) {
                theme::row_hint(
                    ui,
                    &p,
                    "列数 / 行数",
                    Some("列数可以 1..=8；行数固定 4（键表就是 4 行）。"),
                    "列 / 行",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, cols, 1..=8),
                            |ui| theme::pair_num(ui, rows, 1..=4),
                        )
                    },
                );
            }

            theme::check_row(
                ui,
                &p,
                &mut self.driver.cfg.partition.snap_outside,
                "网格外吸附到最近一格",
                Some("默认关闭：网格外的落点属于光标区，按官方行为不产生按键。"),
            );

            // 列宽 / 行高权重。改列数行数后长度自动补齐，不然一改列数整段就被判成坏值。
            let keys = {
                let table = self.driver.cfg.key_table();
                (table.cols().max(1), table.rows().len().max(1))
            };
            let cols_now = self.driver.cfg.partition.cols.unwrap_or(keys.0).clamp(1, 8);
            let rows_now = self.driver.cfg.partition.rows.unwrap_or(keys.1).clamp(1, 4);
            if !self.driver.cfg.partition.col_ratios.is_empty()
                && self.driver.cfg.partition.col_ratios.len() != cols_now
            {
                self.driver.cfg.partition.col_ratios.resize(cols_now, 1.0);
            }
            if !self.driver.cfg.partition.row_ratios.is_empty()
                && self.driver.cfg.partition.row_ratios.len() != rows_now
            {
                self.driver.cfg.partition.row_ratios.resize(rows_now, 1.0);
            }

            let mut col_weights = !self.driver.cfg.partition.col_ratios.is_empty();
            let resp = theme::check_row(
                ui,
                &p,
                &mut col_weights,
                "列宽不等分",
                Some("勾上按权重分配列宽，权重自动归一；全填 1 就是等分。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.col_ratios = if col_weights {
                    vec![1.0; cols_now]
                } else {
                    Vec::new()
                };
            }
            if !self.driver.cfg.partition.col_ratios.is_empty() {
                theme::block(
                    ui,
                    &p,
                    "列宽权重",
                    Some("从左到右每一列占几份；数字越大这列越宽。"),
                    |ui| weight_row(ui, &mut self.driver.cfg.partition.col_ratios),
                );
            }

            let mut row_weights = !self.driver.cfg.partition.row_ratios.is_empty();
            let resp = theme::check_row(
                ui,
                &p,
                &mut row_weights,
                "行高不等分",
                Some("同上，按行分配高度；触摸板上下留白不等时用它。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.row_ratios = if row_weights {
                    vec![1.0; rows_now]
                } else {
                    Vec::new()
                };
            }
            if !self.driver.cfg.partition.row_ratios.is_empty() {
                theme::block(
                    ui,
                    &p,
                    "行高权重",
                    Some("从上到下每一行占几份；数字越大这行越高。"),
                    |ui| weight_row(ui, &mut self.driver.cfg.partition.row_ratios),
                );
            }

            let mut nl_custom = self.driver.cfg.partition.numlock_cell.is_some();
            let resp = theme::check_row(
                ui,
                &p,
                &mut nl_custom,
                "指定 NumLock 格",
                Some("不勾就是第 1 行最后一列，也就是官方布局里那个位置。"),
            );
            if resp.changed() {
                self.driver.cfg.partition.numlock_cell = if nl_custom {
                    Some([0, cols_now.saturating_sub(1)])
                } else {
                    None
                };
            }
            if let Some([row, col]) = self.driver.cfg.partition.numlock_cell {
                let (mut row, mut col) = (row, col);
                theme::row_hint(
                    ui,
                    &p,
                    "NumLock 行 / 列",
                    Some("行列都从 0 数起；预览图上这一格标着 NL。"),
                    "行 / 列",
                    |ui| {
                        theme::pair(
                            ui,
                            |ui| theme::pair_num(ui, &mut row, 0..=rows_now.saturating_sub(1)),
                            |ui| theme::pair_num(ui, &mut col, 0..=cols_now.saturating_sub(1)),
                        )
                    },
                );
                if [row, col] != self.driver.cfg.partition.numlock_cell.unwrap_or([row, col]) {
                    self.driver.cfg.partition.numlock_cell = Some([row, col]);
                }
            }

            ui.add_space(4.0);
            calib_view::lint_notes(ui, &p, &self.driver.cfg);
        });
    }
}

/// 一行并排的权重数字框：几份就几个，等宽平分控件列，右端与其它控件压同一条线。
pub(super) fn weight_row(ui: &mut egui::Ui, values: &mut [f64]) {
    let count = values.len().max(1) as f32;
    let gap = ui.spacing().item_spacing.x;
    let width = ((theme::ctrl_width() - gap * (count - 1.0)) / count).max(40.0);
    ui.horizontal(|ui| {
        for (index, value) in values.iter_mut().enumerate() {
            let resp = ui.add_sized(
                egui::vec2(width, theme::CTRL_H),
                egui::DragValue::new(value)
                    .speed(0.05)
                    .range(0.05..=10.0)
                    .max_decimals(2),
            );
            theme::tip(resp, format!("第 {} 份的权重", index + 1));
        }
    });
}
