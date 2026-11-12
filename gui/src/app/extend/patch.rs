//! 几何增量（patch）：只画与基准不同的项，避免 15 个参数全铺一屏。

use eframe::egui;

use numdeck::custom::{CustomLayoutSpec, LayoutPatch};
use numdeck::layout::{self, Layout};

use crate::theme;

/// 几何覆盖：`LayoutPatch` 的 15 项，逐项「勾选启用 + 数值」。
///
/// 贴纸 / 镭雕方案常常只动 `columns` 与四边内缩、键位照抄基准；界面原先只给键表
/// 不给这些，改完键表还得去手改 JSON，等于半个功能。
pub(crate) fn draw_one_patch(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    index: usize,
    patch: &mut LayoutPatch,
    base: Option<Layout>,
) {
    theme::block(
        ui,
        p,
        "几何覆盖",
        Some(
            "单位 0.1 mm，对应官方 AsusNUMPADFilter.inf 的 NUMPADLAYOUT_<机型>_00..14。\
             只有勾上的项才写进配置，其余沿用基准；不勾时右侧显示基准原值。",
        ),
        |ui| {
            let covered = patch_count(patch);
            let head = if covered == 0 {
                "未覆盖，全按基准几何".to_string()
            } else {
                format!("已覆盖 {covered} 项")
            };
            egui::CollapsingHeader::new(head)
                .id_salt(("custom_patch", index))
                .default_open(covered > 0)
                .show(ui, |ui| {
                    patch_section(ui, p, "键区");
                    patch_row(
                        ui,
                        p,
                        "列数",
                        "键区摊开成几列，1..=8。填了键表时列数以键表为准，这一项会被键表覆盖。",
                        base.map(|l| l.columns),
                        &mut patch.columns,
                        1..=8,
                    );
                    patch_row(
                        ui,
                        p,
                        "首列左留 _01",
                        "第 0 列左侧留白，越小键区越靠左。",
                        base.map(|l| l.index0),
                        &mut patch.index0,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "末列右留 _02",
                        "最后一列右侧留白；列数不是 5 时它压在末列上。",
                        base.map(|l| l.index4),
                        &mut patch.index4,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "首行上留 _03",
                        "第 0 行上侧留白。",
                        base.map(|l| l.level0),
                        &mut patch.level0,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "末行下留 _04",
                        "最后一行下侧留白。",
                        base.map(|l| l.level3),
                        &mut patch.level3,
                        0..=250,
                    );

                    patch_section(ui, p, "有效区内缩");
                    patch_row(
                        ui,
                        p,
                        "左 _07",
                        "有效区左边内缩，防止边缘误触。",
                        base.map(|l| l.left),
                        &mut patch.left,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "右 _08",
                        "有效区右边内缩。",
                        base.map(|l| l.right),
                        &mut patch.right,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "上 _09",
                        "有效区上边内缩。",
                        base.map(|l| l.up),
                        &mut patch.up,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "下 _10",
                        "有效区下边内缩。",
                        base.map(|l| l.down),
                        &mut patch.down,
                        0..=250,
                    );

                    patch_section(ui, p, "NumLock 热区");
                    patch_row(
                        ui,
                        p,
                        "Edge Index _05",
                        "NumLock 边缘的热区索引。当前实现不把它用于热区判定，按官方原值填即可。",
                        base.map(|l| l.numlock_index),
                        &mut patch.numlock_index,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "Edge Level _06",
                        "NumLock 边缘的热区高度。",
                        base.map(|l| l.numlock_level),
                        &mut patch.numlock_level,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "Adv Index _11",
                        "NumLock 扩展热区索引。",
                        base.map(|l| l.numadv_index),
                        &mut patch.numadv_index,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "Adv Level _12",
                        "NumLock 扩展热区高度。",
                        base.map(|l| l.numadv_level),
                        &mut patch.numadv_level,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "InValid Index _13",
                        "NumLock 无效区索引，决定「不算按到键」的那条边界。",
                        base.map(|l| l.invalid_index),
                        &mut patch.invalid_index,
                        0..=250,
                    );
                    patch_row(
                        ui,
                        p,
                        "InValid Level _14",
                        "NumLock 无效区高度。",
                        base.map(|l| l.invalid_level),
                        &mut patch.invalid_level,
                        0..=250,
                    );
                });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let has_base = base.is_some();
                let resp = ui.add_enabled(has_base, {
                    let size = egui::Vec2::new(theme::BTN_W, theme::CTRL_H);
                    egui::Button::new(egui::RichText::new("全部填入基准数值").color(p.text))
                        .fill(p.field)
                        .stroke(egui::Stroke::new(1.0, p.stroke))
                        .min_size(size)
                });
                let resp = theme::tip(
                    resp,
                    "把基准机型的 15 项原值一次性抄进配置，方便在此基础上逐项微调。",
                );
                if resp.clicked() {
                    if let Some(base) = base {
                        patch_fill_base(patch, base);
                    }
                }
                if covered > 0 {
                    let resp = ui.add({
                        let size = egui::Vec2::new(theme::BTN_W, theme::CTRL_H);
                        egui::Button::new(egui::RichText::new("清空覆盖").color(p.text))
                            .fill(p.field)
                            .stroke(egui::Stroke::new(1.0, p.stroke))
                            .min_size(size)
                    });
                    let resp = theme::tip(resp, "撤回这 15 项的全部覆盖，改回「全按基准」。");
                    if resp.clicked() {
                        *patch = LayoutPatch::default();
                    }
                }
            });
        },
    );
}

/// 几何区里的小标题：把 15 项分成键区 / 内缩 / 热区三组。
pub(crate) fn patch_section(ui: &mut egui::Ui, p: &theme::Palette, text: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(text)
            .size(11.5)
            .strong()
            .color(p.text_weak),
    );
    ui.add_space(2.0);
}

/// 一行几何参数：开关决定这写不写进配置，未勾时右侧灰字显示基准原值。
pub(crate) fn patch_row(
    ui: &mut egui::Ui,
    p: &theme::Palette,
    label: &str,
    help: &str,
    base: Option<i32>,
    field: &mut Option<i32>,
    range: std::ops::RangeInclusive<i32>,
) {
    ui.horizontal(|ui| {
        let mut on = field.is_some();
        let resp = theme::switch(ui, p, &mut on);
        let resp = theme::tip(resp, format!("勾上即写进配置。{help}"));
        if resp.changed() {
            *field = if on {
                Some(base.unwrap_or(*range.start()))
            } else {
                None
            };
        }
        ui.add_sized(
            egui::Vec2::new(theme::LABEL_W - 20.0, theme::CTRL_H),
            egui::Label::new(egui::RichText::new(label).size(12.0).color(p.text_weak)).truncate(),
        )
        .on_hover_text(help);
        match field.as_mut() {
            Some(value) => {
                ui.add_sized(
                    egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                    egui::DragValue::new(value).range(range.clone()),
                );
                match base {
                    Some(base) if base != *value => {
                        ui.label(
                            egui::RichText::new(format!("基准 {base}（{:+}）", *value - base))
                                .size(11.5)
                                .color(p.accent),
                        );
                    }
                    Some(base) => {
                        ui.label(
                            egui::RichText::new(format!("基准 {base}"))
                                .size(11.5)
                                .color(p.text_weak),
                        );
                    }
                    None => {}
                }
            }
            None => {
                let text = base
                    .map(|b| b.to_string())
                    .unwrap_or_else(|| "—".to_string());
                ui.add_sized(
                    egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                    egui::Label::new(
                        egui::RichText::new(text)
                            .size(12.0)
                            .monospace()
                            .color(p.text_weak),
                    ),
                );
            }
        }
    });
}

/// 已覆盖的项数，用于折叠标题上标一句「改了几项」。
pub(crate) fn patch_count(patch: &LayoutPatch) -> usize {
    [
        patch.columns,
        patch.index0,
        patch.index4,
        patch.level0,
        patch.level3,
        patch.numlock_index,
        patch.numlock_level,
        patch.left,
        patch.right,
        patch.up,
        patch.down,
        patch.numadv_index,
        patch.numadv_level,
        patch.invalid_index,
        patch.invalid_level,
    ]
    .iter()
    .filter(|v| v.is_some())
    .count()
}

/// 把基准布局的 15 项原值整组抄进覆盖表。
pub(crate) fn patch_fill_base(patch: &mut LayoutPatch, base: Layout) {
    patch.columns = Some(base.columns);
    patch.index0 = Some(base.index0);
    patch.index4 = Some(base.index4);
    patch.level0 = Some(base.level0);
    patch.level3 = Some(base.level3);
    patch.numlock_index = Some(base.numlock_index);
    patch.numlock_level = Some(base.numlock_level);
    patch.left = Some(base.left);
    patch.right = Some(base.right);
    patch.up = Some(base.up);
    patch.down = Some(base.down);
    patch.numadv_index = Some(base.numadv_index);
    patch.numadv_level = Some(base.numadv_level);
    patch.invalid_index = Some(base.invalid_index);
    patch.invalid_level = Some(base.invalid_level);
}

/// 布局列表里那些「装不上」的问题：空名、与内置重名、互相重名、HID 重复绑定。
///
/// 这些正是 `install_layouts` 会退回并写进校验提示的几种，提前在界面上标出来，
/// 省得存盘之后再去命令行对账。
pub(crate) fn layout_issues(specs: &[CustomLayoutSpec]) -> Vec<String> {
    let mut out = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let name = spec.name.trim();
        let tag = if name.is_empty() {
            format!("#{} 未命名", index + 1)
        } else {
            format!("#{} {name}", index + 1)
        };
        if name.is_empty() {
            out.push(format!("{tag}：布局名为空，这条不会被装上"));
        } else {
            if layout::builtin_by_name(name).is_some() {
                out.push(format!("{tag}：与内置布局重名，会被退回"));
            }
            let clash = specs
                .iter()
                .skip(index + 1)
                .any(|other| other.name.trim().eq_ignore_ascii_case(name));
            if clash {
                out.push(format!("{tag}：与后面的另一条自定义布局重名"));
            }
        }
        for hid in &spec.hid_ids {
            let hid = hid.trim().to_ascii_uppercase();
            if hid.is_empty() {
                continue;
            }
            let repeated = specs.iter().enumerate().any(|(other_index, other)| {
                other_index != index
                    && other
                        .hid_ids
                        .iter()
                        .any(|h| h.trim().eq_ignore_ascii_case(&hid))
            });
            if repeated {
                out.push(format!("{tag}：{hid} 同时绑在了多条自定义布局上"));
            }
        }
    }
    out
}
