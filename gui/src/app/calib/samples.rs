//! 采样：粘贴/拖入样本文本，解析成角点与网格探针，画出探针表。

use super::*;

impl App {
    /// 卡三：样本回放——把真机采到的落点喂进解算器，结果直接写回配置。
    pub(super) fn draw_calib_samples(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "样本回放",
                Some(
                    "采集要在真机上按住触摸板，界面里做不了：在终端跑 `numdeck calibrate --samples s.json` \
                     采一份，或者手写一份样本文件，回到这里解算。",
                ),
            );
            ui.add_space(4.0);

            theme::row(
                ui,
                &p,
                "样本文件",
                Some("JSON：corners 四角 + first_point / last_point 两个对角格中心。"),
                |ui| theme::text_field(ui, &mut self.calib_samples_path, "/path/to/samples.json"),
            );

            let mut cols = self.calib_cols;
            let mut rows = self.calib_rows;
            theme::row_hint(
                ui,
                &p,
                "列数 / 行数",
                Some("解算要知道样本对应几列几行；与键表一致，否则格宽会算错。"),
                "列 / 行",
                |ui| {
                    theme::pair(
                        ui,
                        |ui| theme::pair_num(ui, &mut cols, 1..=8),
                        |ui| theme::pair_num(ui, &mut rows, 1..=4),
                    )
                },
            );
            self.calib_cols = cols;
            self.calib_rows = rows;

            ui.add_space(2.0);
            theme::ctrl_column(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::secondary(ui, &p, "读取并解算");
                    let resp = theme::tip(
                        resp,
                        "读样本文件，解出网格矩形与格尺寸，直接写进上面的分区。",
                    );
                    if resp.clicked() {
                        self.calib_msg = Some(self.run_calibration());
                    }
                    let resp = theme::secondary(ui, &p, "清空分区");
                    let resp = theme::tip(resp, "把分区整段清掉，回到官方参数。");
                    if resp.clicked() {
                        self.driver.cfg.partition = Default::default();
                        self.calib_msg = Some(("已清空分区".to_string(), false));
                    }
                });
            });

            if let Some((text, bad)) = self.calib_msg.clone() {
                ui.add_space(4.0);
                if bad {
                    theme::note(ui, &p, format!("解算失败：{text}"));
                } else {
                    for line in text.lines() {
                        theme::note(ui, &p, line.to_string());
                    }
                }
            }

            // 解算只保证矩形对；样本点究竟落进哪一格，还得单独核一遍。
            // 点在手边时最常犯的错就是「按到了隔壁格」，数字全对、键全错一格。
            if let Some(err) = self.calib_probe_msg.clone() {
                ui.add_space(4.0);
                theme::note(
                    ui,
                    &p,
                    format!("样本没读成：{err}（预览图上暂时没有样本点）"),
                );
            } else if !self.calib_probes.is_empty() {
                if let Ok(grid) = calib_view::resolve(&self.driver.cfg) {
                    let issues = calib_view::probe_check(&grid, &self.calib_probes);
                    ui.add_space(4.0);
                    if issues.is_empty() {
                        theme::note(ui, &p, "样本点都和网格对得上：每个点都落在它该在的那一格。");
                    } else {
                        for line in issues {
                            theme::note(ui, &p, line);
                        }
                    }
                }
            }
        });
    }
}

/// 样本 JSON → 结构化样本；字段缺失或类型不对时把是哪个键说清楚。
pub(super) fn parse_samples(text: &str) -> Result<RawSamples, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("不是合法 JSON：{err}"))?;
    let corner = |name: &str| -> Result<(i32, i32), String> {
        let item = value
            .get("corners")
            .and_then(|c| c.get(name))
            .ok_or_else(|| format!("样本缺少 corners.{name}"))?;
        pair(item).ok_or_else(|| format!("corners.{name} 必须是 [x, y] 两个整数"))
    };
    let point = |name: &str| -> Result<(i32, i32), String> {
        let item = value.get(name).ok_or_else(|| format!("样本缺少 {name}"))?;
        pair(item).ok_or_else(|| format!("{name} 必须是 [x, y] 两个整数"))
    };
    Ok(RawSamples {
        corners: CornerSamples {
            top_left: corner("top_left")?,
            top_right: corner("top_right")?,
            bottom_left: corner("bottom_left")?,
            bottom_right: corner("bottom_right")?,
        },
        first_point: point("first_point")?,
        last_point: point("last_point")?,
    })
}

pub(super) fn pair(value: &serde_json::Value) -> Option<(i32, i32)> {
    let arr = value.as_array()?;
    if arr.len() != 2 {
        return None;
    }
    let x = arr[0].as_i64()?;
    let y = arr[1].as_i64()?;
    Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

/// 样本 → 预览图上的点。
///
/// 四角只画不判：它们标的是触摸板外框，网格可以只覆盖其中一块，落在网格外属正常。
/// 格中心那两个才带「该在第几格」，画上去用来自查有没有按偏。
pub(super) fn probes_of(samples: &RawSamples, cols: usize, rows: usize) -> Vec<calib_view::Probe> {
    let cols = cols.clamp(1, 8);
    let rows = rows.clamp(1, 4);
    let probe = |raw: (i32, i32), want, label| calib_view::Probe {
        raw: (raw.0 as f64, raw.1 as f64),
        want,
        label,
    };
    vec![
        probe(samples.corners.top_left, None, "左上角"),
        probe(samples.corners.top_right, None, "右上角"),
        probe(samples.corners.bottom_left, None, "左下角"),
        probe(samples.corners.bottom_right, None, "右下角"),
        probe(samples.first_point, Some((0, 0)), "首格中心"),
        probe(samples.last_point, Some((rows - 1, cols - 1)), "末格中心"),
    ]
}
