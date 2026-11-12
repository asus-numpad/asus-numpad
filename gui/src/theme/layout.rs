//! 排版骨架：页面起始、卡片进出、行/块/单元格、控件列与成对控件。

use super::*;
use eframe::egui;

/// 每页开画时调用一次：按本页内容宽度定下控件列宽度。
pub(crate) fn begin_page(ui: &mut egui::Ui) {
    use std::sync::atomic::Ordering;
    // 控件列 = 卡片内宽 - 标签列 - 列间距，正好吃掉卡片右侧那段内边距：
    // 输入框、下拉框、档位条、按钮组右端于是都压在卡片右边这条竖线上。
    let content = ui.available_width() - CARD_PAD_X * 2.0;
    let width = (content - LABEL_W - ROW_GAP).max(CTRL_MIN_W);
    COLUMN_W_MILLI.store((width * 1000.0).round() as u32, Ordering::Relaxed);
}

/// 进卡时按本卡内宽换上控件列宽度，返回上一份以便出卡还原。
///
/// 控件右端必须落在**本卡**的右内边距上：页面级 [`begin_page`] 量的是滚动区宽度，
/// 与 `Frame` 扣完留白后的实宽能差几十像素，控件于是停在卡片右边之前一截，
/// 一页里输入框、下拉框、按钮组的右缘就长短不齐。这里进卡换值、出卡还原，
/// 并列卡片各页一致，嵌套卡片也不会把外层顶宽。
pub(crate) fn enter_card(card_inner_w: f32) -> u32 {
    use std::sync::atomic::Ordering;
    let prev = COLUMN_W_MILLI.load(Ordering::Relaxed);
    if card_inner_w.is_finite() && card_inner_w > 0.0 {
        let width = (card_inner_w - LABEL_W - ROW_GAP).max(CTRL_MIN_W);
        COLUMN_W_MILLI.store((width * 1000.0).round() as u32, Ordering::Relaxed);
    }
    prev
}

/// 出卡时把页面级控件列宽度换回去。
pub(crate) fn leave_card(prev: u32) {
    use std::sync::atomic::Ordering;
    COLUMN_W_MILLI.store(prev, Ordering::Relaxed);
}

/// 本页控件列宽度。
pub(crate) fn ctrl_width() -> f32 {
    use std::sync::atomic::Ordering;
    COLUMN_W_MILLI.load(Ordering::Relaxed) as f32 / 1000.0
}

/// 并排两控件时右边那个的宽度：控件列减掉左边那格与间距。
pub(crate) fn pair_b() -> f32 {
    ctrl_width() - PAIR_A - PAIR_GAP
}

/// 在行内切出一块**宽度精确**的格子，让内部内容照自己的布局画。
///
/// 不用 `allocate_ui_with_layout`：它按内容实际大小结算，标签只有两个字时那一格就
/// 只占两字的宽度，控件起点于是跟着往左跳（同一页里曾出现 492 / 514 / 520 三种起点）。
/// 这里先定死矩形、画完再把矩形整块交给布局，起点与宽度都不受内容影响。
pub(crate) fn cell<R>(
    ui: &mut egui::Ui,
    size: Vec2,
    layout: egui::Layout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let rect = egui::Rect::from_min_size(ui.next_widget_position(), size);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout));
    let out = add(&mut child);
    ui.allocate_rect(rect, Sense::hover());
    out
}

/// 同上，但高度按内容走（多行格子用）。
pub(crate) fn cell_auto_height<R>(
    ui: &mut egui::Ui,
    width: f32,
    layout: egui::Layout,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let start = ui.next_widget_position();
    let max = egui::Rect::from_min_size(start, Vec2::new(width, ui.available_height().max(CTRL_H)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(max).layout(layout));
    let out = add(&mut child);
    let height = child.min_rect().height().max(CTRL_H);
    ui.allocate_rect(
        egui::Rect::from_min_size(start, Vec2::new(width, height)),
        Sense::hover(),
    );
    out
}

/// 一行「标签 + 控件」：标签右对齐占固定宽度，控件从控件列起、宽度锁死为控件列宽。
///
/// 有没有 info 图标都预留同样的位置，控件再被塞进宽度正好是控件列的格子里，无论里面
/// 放下拉框、滑块还是数字框，左右边缘都落在同一条竖线上——调用点不用再各自 add_sized。
pub(crate) fn row<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        ctrl_cell(ui, add)
    })
    .inner
}

/// 与 [`row`] 相同，但控件右边再挂一句说明：说明从固定的说明列起排，
/// 于是整张卡片里所有说明文字的左边缘也是一条线。
pub(crate) fn row_hint<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    hint: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        let value = ctrl_cell(ui, add);
        hint_cell(ui, p, hint);
        value
    })
    .inner
}

/// 多行控件用的行：标签仍在左列，右侧格子的宽度同样是控件列，高度由内容决定。
///
/// 档位表、列表这类「一格里有好几行」的内容走这个，标签列与单行控件同样对齐。
pub(crate) fn block<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal_top(|ui| {
        label_cell(ui, p, label, help);
        cell_auto_height(
            ui,
            ctrl_width(),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                add(ui)
            },
        )
    })
    .inner
}

/// 没有标签的一行：内容缩进到控件列，左右边缘与其它带标签的行对齐。
///
/// 档位条、「执行 / 复制命令」这类整排控件没有标签，若直接从卡片左边起排，
/// 就会比别处整整左移一个标签列，看着像单独一栏。
pub(crate) fn ctrl_column<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.horizontal_top(|ui| {
        ui.add_space(LABEL_W + ROW_GAP);
        cell_auto_height(
            ui,
            ctrl_width(),
            egui::Layout::left_to_right(egui::Align::Center),
            add,
        )
    })
    .inner
}

/// 控件格：宽度锁死控件列，一行里的控件从同一竖线起、到同一竖线止。
pub(crate) fn ctrl_cell<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    cell(
        ui,
        Vec2::new(ctrl_width(), CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ctrl_spacing(ui);
            add(ui)
        },
    )
}

/// 说明格：控件右边那句话，左边缘与其它行的说明对齐。
pub(crate) fn hint_cell(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let width = ui.available_width().max(120.0);
    cell_auto_height(
        ui,
        width,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(
                egui::Label::new(RichText::new(text).size(12.0).color(p.text_weak))
                    .wrap()
                    .halign(egui::Align::Min),
            )
        },
    )
}

/// 把当前 `ui` 的控件尺寸统一到控件列宽 / [`CTRL_H`]。
///
/// 滑块要把「滑块长度 + 右侧数值框」凑成整列，所以这里给数值框留出 52 像素。
pub(crate) fn ctrl_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().combo_width = ctrl_width();
    ui.spacing_mut().slider_width = ctrl_width() - 52.0;
    ui.spacing_mut().interact_size.y = CTRL_H;
}

/// 单行输入框：宽度与行高都取控件列，和下拉框、滑块右边缘齐平。
pub(crate) fn text_field(ui: &mut egui::Ui, value: &mut String, hint: &str) -> Response {
    ui.add_sized(
        Vec2::new(ctrl_width(), CTRL_H),
        egui::TextEdit::singleline(value).hint_text(hint),
    )
}

/// 一行里并排两个控件：左边固定 [`PAIR_A`]，右边补到控件列剩下的宽度。
///
/// 「自动 + 数字框」「勾选框 + 数值」都用这个，右边缘因此不会比整宽控件短一截。
pub(crate) fn pair<A, B>(
    ui: &mut egui::Ui,
    left: impl FnOnce(&mut egui::Ui) -> A,
    right: impl FnOnce(&mut egui::Ui) -> B,
) -> (A, B) {
    let a = cell(
        ui,
        Vec2::new(PAIR_A, CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        left,
    );
    ui.add_space(PAIR_GAP);
    let b = cell(
        ui,
        Vec2::new(pair_b(), CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        right,
    );
    (a, b)
}

/// 等宽数字框：宽度取 [`pair_b`]，和 [`pair`] 右边的格子一样宽。
pub(crate) fn pair_num<Num: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    value: &mut Num,
    range: std::ops::RangeInclusive<Num>,
) -> Response {
    ui.add_sized(
        Vec2::new(pair_b(), CTRL_H),
        egui::DragValue::new(value).range(range),
    )
}

/// 档位表格子：每档一个数字框，四格一行铺满控件列，第二行的格子与第一行严格对齐。
///
/// 之前用的是 `horizontal_wrapped` + 固定 74 像素的框，档数一变就换行位置不定，
/// 表里表外的框宽还各不相同。
pub(crate) fn slot_grid(ui: &mut egui::Ui, p: &Palette, values: &mut [u8], low: u8, high: u8) {
    const COLS: usize = 4;
    let gap = 6.0;
    let slot = (ctrl_width() - gap * (COLS as f32 - 1.0)) / COLS as f32;
    for chunk_start in (0..values.len()).step_by(COLS) {
        let chunk_end = (chunk_start + COLS).min(values.len());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (offset, value) in values[chunk_start..chunk_end].iter_mut().enumerate() {
                let index = chunk_start + offset + 1;
                let resp = ui.add_sized(
                    Vec2::new(slot, CTRL_H),
                    egui::DragValue::new(value)
                        .range(low..=high)
                        .custom_formatter(|v, _| format!("0x{:02x}", v as u8)),
                );
                tip(
                    resp,
                    format!("第 {index} 档的命令字，取值 {low:#04x}..={high:#04x}"),
                );
            }
        });
    }
    let _ = p;
}

/// 一排按钮：整组铺满控件列，几颗就几等分，右端压在卡片的内右边缘上。
///
/// 早先定死一颗 [`BTN_W`]，一排三颗只占列宽一半，同一张卡片里输入框 636 宽、
/// 按钮组 328 宽，白色块「有长有短」正是这么来的。现在按钮组与输入框、下拉框同宽，
/// 一排内各颗按钮彼此等宽，右缘与整页控件列同一条竖线。
pub(crate) fn actions(
    ui: &mut egui::Ui,
    p: &Palette,
    items: &[(&str, bool, &str)],
) -> Option<usize> {
    let gap = ui.spacing().item_spacing.x;
    let count = items.len().max(1) as f32;
    // 单颗宽度不低于 [`BTN_W`]：数量少时不至于把按钮拉成一条长条，数量多时保证铺满。
    let btn_w = ((ctrl_width() - gap * (count - 1.0)) / count).max(BTN_W);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (index, (text, primary_style, help)) in items.iter().enumerate() {
            let button = if *primary_style {
                egui::Button::new(RichText::new(*text).color(Color32::WHITE).strong())
                    .fill(p.btn)
                    .stroke(Stroke::new(0.0, Color32::TRANSPARENT))
                    .corner_radius(CornerRadius::same(WIDGET_RADIUS))
                    .min_size(Vec2::new(btn_w, CTRL_H))
            } else {
                egui::Button::new(RichText::new(*text).color(p.text))
                    .fill(p.field)
                    .stroke(Stroke::new(1.0, p.stroke))
                    .corner_radius(CornerRadius::same(WIDGET_RADIUS))
                    .min_size(Vec2::new(btn_w, CTRL_H))
            };
            // 用 add_sized 给死宽高：`min_size` 只是下限，文字一长按钮就自己长胖，
            // 一排里几颗按钮于是宽窄不一、还把右边那颗顶出控件列。
            let resp = ui.add_sized(Vec2::new(btn_w, CTRL_H), button);
            wash(ui, &resp, p, CornerRadius::same(WIDGET_RADIUS));
            let resp = if help.is_empty() {
                resp
            } else {
                tip(resp, *help)
            };
            if resp.clicked() && clicked.is_none() {
                clicked = Some(index);
            }
        }
    });
    clicked
}

/// 「标签 + 一排按钮」：按钮组与下拉框、输入框同宽，右缘压在同一条竖线上。
pub(crate) fn action_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    help: Option<&str>,
    items: &[(&str, bool, &str)],
) -> Option<usize> {
    ui.horizontal(|ui| {
        label_cell(ui, p, label, help);
        ctrl_cell(ui, |ui| actions(ui, p, items))
    })
    .inner
}

/// 只出标签、不带控件：给那些自带 `ui.horizontal(|ui| { label; combo })` 的老布局就地补对齐。
///
/// 标签本身也挂悬浮说明——这类行没有可以放 `i` 的地方，鼠标停在标签上一样能看解释。
pub(crate) fn label_cell(ui: &mut egui::Ui, p: &Palette, text: &str, help: Option<&str>) {
    // 左对齐四线网格：图标槽位 / 标签文字 / 控件左缘 / 控件右缘，整页各成一条竖线。
    // 标签右对齐会让左缘随字数参差，看上去像「一堆宽度不一样的东西」，所以改成左对齐、
    // 说明图标统一放最左侧的图标槽里，没有说明的行也留同样宽的槽。
    cell(
        ui,
        Vec2::new(LABEL_W, CTRL_H),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            match help {
                Some(h) => {
                    info_dot(ui, p, h);
                }
                None => {
                    ui.add_space(16.0);
                }
            }
            let resp = ui.label(RichText::new(text).size(13.0).color(p.text_weak));
            if let Some(h) = help {
                tip(resp, h);
            }
        },
    );
}

/// 开关行：开关挪到控件列，和下拉框、输入框对齐，不再各自缩进。
pub(crate) fn check_row(
    ui: &mut egui::Ui,
    p: &Palette,
    checked: &mut bool,
    label: &str,
    help: Option<&str>,
) -> Response {
    row(ui, p, label, help, |ui| {
        let mut resp = switch(ui, p, checked);
        if let Some(text) = help {
            resp = tip(resp, text);
        }
        resp
    })
}

/// 只读行：左边标签、右边值。值右对齐到控件列右缘——一串路径、一个版本号，右对齐之后
/// 上下几行能对齐成一条线，读起来像一张表；过长先截断，悬浮看全文。
pub(crate) fn read_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    value: &str,
    help: Option<&str>,
) -> Response {
    let text = RichText::new(value)
        .size(12.0)
        .monospace()
        .color(p.text_weak);
    row(ui, p, label, help, |ui| {
        let resp = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add(egui::Label::new(text).truncate())
        });
        tip(resp.inner, value.to_owned())
    })
}

/// 说明文字：缩进到控件列，和上面的控件左缘对齐，不再贴着卡片左边堆成一片。
pub(crate) fn note(ui: &mut egui::Ui, p: &Palette, text: impl Into<String>) {
    let offset = LABEL_W + ui.spacing().item_spacing.x;
    ui.horizontal_top(|ui| {
        ui.add_space(offset);
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(RichText::new(text.into()).size(12.0).color(p.text_weak))
                    .wrap()
                    .halign(egui::Align::Min),
            );
        });
    });
}
