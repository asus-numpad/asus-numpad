//! 解算与刷新：跑解算、导入拖来的样本文件、重探针。

use super::*;

impl App {
    /// 读样本文件 → 解算 → 写回分区。返回给界面显示的文案与是否出错。
    pub(super) fn run_calibration(&mut self) -> (String, bool) {
        let path = self.calib_samples_path.trim().to_string();
        if path.is_empty() {
            return ("先填样本文件路径".to_string(), true);
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => return (format!("读不到 {path}：{err}"), true),
        };
        let samples = match parse_samples(&text) {
            Ok(samples) => samples,
            Err(err) => return (err, true),
        };
        let cols = self.calib_cols.clamp(1, 8);
        let rows = self.calib_rows.clamp(1, 4);
        let probe = GridProbe {
            first_cell: (0, 0),
            first_point: samples.first_point,
            last_cell: (rows - 1, cols - 1),
            last_point: samples.last_point,
            cols,
            rows,
        };
        let cal = match solve_calibration(&samples.corners, &probe) {
            Ok(cal) => cal,
            Err(err) => return (err, true),
        };

        let mut out = String::new();
        out.push_str(&format!(
            "网格 {}列 x {}行  [{}] .. [{}]",
            cal.partition.cols.unwrap_or(cols),
            cal.partition.rows.unwrap_or(rows),
            cal.partition
                .rect
                .map(|r| format!("{:.0}, {:.0}", r[0], r[1]))
                .unwrap_or_default(),
            cal.partition
                .rect
                .map(|r| format!("{:.0}, {:.0}", r[2], r[3]))
                .unwrap_or_default(),
        ));
        if let Some(rect) = cal.partition.rect {
            let w = (rect[2] - rect[0]) / cols as f64;
            let h = (rect[3] - rect[1]) / rows as f64;
            out.push('\n');
            out.push_str(&format!("格尺寸 {w:.1} x {h:.1} units（已写进分区）"));
        }
        out.push('\n');
        out.push_str(&format!(
            "实测角点范围 x {}..{} y {}..{}——若与内核上报差很多，再在「手动校准」里覆盖轴范围。",
            cal.panel.xmin, cal.panel.xmax, cal.panel.ymin, cal.panel.ymax
        ));

        self.driver.cfg.partition = cal.partition;
        self.calib_probes = probes_of(&samples, cols, rows);
        (out, false)
    }

    /// 拖进来的文件直接当样本路径：省掉「翻目录找文件」这一步。
    pub(super) fn import_dropped_samples(&mut self, ui: &mut egui::Ui) {
        let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        let Some(path) = dropped.into_iter().next() else {
            return;
        };
        let text = path.display().to_string();
        if text == self.calib_samples_path {
            return;
        }
        self.calib_samples_path = text.clone();
        self.calib_msg = Some((format!("已载入样本文件 {text}"), false));
    }

    /// 样本路径或行列数一变就重解析：预览图上的点跟着输入框走，不必再点「读取并解算」。
    ///
    /// 每帧只比一个字符串，没有 IO；真正读文件只发生在路径或行列数变了以后。
    pub(super) fn refresh_calib_probes(&mut self) {
        let key = format!(
            "{}|{}x{}",
            self.calib_samples_path.trim(),
            self.calib_cols,
            self.calib_rows
        );
        if key == self.calib_probe_path {
            return;
        }
        self.calib_probe_path = key;
        self.calib_probes.clear();
        self.calib_probe_msg = None;
        let path = self.calib_samples_path.trim().to_string();
        if path.is_empty() {
            return;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) => {
                self.calib_probe_msg = Some(format!("读不到 {path}：{err}"));
                return;
            }
        };
        match parse_samples(&text) {
            Ok(samples) => {
                self.calib_probes = probes_of(&samples, self.calib_cols, self.calib_rows)
            }
            Err(err) => self.calib_probe_msg = Some(err),
        }
    }
}
