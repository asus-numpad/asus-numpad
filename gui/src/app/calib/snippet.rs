//! 片段：把本页两项配置序列化成可分发片段，或反向吃进一段片段。

use super::*;

impl App {
    /// 卡四：把分区 + 校准打包成片段，同机型对拷用。
    pub(super) fn draw_calib_snippet(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "配置片段",
                Some(
                    "把 partition 与 calibration 打包成一段 JSON。不含键表：\
                     键表在「扩展」页的自定义布局里。",
                ),
            );
            ui.add_space(4.0);
            theme::row(
                ui,
                &p,
                "片段",
                Some("点「导出」从当前配置生成；也可以直接粘贴别人给的片段。"),
                |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.calib_snippet)
                            .desired_rows(5)
                            .desired_width(theme::ctrl_width())
                            .hint_text("{ ... }"),
                    )
                },
            );
            ui.add_space(2.0);
            theme::ctrl_column(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::secondary(ui, &p, "导出");
                    if resp.clicked() {
                        self.calib_snippet = calib_fragment_of(&self.driver.cfg);
                        let len = self.calib_snippet.len();
                        self.calib_snippet_msg = Some((format!("已生成 {len} 字节的片段"), false));
                    }
                    let resp = theme::secondary(ui, &p, "复制");
                    if resp.clicked() {
                        if self.calib_snippet.trim().is_empty() {
                            self.calib_snippet_msg =
                                Some(("框里还是空的，先点「导出」".to_string(), true));
                        } else {
                            ui.ctx().copy_text(self.calib_snippet.clone());
                            let len = self.calib_snippet.len();
                            self.calib_snippet_msg =
                                Some((format!("已复制 {len} 字节到剪贴板"), false));
                        }
                    }
                    let resp = theme::secondary(ui, &p, "存成文件");
                    if resp.clicked() {
                        self.calib_snippet_msg = Some(match save_snippet(&self.calib_snippet) {
                            Ok(path) => (format!("已写入 {}", path.display()), false),
                            Err(err) => (err, true),
                        });
                    }
                    let resp = theme::secondary(ui, &p, "应用");
                    let resp = theme::tip(
                        resp,
                        "写进当前配置；只有 partition / calibration 两段，其它键保持原样。",
                    );
                    if resp.clicked() {
                        self.calib_snippet_msg = Some(apply_calib_fragment(
                            &mut self.driver.cfg,
                            &self.calib_snippet,
                        ));
                    }
                });
            });
            if let Some((text, bad)) = self.calib_snippet_msg.clone() {
                ui.add_space(4.0);
                if bad {
                    theme::note(ui, &p, format!("片段有问题：{text}"));
                } else {
                    theme::note(ui, &p, text);
                }
            }
        });
    }
}

/// 分区 + 校准打包成一段 JSON 文本。
pub(super) fn calib_fragment_of(cfg: &numdeck::config::Config) -> String {
    let value = serde_json::json!({
        "partition": cfg.partition,
        "calibration": cfg.calibration,
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// 片段写回配置：两段先都解析成功，再一次性写回。
pub(super) fn apply_calib_fragment(
    cfg: &mut numdeck::config::Config,
    text: &str,
) -> (String, bool) {
    let text = text.trim();
    if text.is_empty() {
        return (
            "片段是空的：先点「导出」，或粘贴别人的片段".to_string(),
            true,
        );
    }
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => return (format!("不是合法 JSON：{err}"), true),
    };
    let partition = match value.get("partition") {
        Some(raw) => match serde_json::from_value(raw.clone()) {
            Ok(spec) => Some(spec),
            Err(err) => return (format!("partition 解析失败：{err}"), true),
        },
        None => None,
    };
    let calibration = match value.get("calibration") {
        Some(raw) => match serde_json::from_value(raw.clone()) {
            Ok(spec) => Some(spec),
            Err(err) => return (format!("calibration 解析失败：{err}"), true),
        },
        None => None,
    };
    if partition.is_none() && calibration.is_none() {
        return (
            "片段里没有 partition 或 calibration，没什么可应用的".to_string(),
            true,
        );
    }
    let mut touched = Vec::new();
    if let Some(spec) = partition {
        cfg.partition = spec;
        touched.push("分区".to_string());
    }
    if let Some(spec) = calibration {
        cfg.calibration = spec;
        touched.push("校准".to_string());
    }
    (format!("已应用：{}", touched.join("、")), false)
}
