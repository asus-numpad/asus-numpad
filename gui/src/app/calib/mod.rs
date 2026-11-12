//! 校准页：官方 15 个布局参数覆盖不到的机型，在这里补分区与面板参数。
//!
//! 两段配置各管一摊：`partition` 决定网格切在哪、切几份；`calibration` 决定面板的
//! 绝对坐标范围与分辨率报的是什么。前者影响按键落在哪一格，后者影响毫米级阈值
//! （手势距离、NumLock 有效距离）算出来的像素值对不对。
//!
//! 采样本身要在真机上按住触摸板，界面里做不了；所以这页的定位是「把采集到的样本
//! 喂进来解算，或者手工填」，解算用与 CLI 完全同一个纯函数。

use eframe::egui;

use numdeck::partition::{solve_calibration, CornerSamples, GridProbe, RawSamples};

use super::calib_view;
use super::extend::save_snippet;
use super::{Action, App};
use crate::theme;

impl App {
    /// 校准页整体。与扩展页同一套「画完比一次配置」的标脏逻辑。
    pub(super) fn draw_calib(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.import_dropped_samples(ui);
        self.refresh_calib_probes();
        self.draw_calib_header(ui, action);
        ui.add_space(10.0);
        self.draw_calib_partition(ui);
        ui.add_space(10.0);
        self.draw_calib_axes(ui);
        ui.add_space(10.0);
        self.draw_calib_samples(ui);
        ui.add_space(10.0);
        self.draw_calib_snippet(ui);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    /// 页首：写哪份配置、改没改、存盘动作用。
    fn draw_calib_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let dirty = self.driver.dirty;
        let write_path = self.driver.write_path.display().to_string();

        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row_help(
                    ui,
                    &p,
                    "自定义分区与校准",
                    Some(
                        "给官方表里没有的机型准备的：贴纸行列不是 5x4、列宽不等分、\
                         或者内核上报的 ABS_MT_POSITION 范围不准时，在这里改。",
                    ),
                );
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
            ui.add_space(6.0);
            calib_view::status_row(ui, &p, &self.driver.cfg);
            ui.add_space(8.0);
            theme::ctrl_column(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::primary(ui, &p, "保存");
                    if resp.clicked() {
                        *action = Some(Action::SaveDriver);
                    }
                    let resp = theme::secondary(ui, &p, "保存并重启服务");
                    if resp.clicked() {
                        *action = Some(Action::SaveDriverAndRestart);
                    }
                    let resp = theme::secondary(ui, &p, "重新读取");
                    if resp.clicked() {
                        *action = Some(Action::ReloadDriver);
                    }
                });
            });
            ui.add_space(4.0);
            theme::note(
                ui,
                &p,
                "分区只改几何：键表仍由「扩展」页的自定义布局给。两段都留空时，官方参数原样生效。",
            );
        });
    }
}

mod axes;
mod ops;
mod partition;
mod samples;
mod snippet;

use samples::{parse_samples, probes_of};

#[cfg(test)]
mod tests {
    use super::snippet::{apply_calib_fragment, calib_fragment_of};
    use super::*;

    fn sample_json() -> String {
        r#"{
            "corners": {
                "top_left": [405, 520],
                "top_right": [3630, 520],
                "bottom_left": [405, 2280],
                "bottom_right": [3630, 2280]
            },
            "first_point": [730, 740],
            "last_point": [3300, 2060]
        }"#
        .to_string()
    }

    #[test]
    fn fragment_roundtrip_keeps_both_sections() {
        let mut src = numdeck::config::Config::default();
        src.partition.rect = Some([100.0, 200.0, 3000.0, 2000.0]);
        src.partition.cols = Some(6);
        src.partition.rows = Some(4);
        src.calibration.x.min = Some(0);
        src.calibration.x.max = Some(3600);

        let text = calib_fragment_of(&src);
        let mut dst = numdeck::config::Config::default();
        let (msg, bad) = apply_calib_fragment(&mut dst, &text);
        assert!(!bad, "{msg}");
        assert_eq!(dst.partition, src.partition);
        assert_eq!(dst.calibration, src.calibration);
    }

    #[test]
    fn fragment_rejects_bad_payload() {
        let mut cfg = numdeck::config::Config::default();
        let (msg, bad) = apply_calib_fragment(&mut cfg, "not json");
        assert!(bad);
        assert!(msg.contains("JSON"), "{msg}");

        let (msg, bad) = apply_calib_fragment(&mut cfg, "{\"partition\": 5}");
        assert!(bad);
        assert!(msg.contains("partition"), "{msg}");

        let (msg, bad) = apply_calib_fragment(&mut cfg, "{\"other\": 1}");
        assert!(bad);
        assert!(msg.contains("没什么可应用"), "{msg}");
    }

    #[test]
    fn samples_missing_key_is_named() {
        let err = parse_samples("{\"corners\":{}}").unwrap_err();
        assert!(err.contains("corners.top_left"), "{err}");

        let err = parse_samples(
            "{\"corners\":{\"top_left\":[1,2],\"top_right\":[3,4],\
             \"bottom_left\":[5,6],\"bottom_right\":[7,8]}}",
        )
        .unwrap_err();
        assert!(err.contains("first_point"), "{err}");
    }

    /// 样本回放走的是与 CLI 同一个纯函数：把样本解出来的分区写回配置后，
    /// 键位图应当能对上自定义行列。
    #[test]
    fn solved_partition_covers_probe_cells() {
        let samples = parse_samples(&sample_json()).unwrap();
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: samples.first_point,
            last_cell: (3, 4),
            last_point: samples.last_point,
            cols: 5,
            rows: 4,
        };
        let cal = solve_calibration(&samples.corners, &probe).unwrap();
        let rect = cal.partition.rect.expect("解算结果必须带矩形");
        assert_eq!(cal.partition.cols, Some(5));
        assert_eq!(cal.partition.rows, Some(4));
        assert!(rect[0] < samples.first_point.0 as f64);
        assert!(rect[2] > samples.last_point.0 as f64);
        assert!(rect[1] < samples.first_point.1 as f64);
        assert!(rect[3] > samples.last_point.1 as f64);
    }
}
