//! 自定义布局的绘制：基准布局选择、4 行键表、键位预览。

use eframe::egui;

use numdeck::custom::{self, CustomLayoutSpec, KeySlot};
use numdeck::layout::{self, Layout, LAYOUTS};

use super::WARN;
use crate::theme;

use super::*;

/// 画一条自定义布局的全部字段。
///
/// 走自由函数而不是 `&mut self`：编辑 `custom_layouts[i]` 时不能同时借整个 `App`。
pub(crate) fn draw_one_layout(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    spec: &mut CustomLayoutSpec,
    index: usize,
) {
    theme::row(
        ui,
        p,
        "布局名",
        Some("写进驱动配置的 layout_name，也是自动识别命中时显示的名字。别和内置布局重名。"),
        |ui| theme::text_field(ui, &mut spec.name, "例如 GH3X-贴纸"),
    );

    // 基准布局：内置 + 已注册的自定义（可以基于上一条继续改）
    theme::row(
        ui,
        p,
        "基准",
        Some("只有写到的参数才覆盖基准，其余沿用。键表列数与基准不同也没关系。"),
        |ui| {
            let mut current = spec.base.clone();
            let text = if current.trim().is_empty() {
                "（未选）".to_string()
            } else {
                current.clone()
            };
            egui::ComboBox::from_id_salt(("custom_base", index))
                .width(theme::ctrl_width())
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for entry in LAYOUTS {
                        ui.selectable_value(
                            &mut current,
                            entry.name.to_string(),
                            format!("{} · {} 列", entry.name, entry.layout.columns),
                        );
                    }
                    for (name, _hids, _rows) in custom::list_custom() {
                        ui.selectable_value(&mut current, name.clone(), format!("{name} · 自定义"));
                    }
                });
            if current != spec.base {
                spec.base = current;
            }
        },
    );

    // 键表：4 行单行输入，一行一串键名。
    // 只补齐、不截断：配置里若是手写了第 5 行，驱动侧解析会明确报错，界面这里
    // 悄悄把那行删掉就成了「打开界面即改配置」，保存后用户才发现内容少了。
    while spec.keys.len() < KEY_ROWS {
        spec.keys.push(String::new());
    }
    let extra_rows = spec.keys.len() - KEY_ROWS;
    theme::block(
        ui,
        p,
        "键表",
        Some("按小键盘实际排布逐行写。每行从左到右，空格分隔；行数与列数不必和官方一致。"),
        |ui| {
            for row in 0..KEY_ROWS {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        egui::Vec2::new(28.0, theme::CTRL_H),
                        egui::Label::new(
                            egui::RichText::new(format!("R{}", row + 1))
                                .size(12.0)
                                .color(p.text_weak),
                        ),
                    );
                    ui.add_sized(
                        egui::Vec2::new(theme::ctrl_width() - 40.0, theme::CTRL_H),
                        egui::TextEdit::singleline(&mut spec.keys[row]).hint_text(match row {
                            0 => "7 8 9 +",
                            1 => "4 5 6 -",
                            2 => "1 2 3 *",
                            _ => "0 . enter /",
                        }),
                    );
                });
            }
            ui.add_space(4.0);
            // 贴纸/镭雕多半是「照基准机型改一两个键」，先把基准键表抄成文本再改最省事
            ui.horizontal(|ui| {
                let base_name = spec.base.trim().to_string();
                let hint = if base_name.is_empty() {
                    "未选基准，会按内置键表填".to_string()
                } else {
                    format!("把 {base_name} 的键表抄成 4 行文字，在这上面逐个改键")
                };
                let resp = theme::secondary(ui, p, "按基准填满");
                let resp = theme::tip(resp, hint);
                if resp.clicked() {
                    spec.keys = custom::table_for(&base_name).describe();
                }
                let resp = theme::secondary(ui, p, "清空键表");
                let resp = theme::tip(resp, "清空表示键位沿用基准，只改行列几何时用这个。");
                if resp.clicked() {
                    spec.keys = vec![String::new(); KEY_ROWS];
                }
            });
        },
    );

    // 实时解析结果：不用点保存就能知道这行键表能不能认
    if custom::keys_inherit_base(&spec.keys) {
        theme::read_row(
            ui,
            p,
            "键表解析",
            &format!(
                "留空，沿用 {} 的键表",
                if spec.base.trim().is_empty() {
                    "默认布局"
                } else {
                    spec.base.trim()
                }
            ),
            Some("键位照抄基准机型、只改行列几何时，把这一栏清空即可。"),
        );
        return;
    }
    match custom::KeyTable::parse(&spec.keys) {
        Ok(table) => {
            theme::read_row(
                ui,
                p,
                "键表解析",
                &format!("{} 列 × {} 行，可用", table.cols(), KEY_ROWS),
                Some("列数以键表为准：贴纸几列，键区就几列，不会串到相邻列。"),
            );
            draw_key_preview(ui, p, &table);
        }
        Err(err) => {
            theme::row(ui, p, "键表解析", None, |ui| {
                ui.label(egui::RichText::new(err).size(12.0).color(WARN));
            });
        }
    }

    if extra_rows > 0 {
        theme::row(ui, p, "", None, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "配置里这段键表写了 {} 行，界面只编辑前 {KEY_ROWS} 行；驱动上限就是 {KEY_ROWS} 行，多出的行会在校验时报错。",
                    spec.keys.len()
                ))
                .size(12.0)
                .color(WARN),
            );
        });
    }

    // 几何覆盖：贴纸 / 镭雕方案真正要动的往往是这些 0.1 mm 的边界值，
    // 键表照抄基准就能用，键区却还压在基准机型的位置上。
    draw_one_patch(ui, p, index, &mut spec.patch, resolve_base(&spec.base));

    // HID 绑定
    theme::block(
        ui,
        p,
        "绑定 HID",
        Some("填触摸板 HID 模组 ID（如 ASUE1416）。layout 为 auto 时命中即自动用这套键表。"),
        |ui| {
            let mut remove_hid: Option<usize> = None;
            for slot in 0..spec.hid_ids.len() {
                let id = slot;
                ui.horizontal(|ui| {
                    ui.add_sized(
                        egui::Vec2::new(theme::ctrl_width() - 96.0, theme::CTRL_H),
                        egui::TextEdit::singleline(&mut spec.hid_ids[id]).hint_text("ASUE1416"),
                    );
                    if theme::secondary(ui, p, "删除").clicked() {
                        remove_hid = Some(id);
                    }
                });
            }
            if let Some(slot) = remove_hid {
                spec.hid_ids.remove(slot);
            }
            ui.horizontal(|ui| {
                if theme::secondary(ui, p, "添加 HID").clicked() {
                    spec.hid_ids.push(String::new());
                }
            });
        },
    );
}

/// 新加一条自定义布局的初值：键表留空，用户照贴纸填。
pub(crate) fn new_layout(index: usize) -> CustomLayoutSpec {
    CustomLayoutSpec {
        name: format!("custom-{}", index + 1),
        ..CustomLayoutSpec::default()
    }
}

/// 键表预览：照贴纸的排布摆成网格，列数、行数、空位一眼看清。
///
/// 只画解析结果，不参与命中；真正生效的行列数仍以驱动解析为准。
pub(crate) fn draw_key_preview(ui: &mut egui::Ui, p: &theme::Palette, table: &custom::KeyTable) {
    let cols = table.cols().clamp(1, 8);
    let gap = 4.0;
    let cell_w = ((theme::ctrl_width() - gap * (cols as f32 - 1.0)) / cols as f32).min(76.0);
    ui.horizontal_top(|ui| {
        ui.add_space(theme::LABEL_W + ui.spacing().item_spacing.x);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = gap;
            for row in 0..KEY_ROWS {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for col in 0..cols {
                        let slot = table.get(row, col).unwrap_or(KeySlot::None);
                        key_cell(ui, p, slot, cell_w);
                    }
                });
            }
        });
    });
}

/// 预览里的一格：有键实底、空位凹底，空位印 `·` 以免看成少了格子。
pub(crate) fn key_cell(ui: &mut egui::Ui, p: &theme::Palette, slot: KeySlot, cell_w: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(cell_w, 26.0), egui::Sense::hover());
    let empty = matches!(slot, KeySlot::None);
    let fill = if empty { p.sunk } else { p.field };
    let color = if empty { p.text_weak } else { p.text };
    let text = if empty {
        "·".to_string()
    } else {
        slot.label()
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        fill,
        egui::Stroke::new(1.0, p.stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.0),
        color,
    );
}

/// 配置里 `base` 指向的基准布局：先查内置表，再查已注册的自定义布局。
///
/// 只用于把基准数值显示在界面上给人对照，不参与命中判定。
pub(crate) fn resolve_base(name: &str) -> Option<Layout> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    if let Some(entry) = layout::builtin_by_name(name) {
        return Some(entry.layout);
    }
    custom::entry_by_name(name).map(|entry| entry.layout)
}
