//! 品牌与提示件：Logo、图标、标签、气泡提示。

use super::*;
use eframe::egui;

/// 品牌标记：官网页头那枚键盘图标，24×24 视框等比放大到 `size`。
///
/// 与 `site/index.html` 里的 `.brand-mark` 是同一套几何：19×15 的圆角外框加四个
/// 键位短线。用矢量画而不是塞位图——深浅两套主题、任意缩放和高 DPI 都清晰，
/// 仓库里也不用维护多份 PNG（站点那边只有一枚 favicon.svg）。
pub(crate) fn logo(ui: &mut egui::Ui, p: &Palette, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_logo(ui.painter(), rect, p.accent);
    resp
}

/// 把品牌标记画进任意矩形，供 [`logo`] 和窗口图标复用同一套坐标。
pub(crate) fn paint_logo(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    let s = (rect.width().min(rect.height()) / 24.0).max(0.1);
    let at = |x: f32, y: f32| egui::pos2(rect.left() + x * s, rect.top() + y * s);
    // 外框：视框内 (2.5,4.5) 起、19×15、圆角 3、线宽 1.6
    let body = egui::Rect::from_min_max(at(2.5, 4.5), at(21.5, 19.5));
    painter.rect_stroke(
        body,
        CornerRadius::same(((3.0 * s).round() as u8).max(2)),
        Stroke::new((1.6 * s).max(1.0), color),
        egui::StrokeKind::Inside,
    );
    // 四个键位：站点上是 `M8 10h2` 这一组短线，小尺寸下比实心方块更清楚
    let w = (1.6 * s).max(1.0);
    for (x, y) in [(8.0, 10.0), (14.0, 10.0), (8.0, 14.5), (14.0, 14.5)] {
        painter.line_segment([at(x, y), at(x + 2.0, y)], Stroke::new(w, color));
    }
}

/// 版本胶囊：官网页头 `.brand-tag` 的同款——等宽字体、细描边、全圆角。
///
/// 摆在品牌名右边当「这是哪个版本」的角标，比在正文里单开一行写版本号省地方。
pub(crate) fn tag(ui: &mut egui::Ui, p: &Palette, text: &str) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::monospace(11.0), p.text_weak);
    let size = Vec2::new(galley.size().x + 14.0, galley.size().y + 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same((size.y / 2.0) as u8),
        Stroke::new(1.0, p.stroke),
        egui::StrokeKind::Inside,
    );
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, p.text_weak);
    resp
}

/// 给任意控件挂一段悬浮说明：统一宽度与字号，各页 tooltip 不会长短不一。
///
/// 内容分两层：一行灰色小字「说明」交代这是什么，下面是正文。原先只有一句 12.5 号
/// 文字浮在控件旁，长句子跑到两三行时会被误读成界面正文；补一行标签后，凡是浮出来的
/// 方块都能一眼认出是解释。宽度压到 300 像素（约 25 个汉字一行），再长也读得下去。
pub(crate) fn tip(resp: Response, text: impl Into<String>) -> Response {
    let text = text.into();
    // 截图模式下要靠这个矩形决定指针停在哪，气泡才拍得到（正常启动是空操作）
    crate::shot::note_tip_rect(resp.rect, &text);
    // 挂说明的控件统一换成问号光标，用户能先看出「这里有东西可停」再停上去。
    resp.on_hover_cursor(egui::CursorIcon::Help)
        .on_hover_ui(move |ui| {
            ui.set_max_width(300.0);
            ui.spacing_mut().item_spacing.y = 4.0;
            let v = ui.visuals();
            let accent = v.hyperlink_color;
            let body = v.override_text_color.unwrap_or(v.text_color());
            ui.label(
                RichText::new("说明")
                    .size(10.0)
                    .color(accent.gamma_multiply(0.9)),
            );
            ui.label(
                RichText::new(text.as_str())
                    .size(12.5)
                    .line_height(Some(19.0))
                    .color(body),
            );
        })
}

/// 标签后面那个小圆圈 `i`：鼠标停上去出说明。
///
/// 参数面板里每一项都值得解释一句，但全铺成正文会把界面撑散，所以统一做法是
/// 标签右侧留一个图标，想看的人停一下鼠标。
pub(crate) fn info_dot(ui: &mut egui::Ui, p: &Palette, text: impl Into<String>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(15.0), Sense::hover());
    let hot = resp.hovered();
    // 常态是「淡底 + 淡描边」的圆圈：整页几十个 i 全染成实心强调色会变成一堆蓝噪点，
    // 但只画一圈卡片描边色又几乎看不见（浅色卡片尤其如此），折中方案是强调色淡底 +
    // 半透明描边——安静，又能被扫到。鼠标停上去才反过来变成实心强调色。
    //
    // 深色底要单独提一档：同样的半透明描边压在近黑卡片上会糊掉，10 号小字跟着一起
    // 看不清，所以描边和底色都加浓。
    let dark = p.bg.to_array()[0] < 128;
    let fill = if hot {
        p.accent
    } else if dark {
        p.accent.gamma_multiply(0.22)
    } else {
        p.accent_soft
    };
    let line = if hot {
        p.accent
    } else {
        p.accent.gamma_multiply(if dark { 0.8 } else { 0.45 })
    };
    let glyph = if hot { p.card } else { p.accent };
    ui.painter()
        .circle(rect.center(), 6.5, fill, Stroke::new(1.0, line));
    ui.painter().text(
        rect.center() + Vec2::new(0.0, 0.5),
        egui::Align2::CENTER_CENTER,
        "i",
        FontId::proportional(9.5),
        glyph,
    );
    tip(resp, text)
}

/// 圆角矩形的有符号距离：负数在内部，正数在外部。
///
/// 图标光栅化用，`x`/`y` 是左上角，`w`/`h` 是宽高，`r` 是圆角半径。
pub(crate) fn sd_round_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let qx = (px - cx).abs() - (w / 2.0 - r);
    let qy = (py - cy).abs() - (h / 2.0 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// 窗口 / 任务栏图标：`site/assets/favicon.svg` 的光栅版。
///
/// 站点上那枚 favicon 就是本项目的标记（黑底圆角方块 + 红框键盘 + 六键），这里按
/// 32×32 视框逐点判定 SDF 再 4×4 超采样，得到任意尺寸的 RGBA。不引 resvg / tiny-skia：
/// 为一张图标拉一条 SVG 解析链不划算，而这点几何用距离函数几行就写完了。
///
/// 返回值直接喂 `eframe::egui::IconData`，Linux / Windows 的窗口标题栏与任务栏都会用。
pub(crate) fn app_icon(size: u32) -> egui::IconData {
    let size = size.clamp(8, 1024);
    let n = size as f32;
    // 视框 32 → 像素的缩放
    let k = n / 32.0;
    /// 每像素在一维上的采样数，4×4 = 16 个采样点，足够把圆角磨平。
    const SS: u32 = 4;
    // favicon 里的颜色
    let plate = (10u8, 11u8, 15u8);
    let accent = (255u8, 45u8, 85u8);
    let light = (236u8, 238u8, 244u8);
    // 六个键位：前三后三，最后一枚是亮色（NumLock 打开的样子）
    let keys = [
        (8.0f32, 12.0f32, accent),
        (14.3, 12.0, accent),
        (20.6, 12.0, accent),
        (8.0, 17.4, accent),
        (14.3, 17.4, accent),
        (20.6, 17.4, light),
    ];

    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let step = 1.0 / SS as f32;
    for py in 0..size {
        for px in 0..size {
            let mut rgb = [0f32; 3];
            let mut hits = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    // 采样点取像素中心偏移，避免整体左上半像素的漂移
                    let x = (px as f32 + (sx as f32 + 0.5) * step) / k;
                    let y = (py as f32 + (sy as f32 + 0.5) * step) / k;
                    // 自底向上叠：外框描边和键位都画在底板上，底板不透明所以三点式取色即可
                    let mut color = None;
                    if sd_round_rect(x, y, 4.5, 8.0, 23.0, 16.0, 3.5) < 0.0
                        && sd_round_rect(x, y, 4.5 + 1.8, 8.0 + 1.8, 23.0 - 3.6, 16.0 - 3.6, 2.5)
                            > 0.0
                    {
                        color = Some(accent);
                    }
                    for (kx, ky, key) in keys {
                        if sd_round_rect(x, y, kx, ky, 3.4, 3.4, 1.0) < 0.0 {
                            color = Some(key);
                        }
                    }
                    if color.is_none() && sd_round_rect(x, y, 0.0, 0.0, 32.0, 32.0, 7.0) < 0.0 {
                        color = Some(plate);
                    }
                    if let Some((r, g, b)) = color {
                        rgb[0] += r as f32;
                        rgb[1] += g as f32;
                        rgb[2] += b as f32;
                        hits += 1;
                    }
                }
            }
            let idx = ((py * size + px) * 4) as usize;
            if hits == 0 {
                // 圆角外的空白：全透明
                continue;
            }
            let coverage = hits as f32 / (SS * SS) as f32;
            rgba[idx] = (rgb[0] / hits as f32).round() as u8;
            rgba[idx + 1] = (rgb[1] / hits as f32).round() as u8;
            rgba[idx + 2] = (rgb[2] / hits as f32).round() as u8;
            rgba[idx + 3] = (coverage * 255.0).round() as u8;
        }
    }
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}
