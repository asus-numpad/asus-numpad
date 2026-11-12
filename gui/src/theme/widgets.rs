//! 通用控件：卡片、标题、主次按钮、开关、胶囊、分段选择、电平条。

use super::*;
use eframe::egui;

/// 一张卡片：大圆角、宽留白，浅色下靠投影和底色分层，深色下再补一圈极淡描边。
///
/// 浅色卡片**不画描边**：白卡 + 1px 灰线是上一版的观感问题来源——线在缩放和高分屏上
/// 一半糊一半锐，看着像表格线；改用一层柔和大半径投影（`blur 18`）后边界由明度差交代，
/// 干净得多。深色底上投影几乎不可见，所以那一套保留 1px 描边来托边界。
pub(crate) fn card<R>(ui: &mut egui::Ui, p: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let dark = p.bg.to_array()[0] < 128;
    let frame = egui::Frame::new()
        .fill(p.card)
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(Margin {
            left: 20,
            right: 20,
            top: 18,
            bottom: 18,
        })
        .shadow(egui::epaint::Shadow {
            offset: [0, 2],
            blur: if dark { 12 } else { 22 },
            spread: 0,
            color: Color32::from_black_alpha(if dark { 90 } else { 26 }),
        });
    let frame = if dark {
        frame.stroke(Stroke::new(1.0, p.stroke))
    } else {
        frame.stroke(Stroke::new(0.0, Color32::TRANSPARENT))
    };
    let mut prev_column = 0_u32;
    let out = frame
        .show(ui, |ui| {
            // 卡片宽度一律吃满内容列，不按内容收缩：`Frame` 默认是「有多大内容就多宽」，
            // 于是「机型与布局」这种窄卡片右边缘停在 1030，「命令行」那种宽内容撑到
            // 1159，一页里几个圆角矩形右边缘参差不齐。宽度交给页面统一规定。
            let w = ui.available_width();
            if w.is_finite() {
                ui.set_width(w);
            }
            // 卡内所有表单行都照本卡内宽排，控件右端因此压在卡片右内边距这条线上，
            // 与卡片宽度同源，不会再出现「卡片 800、里面控件只到 720」。
            prev_column = enter_card(w);
            // 卡片里不缩进：折叠头、列表这些默认会按 indent 往右推，一推控件列就跟着
            // 偏移，同一份表单在「折叠卡片」和「带标题卡片」里就错开十几像素。
            ui.spacing_mut().indent = 0.0;
            add(ui)
        })
        .inner;
    leave_card(prev_column);
    out
}

/// 带标题的卡片。
pub(crate) fn section<R>(
    ui: &mut egui::Ui,
    p: &Palette,
    title: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    card(ui, p, |ui| {
        title_row(ui, p, title);
        ui.add_space(4.0);
        add(ui)
    })
}

/// 卡片标题：一行粗体标题，下面一条**横贯整张卡片**的细线。
///
/// 上一版是「标题 + 拖到右缘的尾线」，十几个卡片排下来，右侧全是长短不一的横线，
/// 反而显得琐碎。改成标题独立成行、分隔线走满卡片宽度（左右各伸出内边距，
/// 直接贴到卡片边缘），层次更像系统设置面板：标题区与内容区一刀切开，读者不会
/// 把标题误当成第一行内容。
pub(crate) fn title_row(ui: &mut egui::Ui, p: &Palette, title: &str) {
    title_row_help(ui, p, title, None);
}

/// 同上，但标题右边还能挂一个 `i`。
///
/// 区块级的说明（这一栏是什么、为什么会有好几个条目）塞不进任何一行参数里，
/// 挂在标题上最省地方，也不会和参数行的 `i` 挤在同一列。
pub(crate) fn title_row_help(ui: &mut egui::Ui, p: &Palette, title: &str, help: Option<&str>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(14.5).strong().color(p.text));
        if let Some(help) = help {
            info_dot(ui, p, help);
        }
    });
    let r = ui.max_rect();
    let y = ui.cursor().top() - ui.spacing().item_spacing.y * 0.5;
    ui.painter().hline(
        egui::Rangef::new(r.left() - CARD_PAD_X, r.right() + CARD_PAD_X),
        y,
        Stroke::new(1.0, p.stroke),
    );
}

/// 悬停时在控件上盖一层极淡的中性色。
///
/// 按钮的底色是定制的（白底/实心强调色），egui 的 hovered 样式不会生效，而「整块换色」
/// 会让一排按钮的明度互相跳。盖一层几乎看不见的浮色，鼠标扫过去有反应、静止时又不变。
pub(crate) fn wash(ui: &egui::Ui, resp: &Response, p: &Palette, radius: CornerRadius) {
    if !resp.hovered() {
        return;
    }
    let dark = p.bg.to_array()[0] < 128;
    let tint = if dark {
        Color32::from_white_alpha(12)
    } else {
        Color32::from_black_alpha(8)
    };
    ui.painter().rect_filled(resp.rect, radius, tint);
}

/// 主按钮：品牌红渐变，用于「点亮」这类主动作。
///
/// 官网上 `.btn-primary` 是 180° 渐变 `--btn-a → --btn-b`，egui 的 `Button` 只能给一个
/// 单色填充，所以这里自己画：先按 16 条色带铺出渐变（上下两端各自成圆角，中间直角，
/// 圆角区因此不会被色带切成台阶），再叠悬停浮色，最后把文字压在中心。
///
/// 用自绘而不是「给 Button 加 fill 再盖一层」的原因：`add()` 之后再画的东西都在文字
/// 之上，盖深色会把白字压脏。自绘保住 `Sense::click` —— 命中、悬停、按下、点击回执
/// 与 `ui.button` 完全一致，外层 `tip()` 的说明气泡也照常工作。
pub(crate) fn primary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let font = TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), Color32::WHITE);
    let pad = ui.spacing().button_padding;
    let size = Vec2::new(
        (galley.size().x + pad.x * 2.0).max(BTN_W),
        CTRL_H.max(galley.size().y + pad.y * 2.0),
    );
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(WIDGET_RADIUS);
        let enabled = ui.is_enabled();
        // 按下时整块用下缘色：没有位移和阴影可借，颜色加深是唯一还读得出来的反馈。
        let (top, bottom) = if !enabled {
            (p.sunk, p.sunk)
        } else if resp.is_pointer_button_down_on() {
            (p.btn_deep, p.btn_deep)
        } else {
            (p.btn, p.btn_deep)
        };
        if top == bottom {
            ui.painter()
                .rect_filled(rect, radius, if enabled { p.btn_deep } else { p.sunk });
        } else {
            gradient(ui.painter(), rect, radius, top, bottom);
        }
        if enabled && resp.hovered() {
            ui.painter()
                .rect_filled(rect, radius, Color32::from_white_alpha(14));
        }
        // 顶部一道 1px 亮边，对应站点按钮的高光
        if enabled {
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 1.0, rect.top() + 1.0),
                    egui::pos2(rect.right() - 1.0, rect.top() + 2.0),
                ),
                CornerRadius::same(1),
                Color32::from_white_alpha(30),
            );
        }
        let ink = if enabled {
            Color32::WHITE
        } else {
            Color32::from_white_alpha(130)
        };
        ui.painter()
            .galley(rect.center() - galley.size() / 2.0, galley, ink);
    }
    if ui.is_enabled() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// 竖向渐变：`top → bottom`，上下两端按 `radius` 收圆角，中间各带直角。
///
/// 16 条色带在 32px 高的按钮上每带两像素，色差不到 3，肉眼看是连续过渡。
pub(crate) fn gradient(
    painter: &egui::Painter,
    rect: egui::Rect,
    radius: CornerRadius,
    top: Color32,
    bottom: Color32,
) {
    const BANDS: usize = 16;
    let h = rect.height();
    for i in 0..BANDS {
        let t = (i as f32 + 0.5) / BANDS as f32;
        let band = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + h * i as f32 / BANDS as f32),
            egui::pos2(rect.right(), rect.top() + h * (i + 1) as f32 / BANDS as f32),
        );
        painter.rect_filled(
            band,
            CornerRadius {
                nw: if i == 0 { radius.nw } else { 0 },
                ne: if i == 0 { radius.ne } else { 0 },
                sw: if i + 1 == BANDS { radius.sw } else { 0 },
                se: if i + 1 == BANDS { radius.se } else { 0 },
            },
            mix(top, bottom, t),
        );
    }
}

/// 两个色之间按 `t` 线性取色。
pub(crate) fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// 次级按钮：白底 + 细描边，和主按钮并排时不抢注意力。
///
/// 底色从「凹陷灰」改成「输入框同款的白底」，一排按钮旁边就是下拉框和输入框，
/// 底色一致之后它们看起来才是一套控件，而不是两种材质。
pub(crate) fn secondary(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let resp = ui.add(
        egui::Button::new(RichText::new(text).color(p.text))
            .fill(p.field)
            .stroke(Stroke::new(1.0, p.stroke))
            .corner_radius(CornerRadius::same(WIDGET_RADIUS))
            .min_size(Vec2::new(BTN_W, CTRL_H)),
    );
    wash(ui, &resp, p, CornerRadius::same(WIDGET_RADIUS));
    resp
}

/// 开关：自绘的 34×20 胶囊，开时滑到右侧并染成强调色。
///
/// 复选框那一格小白框在卡片式界面里存在感太弱，「开 / 关」这类二元项用开关更符合
/// 现在的阅读习惯；顺带省掉了勾选框里那圈看不清的描边。
pub(crate) fn switch(ui: &mut egui::Ui, p: &Palette, on: &mut bool) -> Response {
    let size = Vec2::new(36.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    // 位置按时间插值：点击瞬间滑块滑过去，而不是硬跳
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let track = if *on {
        p.accent.gamma_multiply(0.35 + 0.65 * t)
    } else {
        p.stroke
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((size.y / 2.0) as u8), track);
    let pad = 3.0;
    let travel = size.x - size.y;
    let knob_x = rect.left() + pad + travel * t;
    let knob = egui::Rect::from_center_size(
        egui::pos2(knob_x + size.y / 2.0 - pad, rect.center().y),
        Vec2::splat(size.y - pad * 2.0),
    );
    ui.painter().rect_filled(
        knob,
        CornerRadius::same((knob.height() / 2.0) as u8),
        Color32::WHITE,
    );
    resp
}

/// 状态胶囊：圆角小标签，用来显示「真实硬件 / 模拟后端」这类一眼要看到的状态。
pub(crate) fn pill(ui: &mut egui::Ui, text: &str, color: Color32) -> Response {
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = Vec2::new(galley.size().x + 18.0, galley.size().y + 8.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let bg = Color32::from_rgba_premultiplied(
        (color.r() as u16 * 32 / 255) as u8,
        (color.g() as u16 * 32 / 255) as u8,
        (color.b() as u16 * 32 / 255) as u8,
        32,
    );
    ui.painter().rect(
        rect,
        CornerRadius::same((size.y / 2.0) as u8),
        bg,
        Stroke::new(1.0, color.gamma_multiply(0.35)),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    resp
}

/// 页签：文字 + 底部指示条，整条基线横贯控件区。
///
/// 上一版是 iOS 那种分段胶囊：一条灰轨道，选中格用实心强调色托起。功能没错，
/// 但深色实心块在桌面工具里太抢眼——一屏十几处强调色之后，页签变成最重的一块，
/// 反而压过内容。改成下划线页签：未选中只有灰字，当前页靠加粗的黑字 + 2px 强调色
/// 短条表达，视线重心回到内容上。
pub(crate) fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    p: &Palette,
    id: &str,
    current: T,
    items: &[(T, &str, &str)],
) -> Option<T> {
    let font = FontId::proportional(13.5);
    let pad = Vec2::new(14.0, 8.0);
    let widths: Vec<f32> = items
        .iter()
        .map(|(_, text, _)| {
            ui.painter()
                .layout_no_wrap((*text).to_owned(), font.clone(), p.text)
                .size()
                .x
                + pad.x * 2.0
        })
        .collect();
    let total = ui.available_width();
    let height = pad.y * 2.0 + 18.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(total, height), Sense::hover());

    // 基线：一条压在所有页签下面的细线，让「一排页签」自己成为一条控件，
    // 而不是几个散落的文字。
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, p.stroke),
    );

    let mut picked = None;
    let mut x = rect.left();
    for ((value, text, help), width) in items.iter().zip(&widths) {
        let item =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), Vec2::new(*width, height - 1.0));
        let selected = *value == current;
        let resp = ui.interact(item, ui.scope_id().with((id, text)), Sense::click());
        if resp.hovered() && !selected {
            ui.painter().rect_filled(
                item.shrink2(Vec2::new(4.0, 6.0)),
                CornerRadius::same(WIDGET_RADIUS),
                p.stroke.gamma_multiply(0.5),
            );
        }
        let color = match (selected, resp.hovered()) {
            (true, _) => p.text,
            (false, true) => p.text,
            (false, false) => p.text_weak,
        };
        let label = ui
            .painter()
            .layout_no_wrap((*text).to_owned(), font.clone(), color);
        let center = item.center();
        ui.painter().galley(
            egui::pos2(
                center.x - label.size().x / 2.0,
                center.y - label.size().y / 2.0,
            ),
            label.clone(),
            color,
        );
        if selected {
            // 指示条只覆盖文字宽度，两端留出内边距，看起来像「贴在词下面」而不是「铺满格子」
            let bar_w = (label.size().x + 8.0).min(item.width() - 8.0);
            let bar = egui::Rect::from_min_size(
                egui::pos2(center.x - bar_w / 2.0, item.bottom() - 3.0),
                Vec2::new(bar_w, 2.5),
            );
            ui.painter()
                .rect_filled(bar, CornerRadius::same(2), p.accent);
        }
        if resp.hovered() && !help.is_empty() {
            tip(resp.clone(), *help);
        }
        if resp.clicked() {
            picked = Some(*value);
        }
        x += width;
    }
    picked
}

/// 亮度档位条：一格一档，点哪格就是哪档。
///
/// 比默认 `Slider` 直观——滑块只能看到当前位置，看不到「一共几档、还能往哪走」；
/// 这里每格都画出来，前 N 格实心表示当前亮度，后面留空。
pub(crate) fn level_meter(
    ui: &mut egui::Ui,
    p: &Palette,
    id: &str,
    step: u8,
    steps: u8,
) -> Option<u8> {
    let steps = steps.max(1);
    let gaps = (steps - 1) as f32 * 6.0;
    // 档位格铺满控件列：之前封顶 520，4 档只排到控件列一半（485 对 615），
    // 同一张卡片里上面是满宽输入框、下面是半截的档位条，白色块一长一短最扎眼。
    let width = ui.available_width().min(ctrl_width()).max(CTRL_MIN_W);
    let cell = ((width - gaps) / steps as f32).max(18.0);
    let height = 30.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(cell * steps as f32 + gaps, height),
        Sense::hover(),
    );

    let mut picked = None;
    for i in 0..steps {
        let cell_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + i as f32 * (cell + 6.0), rect.top()),
            Vec2::new(cell, height),
        );
        let index = i + 1;
        let lit = index <= step;
        let resp = tip(
            ui.interact(cell_rect, ui.scope_id().with((id, index)), Sense::click()),
            format!("第 {index} 档：点这一格直接切到该亮度"),
        );
        let fill = if lit {
            // 越亮越偏暖，暗档用强调色的深色版本，视觉上有「渐亮」的连续感
            let t = if steps > 1 {
                (index - 1) as f32 / (steps - 1) as f32
            } else {
                1.0
            };
            lerp_color(
                p.accent.gamma_multiply(0.7),
                Color32::from_rgb(255, 208, 138),
                t,
            )
        } else {
            p.sunk
        };
        let stroke = if resp.hovered() {
            Stroke::new(1.0, p.accent)
        } else {
            Stroke::new(1.0, p.stroke)
        };
        ui.painter().rect(
            cell_rect,
            CornerRadius::same(8),
            fill,
            stroke,
            egui::StrokeKind::Inside,
        );
        // 点亮的格子是暖色实心，用深色字；未点亮是凹底色，用次要色
        let text_color = if lit {
            Color32::from_rgb(38, 30, 12)
        } else {
            p.text_weak
        };
        ui.painter().text(
            cell_rect.center(),
            egui::Align2::CENTER_CENTER,
            index.to_string(),
            FontId::proportional(13.0),
            text_color,
        );
        if resp.clicked() {
            picked = Some(index);
        }
    }
    picked
}

pub(crate) fn lerp_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    Color32::from_rgb(
        mix(from.r(), to.r()),
        mix(from.g(), to.g()),
        mix(from.b(), to.b()),
    )
}
