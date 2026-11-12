//! 机型身份：识别结果、布局归属、SKU 来源与压感支持。

use super::*;

impl App {
    /// 机型与布局：这两项决定键位怎么映射，放最前面。
    pub(super) fn draw_identity(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("机型与布局")
            .default_open(true)
            .show(ui, |ui| {
                // ---- 机型 ----
                let mut manual = self.driver.cfg.sku.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "机型",
                        Some("驱动认哪台机器：自动识别按 HID 模组判定。认错会让键位、背光档位一起错，先看下面的判定依据。"),
                    );
                    let text = if manual.is_empty() {
                        "自动识别".to_string()
                    } else {
                        manual.clone()
                    };
                    egui::ComboBox::from_id_salt("cfg_sku")
                        .width(theme::ctrl_width())
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut manual, SKU_AUTO.to_string(), "自动识别");
                            for row in SKUS {
                                ui.selectable_value(
                                    &mut manual,
                                    row.id.to_string(),
                                    format!("{} · {}", row.id, row.display),
                                );
                            }
                        });
                });
                let new_sku = if manual.trim().is_empty() {
                    None
                } else {
                    Some(manual.trim().to_string())
                };
                if new_sku != self.driver.cfg.sku {
                    self.driver.cfg.sku = new_sku;
                }

                match sku::current(self.driver.cfg.sku.as_deref()) {
                    Ok(resolved) => {
                        theme::note(ui, &p, format!(
                            "当前判定：{}（来源 {}）",
                            resolved.id(),
                            source_label(resolved.source)
                        ));
                        if let Some(detail) = &resolved.detail {
                            theme::note(ui, &p, format!("依据：{detail}"));
                        }
                        theme::note(ui, &p, resolved.sku.note);
                    }
                    Err(err) => {
                        ui.label(
                            egui::RichText::new(format!("当前判定：未识别（{err}）"))
                                .small()
                                .color(WARN),
                        );
                        theme::note(ui, &p, "未识别不代表不能跑：手动选一台同模组的机型，或先看下面的系统状态里探测到的模组名。",);
                    }
                }

                // ---- 布局 ----
                ui.add_space(4.0);
                let mut layout = self.driver.cfg.layout_name.clone();
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "布局",
                        Some("数字键区的键表。auto 按 HID 模组挑；列数决定键区宽度，认错时按 7 会串到 8。"),
                    );
                    let text = if layout == "auto" || layout.is_empty() {
                        "自动（按 HID 模组）".to_string()
                    } else {
                        layout.clone()
                    };
                    egui::ComboBox::from_id_salt("cfg_layout")
                        .width(theme::ctrl_width())
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut layout,
                                "auto".to_string(),
                                "自动（按 HID 模组）",
                            );
                            for entry in LAYOUTS {
                                ui.selectable_value(
                                    &mut layout,
                                    entry.name.to_string(),
                                    format!(
                                        "{} · {} 列 · {}",
                                        entry.name,
                                        entry.layout.columns,
                                        support_label(entry.support)
                                    ),
                                );
                            }
                            // 自定义布局排在官方表之后：贴纸 / 镭雕方案的键表都在这里
                            for (name, hids, rows) in custom::list_custom() {
                                let bind = if hids.is_empty() {
                                    "自定义".to_string()
                                } else {
                                    format!("自定义 · {}", hids.join(" "))
                                };
                                ui.selectable_value(
                                    &mut layout,
                                    name.clone(),
                                    format!("{name} · {} 列 · {bind}", rows.len()),
                                );
                            }
                        });
                });
                if layout != self.driver.cfg.layout_name {
                    self.driver.cfg.layout_name = layout;
                }

                if let Some((name, hids, _rows)) = custom::list_custom()
                    .into_iter()
                    .find(|(name, _, _)| *name == self.driver.cfg.layout_name)
                {
                    let cols = custom::table_for(&self.driver.cfg.layout_name).cols();
                    theme::note(ui, &p, format!(
                        "{name} 是自定义布局：{cols} 列 × 4 行{}。键表在「扩展」页里编辑。",
                        if hids.is_empty() {
                            String::new()
                        } else {
                            format!("，绑定 HID {}", hids.join(" "))
                        }
                    ));
                } else if let Some(entry) = LAYOUTS
                    .iter()
                    .find(|e| e.name == self.driver.cfg.layout_name)
                {
                    theme::note(ui, &p, format!(
                        "{} 列 × 4 行，{} 项官方参数",
                        entry.layout.columns,
                        15
                    ));
                    if entry.support.needs_confirm() {
                        ui.label(
                            egui::RichText::new(format!(
                                "这套键表状态是 {}：未在真机上逐键核对，需要显式放行才会用。",
                                support_label(entry.support)
                            ))
                            .small()
                            .color(WARN),
                        );
                    }
                }
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.allow_unverified_layout,
                    "允许未核对布局",
                    Some("family / unsupported 的键表也放行：能用，但没在真机上逐键核对过。"),
                );

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "键码风格",
                        Some("keypad 发 KEY_KP* 小键盘码；digits 发主键区 0-9——某些游戏和输入法只认后者。"),
                    );
                    let mut mode = self.driver.cfg.key_mode;
                    egui::ComboBox::from_id_salt("cfg_key_mode")
                        .width(theme::ctrl_width())
                        .selected_text(match mode {
                            KeyMode::Keypad => "keypad（KEY_KP*）",
                            KeyMode::Digits => "digits（主键区 0-9）",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut mode,
                                KeyMode::Keypad,
                                "keypad（KEY_KP*）",
                            );
                            ui.selectable_value(
                                &mut mode,
                                KeyMode::Digits,
                                "digits（主键区 0-9）",
                            );
                        });
                    self.driver.cfg.key_mode = mode;
                });
                theme::note(
                    ui,
                    &p,
                    "keypad 对小键盘层有效；digits 让数字进主键区，某些游戏与输入法只认这一种。",
                );
            });
        });
    }
}
