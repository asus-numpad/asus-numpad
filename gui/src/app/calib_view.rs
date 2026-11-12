//! 校准页的网格预览与配置体检。
//!
//! 分区与校准这两段全是坐标数字：四个角、行列数、权重、NumLock 格。纯文本输入时，
//! 人只能靠脑补判断「这一填画出来长什么样」，填错也看不出来——把毫米当原始值、x/y
//! 写反、列宽权重漏一个，界面上一律是四行数字，长得都对。
//!
//! 所以这页有两件配套的东西：
//!
//! - [`plate`]：按当前配置把网格画在触摸板全貌上，鼠标划过去就能读到「第几行第几列、
//!   格中心原始坐标是多少」，样本点也一起画上去，点偏了能一眼看到。
//! - [`lints`]：不依赖真机就能判定的问题，就地列出来，别等到保存后服务拒绝启动。
//!
//! 两者都走 [`numdeck::partition::PartitionSpec::resolve`]——与驱动判键时同一个
//! 解算函数，预览里看到什么，真机就是什么。

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Rect, Stroke, StrokeKind,
};

use numdeck::config::Config;
use numdeck::layout::{self as layout_mod, Geometry};
use numdeck::partition::Panel;

use crate::theme::{self, Palette};

/// 预览图底板的颜色：一定要比卡片暗一档，网格才像「从面板里抬起来」。
///
/// 深浅两套调色板里 `sunk` 的相对亮度正好相反（深色主题的 sunk 比卡片亮），拿它
/// 和卡片比一下就能判出主题，不必再给 Palette 加字段。
fn board_color(p: &Palette) -> Color32 {
    let lum = |c: Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
    if lum(p.sunk) < lum(p.card) {
        p.sunk
    } else {
        p.bg
    }
}

/// 预览图高度上限（逻辑像素）。到顶就收窄宽度，保住真实宽高比。
const MAX_PLATE_H: f32 = 200.0;

/// 网格解算结果，坐标值仍是触摸板原始值。
pub(super) struct Grid {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub nl: Option<(usize, usize)>,
    pub snap: bool,
    pub panel: Panel,
}

impl Grid {
    pub fn cols(&self) -> usize {
        self.x.len().saturating_sub(1)
    }

    pub fn rows(&self) -> usize {
        self.y.len().saturating_sub(1)
    }

    /// 落点属于哪一格；`snap` 打开时网格外吸附到最近一格，与驱动行为一致。
    fn cell_of(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let col = bracket(&self.x, x);
        let row = bracket(&self.y, y);
        match (col, row) {
            (Some(col), Some(row)) => Some((row, col)),
            _ if self.snap && self.rows() > 0 && self.cols() > 0 => {
                Some((nearest(&self.y, y), nearest(&self.x, x)))
            }
            _ => None,
        }
    }

    /// 格宽 / 格高的平均值，给「格 720x550」这类提示用。
    /// 格宽 / 格高的平均值，给「格 720x550」这类提示用。
    ///
    /// 用首尾边界相减而不是 `x[cols()]`：网格退化到 0 或 1 条边界时也返回 0，
    /// 不会越界。除法前保证除数至少为 1。
    pub fn cell_size(&self) -> (f64, f64) {
        let span = |edges: &[f64]| -> f64 {
            let (Some(first), Some(last)) = (edges.first(), edges.last()) else {
                return 0.0;
            };
            let n = edges.len().saturating_sub(1).max(1) as f64;
            (last - first) / n
        };
        (span(&self.x), span(&self.y))
    }
}

fn bracket(edges: &[f64], v: f64) -> Option<usize> {
    if edges.len() < 2 || v < *edges.first()? || v > *edges.last()? {
        return None;
    }
    // 落在内部边界上归左边的那一格（与驱动 `cell_of` 同侧包含规则）
    Some(
        edges
            .windows(2)
            .position(|w| v >= w[0] && v < w[1])
            .unwrap_or(edges.len().saturating_sub(2)),
    )
}

fn nearest(edges: &[f64], v: f64) -> usize {
    let n = edges.len().saturating_sub(1);
    (0..n)
        .min_by(|a, b| {
            let da = (v - (edges[*a] + edges[a + 1]) / 2.0).abs();
            let db = (v - (edges[*b] + edges[b + 1]) / 2.0).abs();
            da.total_cmp(&db)
        })
        .unwrap_or(0)
}

/// 与 CLI `check` 同一套虚拟面板：没有真机时按配置里的坐标把面板撑够大。
///
/// 撑开而不是夹回的意义在于：mac 上也能看到「我填的这块矩形画出来是几行几列」，
/// 真机上超范围会被驱动拒绝，那是另一回事。
fn preview_panel(cfg: &Config) -> Panel {
    let (mut side_x, mut side_y) = (1000i32, 1000i32);
    let grow = |r: [f64; 4], sx: &mut i32, sy: &mut i32| {
        if r[2].is_finite() && r[2] > *sx as f64 {
            *sx = (r[2] * 1.05).ceil() as i32;
        }
        if r[3].is_finite() && r[3] > *sy as f64 {
            *sy = (r[3] * 1.05).ceil() as i32;
        }
    };
    if let Some(rect) = cfg.partition.rect {
        grow(rect, &mut side_x, &mut side_y);
    }
    if let Some(mm) = cfg.partition.rect_mm {
        // 毫米先按当前面板换算成原始值，超出再撑——撑开后面板仍按 width_mm 铺满，
        // 毫米到原始值的比例不变，矩形不会跟着漂。
        let ux = side_x as f64 / (cfg.touchpad_width_mm * 1000.0).max(1.0);
        let uy = side_y as f64 / (cfg.touchpad_height_mm * 1000.0).max(1.0);
        grow(
            [
                mm[0] * 1000.0 * ux,
                mm[1] * 1000.0 * uy,
                mm[2] * 1000.0 * ux,
                mm[3] * 1000.0 * uy,
            ],
            &mut side_x,
            &mut side_y,
        );
    }
    Panel {
        xmin: 0,
        xmax: side_x,
        ymin: 0,
        ymax: side_y,
        ux: side_x as f64 / (cfg.touchpad_width_mm * 1000.0).max(1.0),
        uy: side_y as f64 / (cfg.touchpad_height_mm * 1000.0).max(1.0),
    }
}

/// 触摸板物理尺寸（毫米），用来把握预览图的宽高比。
fn panel_mm(cfg: &Config) -> (f64, f64) {
    // 物理尺寸是用户自己填的真值，面板的 units_per_mm 反倒常是错的（表外机型八成
    // 报不准，这正是要校准的原因）。所以画图按配置里的宽高，别拿校准前的换算值
    // 反推——那会把图拉成 2.3:1 的扁条，看着就不像触摸板。
    let (w, h) = (cfg.touchpad_width_mm, cfg.touchpad_height_mm);
    let sane = (40.0..=400.0).contains(&w) && (30.0..=400.0).contains(&h);
    if sane {
        (w, h)
    } else {
        (130.0, 80.0)
    }
}

fn key_dims(cfg: &Config) -> (usize, usize) {
    let table = cfg.key_table();
    // Geometry 恒定按官方 4 行切网格（`KeyTable::flatten` 也按这个补齐）。键表只
    // 写了 3 行时，真实机器第 4 行仍在——预览跟着画 4 行，别少画一行骗人。
    (
        table.cols().max(1),
        table.rows().len().max(layout_mod::ROWS),
    )
}

/// 解算当前配置的网格。
pub(super) fn resolve(cfg: &Config) -> Result<Grid, String> {
    let (panel, _) = cfg.calibration.apply(preview_panel(cfg));
    let keys = key_dims(cfg);
    let base = Geometry::new(
        panel.xmin,
        panel.xmax,
        panel.ymin,
        panel.ymax,
        panel.ux,
        panel.uy,
        cfg.layout(),
    );
    let (x0, y0, x1, y1) = base.grid_rect();
    let unset = cfg.partition.is_unset() || !cfg.partition.applies_to(cfg.layout_entry().name);
    if unset {
        // 没有自定义分区：官方网格按键表等分，预览也照这个画，好让人知道「不改长什么样」
        return Ok(Grid {
            x: split_even(x0, x1, keys.0),
            y: split_even(y0, y1, keys.1),
            nl: Some((0, keys.0 - 1)),
            snap: false,
            panel,
        });
    }
    let resolved = cfg.partition.resolve(panel, (x0, y0, x1, y1), keys)?;
    Ok(Grid {
        x: resolved.x_edges,
        y: resolved.y_edges,
        nl: resolved.nl_cell,
        snap: resolved.snap_outside,
        panel,
    })
}

fn split_even(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let n = n.max(1);
    (0..=n)
        .map(|i| lo + (hi - lo) * i as f64 / n as f64)
        .collect()
}

/// 样本点：原始坐标 + 它应该落在哪一格（由界面按 corners / first / last 语义给出）。
pub(super) struct Probe {
    pub raw: (f64, f64),
    pub want: Option<(usize, usize)>,
    pub label: &'static str,
}

/// 悬停读数。
pub(super) struct Hover {
    pub raw: (f64, f64),
    pub cell: Option<(usize, usize)>,
}

/// 把网格画在触摸板全貌上。
///
/// 返回悬停读数，调用方拿去拼一行文字；解算失败时画一块红框并返回错误文案，
/// 比在卡片末尾补一行「配置有问题」直观得多——眼睛看着图，同时知道图为什么是错的。
pub(super) fn plate(
    ui: &mut egui::Ui,
    p: &Palette,
    cfg: &Config,
    probes: &[Probe],
) -> (Option<Hover>, Option<String>) {
    let grid = match resolve(cfg) {
        Ok(grid) => grid,
        Err(err) => {
            failed_plate(ui, p, &err);
            return (None, Some(err));
        }
    };
    // 比例必须是真的：把触摸板压成 3:1 的扁条，网格就不再反映手指要走多远。
    // 所以先按比例算高度，太高时反过来收窄宽度（左对齐，不居中——左边要和
    // 上面那些控件压同一条线）。
    let (mm_w, mm_h) = panel_mm(cfg);
    let ratio = (mm_h / mm_w) as f32;
    // 宽度锁控件列：别的行都是这个宽度，图跟着它，右缘自然压在同一条竖线上
    let available = theme::ctrl_width().max(200.0);
    let height = available * ratio;
    let width = if height > MAX_PLATE_H {
        (MAX_PLATE_H / ratio).max(160.0)
    } else {
        available
    };
    let height = (width * ratio).max(64.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), egui::Sense::hover());
    let resp = resp.on_hover_cursor(egui::CursorIcon::Crosshair);
    let painter = ui.painter_at(rect);

    let span_x = (grid.panel.xmax - grid.panel.xmin).max(1) as f64;
    let span_y = (grid.panel.ymax - grid.panel.ymin).max(1) as f64;
    let map_x =
        |x: f64| rect.left() + ((x - grid.panel.xmin as f64) / span_x) as f32 * rect.width();
    let map_y =
        |y: f64| rect.top() + ((y - grid.panel.ymin as f64) / span_y) as f32 * rect.height();

    // 触摸板全貌：比卡片凹一档的底色，网格区再抬起来，一眼分得清「板」和「网格」
    painter.rect_filled(rect, CornerRadius::same(8), board_color(p));
    // 底板留一道细边：光靠明暗差，深色主题下这块会像浮在卡片上
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, p.stroke),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, p.stroke),
        StrokeKind::Inside,
    );

    let hover = resp.hover_pos().map(|pos| {
        let rx = grid.panel.xmin as f64
            + (pos.x - rect.left()) as f64 / rect.width().max(1.0) as f64 * span_x;
        let ry = grid.panel.ymin as f64
            + (pos.y - rect.top()) as f64 / rect.height().max(1.0) as f64 * span_y;
        Hover {
            raw: (rx, ry),
            cell: grid.cell_of(rx, ry),
        }
    });

    // 格子里写上键名：一张只画方格子的图看不出这格到底出什么键，校准的人
    // 得回头对键表，等于白画。
    let table = cfg.key_table();
    for row in 0..grid.rows() {
        for col in 0..grid.cols() {
            let cell = Rect::from_min_max(
                pos2(map_x(grid.x[col]), map_y(grid.y[row])),
                pos2(map_x(grid.x[col + 1]), map_y(grid.y[row + 1])),
            )
            .shrink(1.0);
            if cell.width() < 2.0 || cell.height() < 2.0 {
                continue;
            }
            let is_nl = grid.nl == Some((row, col));
            let hovered = hover.as_ref().and_then(|h| h.cell) == Some((row, col));
            // 先铺一档亮色，NumLock 格再叠半透明强调色：压在卡片底色上会和卡片
            // 混成一片，网格就只剩描边，看着像一块空洞。
            painter.rect_filled(cell, CornerRadius::same(4), p.field);
            if is_nl {
                painter.rect_filled(cell, CornerRadius::same(4), p.accent_soft);
            }
            let (stroke_w, stroke_color) = if hovered {
                (2.0, p.accent)
            } else if is_nl {
                (1.0, p.accent)
            } else {
                (1.0, p.stroke)
            };
            painter.rect_stroke(
                cell,
                CornerRadius::same(4),
                Stroke::new(stroke_w, stroke_color),
                StrokeKind::Inside,
            );
            if cell.width() > 30.0 && cell.height() > 18.0 {
                // NumLock 格一律写 NL：键表里那格可能是 none（布局没占满 4 行），
                // 照样要点出「这一格是切换点」，否则一块亮起来的空格子很莫名其妙。
                let (text, color) = if is_nl {
                    ("NL".to_string(), p.accent)
                } else {
                    match table.get(row, col) {
                        Some(slot) => (slot.label(), p.text),
                        None => ("—".to_string(), p.text_weak),
                    }
                };
                if !text.is_empty() {
                    let size = if cell.width() < 46.0 { 9.5 } else { 11.0 };
                    let text = if text.chars().count() > 6 {
                        text.chars().take(6).collect::<String>()
                    } else {
                        text
                    };
                    painter.text(
                        cell.center(),
                        Align2::CENTER_CENTER,
                        text,
                        FontId::proportional(size),
                        color,
                    );
                }
            }
        }
    }

    // 样本点：实心点 + 外圈，落在哪一格由调用方判对错
    for probe in probes {
        let at = pos2(map_x(probe.raw.0), map_y(probe.raw.1));
        if !rect.contains(at) {
            continue;
        }
        painter.circle_filled(at, 3.0, p.ok);
        painter.circle_stroke(at, 6.0, Stroke::new(1.0, p.ok));
        painter.text(
            at + vec2(9.0, -9.0),
            Align2::LEFT_BOTTOM,
            probe.label,
            FontId::proportional(10.0),
            p.text_weak,
        );
    }

    // 角标：把网格矩形的两个角写在图上，省得回头再去数输入框里的数字
    let (x0, y0) = (grid.x[0], grid.y[0]);
    let (x1, y1) = (grid.x[grid.cols()], grid.y[grid.rows()]);
    painter.text(
        pos2(map_x(x0), map_y(y0)) + vec2(2.0, -3.0),
        Align2::LEFT_BOTTOM,
        format!("{x0:.0}, {y0:.0}"),
        FontId::proportional(10.0),
        p.text_weak,
    );
    painter.text(
        pos2(map_x(x1), map_y(y1)) + vec2(-2.0, 3.0),
        Align2::RIGHT_TOP,
        format!("{x1:.0}, {y1:.0}"),
        FontId::proportional(10.0),
        p.text_weak,
    );

    (hover, None)
}

/// 解算失败时画的那块红框：错误原因直接写在图上。
pub(super) fn failed_plate(ui: &mut egui::Ui, p: &Palette, err: &str) {
    let width = ui.available_width().max(200.0);
    let (rect, _) = ui.allocate_exact_size(vec2(width, 96.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(8), p.sunk);
    painter.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, p.bad),
        StrokeKind::Inside,
    );
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "分区无法解算",
        FontId::proportional(12.5),
        p.bad,
    );
    painter.text(
        rect.center() + vec2(0.0, 18.0),
        Align2::CENTER_CENTER,
        err,
        FontId::proportional(10.5),
        p.text_weak,
    );
}

/// 体检项的严重程度。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Level {
    /// 保存后驱动会拒收或整段忽略，必须改
    Error,
    /// 能生效但多半不是本意，值得看一眼
    Warn,
}

pub(super) struct Lint {
    pub level: Level,
    pub text: String,
}

impl Lint {
    fn error(text: impl Into<String>) -> Self {
        Self {
            level: Level::Error,
            text: text.into(),
        }
    }

    fn warn(text: impl Into<String>) -> Self {
        Self {
            level: Level::Warn,
            text: text.into(),
        }
    }
}

/// 只靠配置本身就能判定的问题。
///
/// 判据与 [`numdeck::partition::PartitionSpec::resolve`] / `sanitize` 一致，
/// 但不依赖真机范围：真机上「矩形越界」由驱动拒绝，这里管的是填错、写反、漏一个。
pub(super) fn lints(cfg: &Config) -> Vec<Lint> {
    let mut out = Vec::new();
    let spec = &cfg.partition;
    let keys = key_dims(cfg);

    if !spec.is_unset() && !spec.applies_to(cfg.layout_entry().name) {
        out.push(Lint::warn(format!(
            "分区只对布局「{}」生效，当前生效布局是「{}」，改了也不会应用。",
            spec.layout,
            cfg.layout_entry().name
        )));
    }

    if spec.rect.is_some() && spec.rect_mm.is_some() {
        out.push(Lint::warn(
            "矩形与毫米矩形都填了：原始值优先，毫米那段整段不生效。",
        ));
    }

    if let Some(rect) = spec.rect {
        if !rect.iter().all(|v| v.is_finite()) {
            out.push(Lint::error("矩形里有非数字的值。"));
        } else if rect[2] <= rect[0] || rect[3] <= rect[1] {
            out.push(Lint::error(format!(
                "矩形右下角必须大于左上角，现在是 [{:.0}, {:.0}] .. [{:.0}, {:.0}]。",
                rect[0], rect[1], rect[2], rect[3]
            )));
        } else {
            let (w, h) = (rect[2] - rect[0], rect[3] - rect[1]);
            let ratio = w / h;
            if !(0.8..=3.2).contains(&ratio) {
                out.push(Lint::warn(format!(
                    "网格宽高比 {ratio:.2}:1，触摸板通常接近 1.6:1——确认 x/y 两个坐标没写反。"
                )));
            }
        }
    }

    if let Some(mm) = spec.rect_mm {
        if mm[2] <= mm[0] || mm[3] <= mm[1] {
            out.push(Lint::error("毫米矩形的右下角必须大于左上角。"));
        }
    }

    for (name, cols) in [("列数", spec.cols), ("行数", spec.rows)] {
        if let Some(n) = cols {
            if !(1..=8).contains(&n) {
                out.push(Lint::error(format!("{name}必须在 1..=8，现在是 {n}。")));
            }
        }
    }

    let cols = spec.cols.unwrap_or(keys.0);
    let rows = spec.rows.unwrap_or(keys.1);
    for (name, ratios, n) in [
        ("col_ratios", &spec.col_ratios, cols),
        ("row_ratios", &spec.row_ratios, rows),
    ] {
        if ratios.is_empty() {
            continue;
        }
        if ratios.len() != n {
            out.push(Lint::error(format!(
                "{name} 有 {} 个值，但网格是 {n} 份——要么补齐，要么整段留空走等分。",
                ratios.len()
            )));
        } else if ratios.iter().any(|r| !(r.is_finite() && *r > 0.0)) {
            out.push(Lint::error(format!("{name} 必须全部为正数。")));
        }
    }

    if let Some([row, col]) = spec.numlock_cell {
        if row >= rows || col >= cols {
            out.push(Lint::error(format!(
                "NumLock 格 ({row}, {col}) 超出 {rows} 行 x {cols} 列。"
            )));
        }
    }

    if cols > keys.0 {
        out.push(Lint::warn(format!(
            "网格 {cols} 列多于键表的 {} 列：多出来的列不会出键。",
            keys.0
        )));
    } else if cols < keys.0 {
        out.push(Lint::warn(format!(
            "网格 {cols} 列少于键表的 {} 列：右边 {} 列键没有落点。",
            keys.0,
            keys.0 - cols
        )));
    }
    if rows != keys.1 {
        out.push(Lint::warn(format!(
            "网格 {rows} 行与键表的 {} 行不一致，键会按重叠部分映射。",
            keys.1
        )));
    }

    for (axis, cal) in [("X", &cfg.calibration.x), ("Y", &cfg.calibration.y)] {
        if let (Some(lo), Some(hi)) = (cal.min, cal.max) {
            if lo >= hi {
                out.push(Lint::error(format!(
                    "{axis} 轴范围写反了（min {lo} ≥ max {hi}），加载时整轴会被丢掉。"
                )));
            }
        }
        if let Some(units) = cal.units_per_mm {
            if !(units.is_finite() && units > 0.0) {
                out.push(Lint::error(format!(
                    "{axis} 轴分辨率 {units} 不是正数，加载时会被丢掉。"
                )));
            }
        }
    }

    out
}

/// 页首那条状态：一眼看出「能不能存」。
pub(super) fn status_row(ui: &mut egui::Ui, p: &Palette, cfg: &Config) {
    let lints = lints(cfg);
    let errors = lints.iter().filter(|l| l.level == Level::Error).count();
    let warns = lints.len() - errors;

    ui.horizontal(|ui| {
        if errors == 0 && warns == 0 {
            theme::pill(ui, "自检通过", p.ok);
            ui.label(
                egui::RichText::new("分区与校准没有发现明显问题")
                    .size(12.0)
                    .color(p.text_weak),
            );
        } else {
            if errors > 0 {
                theme::pill(ui, &format!("{errors} 项待修正"), p.bad);
            }
            if warns > 0 {
                theme::pill(ui, &format!("{warns} 项留意"), p.warn);
            }
            if let Some(first) = lints.iter().find(|l| l.level == Level::Error) {
                ui.label(
                    egui::RichText::new(format!("最先要改的是：{}", first.text))
                        .size(12.0)
                        .color(p.text_weak),
                );
            }
        }
    });
}

/// 体检明细：错误在前、告警在后，最多列 [`LIMIT`] 条，免得把卡片撑成一条长龙。
const LIMIT: usize = 4;

pub(super) fn lint_notes(ui: &mut egui::Ui, p: &Palette, cfg: &Config) {
    let lints = lints(cfg);
    if lints.is_empty() {
        return;
    }
    let mut sorted: Vec<&Lint> = lints.iter().filter(|l| l.level == Level::Error).collect();
    sorted.extend(lints.iter().filter(|l| l.level == Level::Warn));
    for lint in sorted.iter().take(LIMIT) {
        let (mark, color) = match lint.level {
            Level::Error => ("需要修正", p.bad),
            Level::Warn => ("留意", p.warn),
        };
        ui.horizontal_top(|ui| {
            theme::pill(ui, mark, color);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(&lint.text)
                        .size(12.0)
                        .color(p.text_weak),
                )
                .wrap(),
            );
        });
    }
    if sorted.len() > LIMIT {
        ui.label(
            egui::RichText::new(format!(
                "另有 {} 项，都在本节各行的说明里。",
                sorted.len() - LIMIT
            ))
            .size(11.5)
            .color(p.text_weak),
        );
    }
}

/// 预览图下方那行读数：没有悬停时说网格概况，有悬停时说到格。
pub(super) fn readout(
    p: &Palette,
    grid: Option<&Grid>,
    hover: Option<&Hover>,
) -> (String, Color32) {
    let Some(grid) = grid else {
        return ("先修好上面的问题，这里才能画图。".to_string(), p.text_weak);
    };
    // 一律换算成毫米：units 是驱动内部坐标，对用户没有意义。
    // `Panel::ux/uy` 是 units per µm，换算成毫米要再乘 1000。
    let mm = |raw: f64, origin: f64, per_um: f64| {
        if per_um > 0.0 {
            (raw - origin) / (per_um * 1000.0)
        } else {
            0.0
        }
    };
    let (cw, ch) = grid.cell_size();
    let (cw_mm, ch_mm) = (mm(cw, 0.0, grid.panel.ux), mm(ch, 0.0, grid.panel.uy));
    match hover {
        Some(h) => match h.cell {
            Some((row, col)) => {
                let nl = if grid.nl == Some((row, col)) {
                    " · NumLock 格"
                } else {
                    ""
                };
                (
                    format!(
                        "第 {} 行第 {} 列{nl} · 距左 {:.0} mm，距上 {:.0} mm",
                        row + 1,
                        col + 1,
                        mm(h.raw.0, grid.panel.xmin as f64, grid.panel.ux),
                        mm(h.raw.1, grid.panel.ymin as f64, grid.panel.uy),
                    ),
                    p.text_weak,
                )
            }
            None => (
                "这块不在网格里，落在触摸板的光标区，不产生按键。".to_string(),
                p.warn,
            ),
        },
        None if cw_mm > 0.1 && ch_mm > 0.1 => (
            format!(
                "{} 行 × {} 列 · 每格约 {cw_mm:.0} × {ch_mm:.0} mm · 移上去看落点",
                // 网格退化（没有可用边界）时仍然报 1 行 1 列，不显示「0 行」
                grid.rows().max(1),
                grid.cols().max(1)
            ),
            p.text_weak,
        ),
        None => (
            format!("{} 行 × {} 列", grid.rows(), grid.cols()),
            p.text_weak,
        ),
    }
}

/// 样本点落格对错：解算只保证矩形，落到哪一格还得自己看一眼。
pub(super) fn probe_check(grid: &Grid, probes: &[Probe]) -> Vec<String> {
    let mut out = Vec::new();
    for probe in probes {
        let Some(want) = probe.want else { continue };
        match grid.cell_of(probe.raw.0, probe.raw.1) {
            Some(got) if got == want => {}
            Some(got) => out.push(format!(
                "{} 应落在第 {} 行第 {} 列，实际落在第 {} 行第 {} 列。",
                probe.label,
                want.0 + 1,
                want.1 + 1,
                got.0 + 1,
                got.1 + 1
            )),
            None => out.push(format!("{} 落在网格外。", probe.label)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_with_mm(w: f64, h: f64) -> Config {
        Config {
            touchpad_width_mm: w,
            touchpad_height_mm: h,
            ..Config::default()
        }
    }

    /// 预览图的长宽比只能来自配置里的物理尺寸；面板的 units/mm 报不准才是常态。
    #[test]
    fn plate_ratio_follows_config_mm() {
        assert_eq!(panel_mm(&cfg_with_mm(130.0, 80.0)), (130.0, 80.0));
        assert_eq!(panel_mm(&cfg_with_mm(200.0, 120.0)), (200.0, 120.0));
    }

    /// 毫米值填坏时兜底成常见触摸板，绝不能拿坏值画出一条扁条。
    #[test]
    fn plate_ratio_falls_back_on_bad_mm() {
        for (w, h) in [(0.0, 0.0), (5.0, 80.0), (130.0, 0.0), (9000.0, 80.0)] {
            assert_eq!(panel_mm(&cfg_with_mm(w, h)), (130.0, 80.0), "{w}x{h}");
        }
    }

    /// 键表只写了 3 行时，机器上第 4 行仍然在，预览不能少画一行。
    #[test]
    fn key_dims_never_below_four_rows() {
        let cfg = Config::default();
        let (cols, rows) = key_dims(&cfg);
        assert!(rows >= layout_mod::ROWS, "rows = {rows}");
        assert!(cols >= 1);
    }

    /// 没有自定义分区时，预览按官方网格等分，且整块落在面板之内。
    #[test]
    fn resolve_even_grid_stays_inside_panel() {
        let cfg = Config::default();
        let grid = resolve(&cfg).expect("默认配置应当能解算");
        assert_eq!(grid.rows(), key_dims(&cfg).1);
        assert_eq!(grid.cols(), key_dims(&cfg).0);
        for x in &grid.x {
            assert!(*x >= grid.panel.xmin as f64 && *x <= grid.panel.xmax as f64);
        }
        for y in &grid.y {
            assert!(*y >= grid.panel.ymin as f64 && *y <= grid.panel.ymax as f64);
        }
        assert_eq!(grid.nl, Some((0, grid.cols() - 1)));
    }

    /// 矩形写反了必须点名报错，别让用户对着一张空白图猜。
    #[test]
    fn lints_catch_reversed_rect() {
        let mut cfg = Config::default();
        cfg.partition = numdeck::partition::PartitionSpec {
            rect: Some([900.0, 800.0, 100.0, 50.0]),
            ..cfg.partition
        };
        let errs: Vec<_> = lints(&cfg)
            .into_iter()
            .filter(|l| l.level == Level::Error)
            .collect();
        assert!(
            errs.iter().any(|l| l.text.contains("右下角")),
            "{:?}",
            errs.iter().map(|l| &l.text).collect::<Vec<_>>()
        );
    }

    /// 行列数越界同样要点名。
    #[test]
    fn lints_catch_out_of_range_cols() {
        let mut cfg = Config::default();
        cfg.partition = numdeck::partition::PartitionSpec {
            cols: Some(99),
            ..cfg.partition
        };
        let errs: Vec<_> = lints(&cfg)
            .into_iter()
            .filter(|l| l.level == Level::Error)
            .collect();
        assert!(
            errs.iter().any(|l| l.text.contains("列数")),
            "{:?}",
            errs.iter().map(|l| &l.text).collect::<Vec<_>>()
        );
    }

    /// 网格退化（空边界 / 只有一条边界）时，取格尺寸与落点判定都不能越界 panic。
    #[test]
    fn degenerate_grid_never_panics() {
        let panel = Panel {
            xmin: 0,
            xmax: 200,
            ymin: 0,
            ymax: 100,
            ux: 0.0077,
            uy: 0.0125,
        };
        for (x, y) in [
            (vec![], vec![]),
            (vec![0.0], vec![0.0]),
            (vec![0.0, 100.0], vec![0.0]),
        ] {
            let grid = Grid {
                x: x.clone(),
                y: y.clone(),
                nl: None,
                snap: true,
                panel,
            };
            let (w, h) = grid.cell_size();
            assert!(w.is_finite() && h.is_finite(), "{x:?} {y:?} → {w}x{h}");
            // 退化时 cols/rows 为 0，snap 分支也不能返回会被拿去索引的坐标
            for raw in [(0.0, 0.0), (50.0, 50.0), (-10.0, 500.0)] {
                if let Some((row, col)) = grid.cell_of(raw.0, raw.1) {
                    assert!(row < grid.rows().max(1) && col < grid.cols().max(1));
                }
            }
        }
    }

    /// 样本落错格要报出来：解算只保证矩形，落哪格得自己看一眼。
    #[test]
    fn probe_check_reports_wrong_cell() {
        let grid = Grid {
            x: vec![0.0, 100.0, 200.0],
            y: vec![0.0, 100.0],
            nl: Some((0, 1)),
            snap: false,
            panel: Panel {
                xmin: 0,
                xmax: 200,
                ymin: 0,
                ymax: 100,
                ux: 0.0077,
                uy: 0.0125,
            },
        };
        let probes = vec![
            Probe {
                raw: (50.0, 50.0),
                want: Some((0, 0)),
                label: "左上角",
            },
            Probe {
                raw: (150.0, 50.0),
                want: Some((0, 0)),
                label: "右下角",
            },
            Probe {
                raw: (300.0, 50.0),
                want: Some((0, 1)),
                label: "板外",
            },
        ];
        let notes = probe_check(&grid, &probes);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].contains("右下角") && notes[0].contains("第 1 行第 2 列"));
        assert!(notes[1].contains("板外"));
    }
}
