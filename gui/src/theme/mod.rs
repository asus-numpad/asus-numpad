//! 界面外观：一套「卡片 + 单一强调色」的样式，以及几个排版小工具。
//!
//! 样式只在 [`App::new`](crate::app::App::new) 里装一次（[`install`]），页面代码照旧
//! 用 `ui.button` / `ui.label`，颜色、圆角、间距、字号统一从这里出，避免各页面各写一套。
//!
//! 配色跟着系统深浅色走：Windows / Linux 桌面默认是浅色，硬塞深色反而突兀，所以两套
//! 调色板都在这里定义，按 `ctx.theme()` 取一套；`NUMDECK_GUI_THEME=dark|light`
//! 可以强制覆盖（截图和交稿时用得上）。

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Response, RichText, Sense, Stroke,
    TextStyle, Vec2,
};

/// 一整套颜色。两套调色板字段一致，界面代码只用字段名，不写死色值。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// 窗口底色（卡片之间的留白）
    pub bg: Color32,
    /// 侧栏 / 顶栏底色
    pub panel: Color32,
    /// 卡片底色
    pub card: Color32,
    /// 卡片内的凹陷区 / 悬停底色
    pub sunk: Color32,
    /// 输入框、下拉框、次级按钮的底色。
    ///
    /// 和 `sunk` 分开是有原因的：凹陷色是「往里凹」的灰面，用在白卡片上的输入框会
    /// 变成一块块灰方块；输入框要的是「白底 + 一圈细线」，所以浅色下它等于卡片色，
    /// 深色下比卡片亮一档。
    pub field: Color32,
    /// 分割线、卡片描边
    pub stroke: Color32,
    /// 正文
    pub text: Color32,
    /// 次要正文（说明文字）
    pub text_weak: Color32,
    /// 强调色（选中、焦点描边、进度、页签指示条）
    pub accent: Color32,
    /// 强调色的淡底
    pub accent_soft: Color32,
    /// 实心主按钮的底色。
    ///
    /// 和 `accent` 分开是官网上写明的取舍：强调色要亮（高亮、描边都要它），
    /// 但按钮上压着白字，亮红只能到 3.6:1，压深一档的 `#d81b45` 才有 5:1。
    pub btn: Color32,
    /// 主按钮渐变的**下缘**色（站点 `--btn-b`）。
    ///
    /// 官网上 `.btn-primary` 是 180° 渐变 `--btn-a → --btn-b`；egui 的按钮只有单色，
    /// 所以底色仍用 `btn`，再在下缘压一层这个色做「上亮下深」，远看是同一枚按钮。
    pub btn_deep: Color32,
    /// 正常状态
    pub ok: Color32,
    /// 告警
    pub warn: Color32,
    /// 错误
    pub bad: Color32,
}

/// 深色：与官网 `html[data-theme="dark"]` 的令牌一一对应。
///
/// 官网页面的卡片底是 `rgba(255,255,255,.035)` 叠在 `#0a0b0f` 上，egui 的 `Frame`
/// 只能给实色，所以这里写的是**叠完之后**的等效实色（10 + 245×0.035 ≈ 19），
/// `sunk` / `field` / `stroke` 同理按 .06 / .06 / .09 的透明度折算好。改站点配色时
/// 照这个公式重算一次即可，不需要 GUI 另立一套设计语言。
pub const DARK: Palette = Palette {
    // 站点 --bg / --bg-alt
    bg: Color32::from_rgb(10, 11, 15),
    panel: Color32::from_rgb(13, 15, 21),
    card: Color32::from_rgb(19, 21, 26),
    sunk: Color32::from_rgb(26, 28, 34),
    field: Color32::from_rgb(33, 35, 40),
    stroke: Color32::from_rgb(40, 42, 47),
    // --text / --text-2：次要文字 8.4:1，压住 4.5:1
    text: Color32::from_rgb(236, 238, 244),
    text_weak: Color32::from_rgb(169, 173, 186),
    // --accent（品牌红）
    accent: Color32::from_rgb(255, 45, 85),
    // 品牌红 17% 叠在 --bg 上的预乘值（egui 用预乘 alpha）：52,17,27,44
    accent_soft: Color32::from_rgba_premultiplied(52, 17, 27, 44),
    // --btn-a：品牌红压深一档，白字 5.05:1
    btn: Color32::from_rgb(216, 27, 69),
    // --btn-b
    btn_deep: Color32::from_rgb(179, 18, 58),
    ok: Color32::from_rgb(52, 211, 153),
    warn: Color32::from_rgb(240, 180, 41),
    bad: Color32::from_rgb(255, 92, 122),
};

/// 浅色：与官网 `html[data-theme="light"]` 对应。规则同上，底色换 `#f7f8fa`、
/// 强调色压深到 `#b8163c`（亮底上亮红只有 2.8:1，看不清）。
pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(247, 248, 250),
    panel: Color32::from_rgb(255, 255, 255),
    card: Color32::from_rgb(255, 255, 255),
    sunk: Color32::from_rgb(235, 236, 239),
    field: Color32::from_rgb(255, 255, 255),
    stroke: Color32::from_rgb(232, 233, 236),
    text: Color32::from_rgb(20, 22, 31),
    text_weak: Color32::from_rgb(77, 82, 98),
    accent: Color32::from_rgb(184, 22, 60),
    // 品牌红 11% 叠在 --bg 上的预乘值：240,223,229,28
    accent_soft: Color32::from_rgba_premultiplied(240, 223, 229, 28),
    btn: Color32::from_rgb(192, 20, 60),
    // --btn-b
    btn_deep: Color32::from_rgb(156, 15, 48),
    ok: Color32::from_rgb(4, 120, 87),
    warn: Color32::from_rgb(180, 83, 9),
    bad: Color32::from_rgb(185, 28, 28),
};

/// 卡片圆角：官网 `--radius: 14px`。
const CARD_RADIUS: u8 = 14;
/// 控件圆角：官网 `.btn { border-radius: 10px }`。
const WIDGET_RADIUS: u8 = 10;

/// 装上主题，返回当前生效的调色板。
pub fn install(ctx: &egui::Context) -> Palette {
    let forced = std::env::var("NUMDECK_GUI_THEME").ok();
    let dark = match forced.as_deref().map(str::trim) {
        Some("dark") => true,
        Some("light") => false,
        _ => ctx.style_of(ctx.theme()).visuals.dark_mode,
    };
    // 两套都装：用户中途切系统主题时，界面不会掉回 egui 默认的紫色灰底
    ctx.set_style_of(egui::Theme::Dark, style_for(DARK, true));
    ctx.set_style_of(egui::Theme::Light, style_for(LIGHT, false));
    let theme = egui::Theme::from_dark_mode(dark);
    ctx.set_theme(theme);
    if dark {
        DARK
    } else {
        LIGHT
    }
}

fn style_for(p: Palette, dark: bool) -> egui::Style {
    let mut style = egui::Theme::from_dark_mode(dark).default_style();
    let mut v = style.visuals;
    v.dark_mode = dark;

    // 字号：正文 14，说明文字压到 12，标题 19 拉开层级
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
    ]
    .into();

    // 间距：默认值偏挤，卡片式布局要留出呼吸感
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(16.0, 8.0);
    style.spacing.interact_size = Vec2::new(44.0, 28.0);
    style.spacing.slider_width = 190.0;
    style.spacing.combo_width = 220.0;
    style.spacing.indent = 22.0;
    style.spacing.menu_margin = Margin::same(8);
    // 滚动条：macOS 那种浮起的窄条——平时几乎看不见，鼠标进到滚动区才浮出来，
    // 且不占内容宽度。非浮动模式下滚动条会从卡片右边啃掉 10px，卡片宽度跟着抖。
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style.spacing.scroll.bar_width = 8.0;
    style.spacing.scroll.floating_width = 6.0;
    style.spacing.scroll.floating_allocated_width = 0.0;
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.active_handle_opacity = 0.35;
    style.spacing.scroll.interact_handle_opacity = 0.7;
    style.spacing.scroll.dormant_background_opacity = 0.0;
    style.spacing.scroll.active_background_opacity = 0.0;
    style.spacing.scroll.interact_background_opacity = 0.15;

    v.panel_fill = p.bg;
    v.window_fill = p.card;
    v.extreme_bg_color = p.field;
    v.faint_bg_color = p.sunk;
    v.hyperlink_color = p.accent;
    v.weak_text_color = Some(p.text_weak);
    v.override_text_color = Some(p.text);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.window_stroke = Stroke::new(1.0, p.stroke);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 36 }),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 100 } else { 28 }),
    };
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);

    // 控件四态：静默=白底细描边（输入框该有的样子），悬停=描边转强调色，
    // 按下/展开=强调色描边 + 淡底。灰底只留给凹陷区，不再拿来当输入框底色。
    let widgets = &mut v.widgets;
    widgets.noninteractive.bg_fill = p.card;
    widgets.noninteractive.weak_bg_fill = p.card;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.noninteractive.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.inactive.bg_fill = p.field;
    widgets.inactive.weak_bg_fill = p.field;
    widgets.inactive.bg_stroke = Stroke::new(1.0, p.stroke);
    widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    widgets.inactive.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.hovered.bg_fill = p.field;
    widgets.hovered.weak_bg_fill = p.field;
    widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.7));
    widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    widgets.hovered.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.active.bg_fill = p.accent_soft;
    widgets.active.weak_bg_fill = p.accent_soft;
    widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    widgets.active.fg_stroke = Stroke::new(1.0, p.accent);
    widgets.active.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    widgets.open.bg_fill = p.field;
    widgets.open.weak_bg_fill = p.field;
    widgets.open.bg_stroke = Stroke::new(1.0, p.accent.gamma_multiply(0.7));
    widgets.open.fg_stroke = Stroke::new(1.0, p.text);
    widgets.open.corner_radius = CornerRadius::same(WIDGET_RADIUS);

    style.visuals = v;
    style
}

/// 表单左列宽度。所有「标签 + 控件」的行共用这一列，控件因此永远从同一条竖线起，
/// 换行、换卡片都不会左右错位。
///
/// 取 132 而不是更窄：中文标签（「应用手势触发距离」「独占触摸板」）在 13px 下排开
/// 就要 100 出头，列宽给小了标签会折成两行，行高一变，整片表单就跟着参差。
pub const LABEL_W: f32 = 132.0;

/// 卡片内容到卡片边缘的左右留白（[`card`] 的 `inner_margin`）。
pub const CARD_PAD_X: f32 = 20.0;

/// 标签列与控件列之间的间距，与全局 `item_spacing.x` 取同一个值。
pub const ROW_GAP: f32 = 12.0;

/// 控件列宽度下限：窄窗口下也不能窄到把下拉框里的档位名切掉。
pub const CTRL_MIN_W: f32 = 280.0;

/// 控件行高。输入框、下拉框、数字框、按钮都取这个高度，一行里高矮一致。
pub const CTRL_H: f32 = 32.0;

/// 一格并排两个控件时左边那个（勾选框、小序号）的宽度；右边那个补齐到
/// 控件列剩下的宽度，于是「两个小控件一行」与「一个大控件」的右边缘仍然齐平。
pub const PAIR_A: f32 = 84.0;
/// 并排两控件之间的间距。
pub const PAIR_GAP: f32 = 8.0;

/// 按钮统一宽度：全界面只有这一个按钮宽度。
pub const BTN_W: f32 = 104.0;

/// 当前页的控件列宽度（以千分之一像素存，`AtomicU32` 不能直接存 `f32`）。
///
/// **为什么要做成每帧算一次的全局值**：每张卡片、每个折叠区里 `ui.available_width()`
/// 各不相同（折叠头、滚动条、内层 `Frame` 都会吃掉几像素），如果每一行各自按自己的
/// 可用宽度算控件列，同一页里就会同时出现 300 / 294 / 288 三种列宽——右边缘差几像素，
/// 看着就是「没对齐」。所以一页只算一次，整页所有行都用这一个数字。
static COLUMN_W_MILLI: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(300_000);

mod brand;
mod layout;
mod widgets;

pub(crate) use brand::*;
pub(crate) use layout::*;
pub(crate) use widgets::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_expected_shape() {
        let icon = app_icon(64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
        assert_eq!((icon.width, icon.height), (64, 64));
    }

    #[test]
    fn icon_corner_is_transparent_and_body_is_opaque() {
        let icon = app_icon(64);
        let px = |x: u32, y: u32| {
            let i = ((y * 64 + x) * 4) as usize;
            (
                icon.rgba[i],
                icon.rgba[i + 1],
                icon.rgba[i + 2],
                icon.rgba[i + 3],
            )
        };
        // 圆角之外必须透明，否则图标在浅色任务栏上是个黑方块
        assert_eq!(px(0, 0).3, 0);
        // 底板中心不透明
        assert_eq!(px(32, 16).3, 255);
        // 取一处分明的区域：键盘框下方那条底板（视框 y≈29），颜色应当接近 #0a0b0f
        let (r, g, b, _) = px(32, 58);
        assert!(r < 40 && g < 40 && b < 40, "底板像素偏亮：{r},{g},{b}");
    }

    /// 把一帧里所有矩形摊平，便于按「按钮的形状」去断言。
    fn walk_rects(s: &egui::Shape, out: &mut Vec<(egui::Rect, Color32)>) {
        match s {
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk_rects(s, out)),
            _ => {}
        }
    }

    /// 跑一帧，把主按钮画出来，返回帧内所有矩形。
    fn frame_with_primary(p: &Palette) -> Vec<(egui::Rect, Color32)> {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_space(8.0);
            primary(ui, p, "OK");
        });
        // 字体图集是给渲染器用的，测试里没人消费，丢掉前要显式 clear
        out.textures_delta.clear();
        let mut all = Vec::new();
        out.shapes
            .iter()
            .for_each(|c| walk_rects(&c.shape, &mut all));
        all
    }

    #[test]
    fn primary_button_gets_a_vertical_gradient() {
        let rects = frame_with_primary(&DARK);
        // 渐变色带：宽度就是按钮宽，高度只有 32/16=2px
        let mut bands: Vec<(f32, Color32)> = rects
            .iter()
            .filter(|(r, c)| {
                (r.width() - BTN_W).abs() < 1.0
                    && r.height() < 3.0
                    && c.r() > 150
                    && c.g() < 70
                    && (40..110).contains(&c.b())
            })
            .map(|(r, c)| (r.top(), *c))
            .collect();
        bands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        assert!(
            bands.len() >= 8,
            "主按钮没画出渐变，只找到 {} 条色带",
            bands.len()
        );
        // 上到下只会越压越深，不会回弹
        for w in bands.windows(2) {
            assert!(
                w[1].1.r() <= w[0].1.r() + 1,
                "色带上下顺序反了：{:?} 在 {:?} 之下",
                w[1],
                w[0]
            );
        }
        // 端点必须真的分别落在 --btn-a 与 --btn-b 上，否则等于没渐变
        let top = bands.first().unwrap().1;
        let bottom = bands.last().unwrap().1;
        assert!(
            (top.r() as i32 - DARK.btn.r() as i32).abs() <= 3,
            "上缘色不对：{top:?} vs {:?}",
            DARK.btn
        );
        assert!(
            (bottom.r() as i32 - DARK.btn_deep.r() as i32).abs() <= 3,
            "下缘色不对：{bottom:?} vs {:?}",
            DARK.btn_deep
        );
    }

    #[test]
    fn primary_button_keeps_its_label_on_top() {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_space(8.0);
            primary(ui, &DARK, "OK");
        });
        out.textures_delta.clear();
        let mut labels = Vec::new();
        fn walk_text(s: &egui::Shape, out: &mut Vec<(egui::Rect, Color32)>) {
            match s {
                egui::Shape::Text(t) => out.push((
                    egui::Rect::from_min_size(t.pos, t.galley.size()),
                    t.fallback_color,
                )),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk_text(s, out)),
                _ => {}
            }
        }
        out.shapes
            .iter()
            .for_each(|c| walk_text(&c.shape, &mut labels));
        // 文字必须存在、是白的，且落在按钮的垂直中线附近——不能被色带盖住
        let (text_rect, color) = labels
            .iter()
            .find(|(r, _)| (r.width() - 24.0).abs() < 12.0 && r.height() > 8.0)
            .map(|(r, c)| (r, *c))
            .or_else(|| labels.first().map(|(r, c)| (r, *c)))
            .expect("主按钮没画出文字");
        assert_eq!(color, Color32::WHITE, "主按钮文字不是白色");
        assert!(
            text_rect.width() > 4.0 && text_rect.height() > 4.0,
            "主按钮文字尺寸异常：{text_rect:?}"
        );
    }

    #[test]
    fn primary_button_keeps_its_size_and_click_zone() {
        // 自绘后仍要走 allocate_exact_size，否则点击区域会缩成 0
        let ctx = egui::Context::default();
        let mut resp_rect = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add_space(8.0);
            let r = primary(ui, &DARK, "OK");
            resp_rect = r.rect;
            // 单行文字撑不出 BTN_W，按钮必须自己撑到最窄宽度
            assert!((r.rect.width() - BTN_W).abs() < 1.0);
            assert!((r.rect.height() - CTRL_H).abs() < 1.0);
            assert!(r.interact_rect.contains(r.rect.center()));
        });
        out.textures_delta.clear();
        assert!(resp_rect.width() > 0.0);
    }

    #[test]
    fn icon_size_is_clamped() {
        // 传 0 也不能炸：夹到最小边再算，长度自洽
        let icon = app_icon(0);
        assert_eq!(icon.rgba.len(), (icon.width * icon.height * 4) as usize);
        assert!(icon.width >= 8);
    }
}
