//! 界面字体：egui 自带字体没有汉字，启动时从系统字体里挑一个能用的装进去。

use std::sync::Arc;

/// 候选字体：先找单 face 的 ttf/otf，再退到 Windows / macOS 上常见的中文字体。
///
/// `ttc` 是字体集合，能不能被 egui 的解析器接受取决于第一个 face 的封装，
/// 所以每个候选都要先过一遍 `ab_glyph` 解析，失败就换下一个。
const CANDIDATES: &[&str] = &[
    // Windows
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\msyhbd.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\simsun.ttc",
    r"C:\Windows\Fonts\Deng.ttf",
    // macOS（开发机预览用）：官网中文正文走 PingFang SC，这里优先装同一族；
    // 解析不了再退到 Arial Unicode / Hiragino，保证预览与目标平台观感一致。
    "/System/Library/Fonts/PingFang.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
    // Linux（交叉预览用）
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];

/// 装上中文字体，返回实际使用的字体路径。
///
/// 全部候选都不可用时返回 `None`，界面会退回 egui 默认字体（中文显示为方块，
/// 但功能不受影响）。
pub fn install(ctx: &eframe::egui::Context) -> Option<String> {
    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        // 先用 egui 背后的同一个解析器验一遍，避免装进去后渲染时炸
        if ab_glyph::FontArc::try_from_vec(bytes.clone()).is_err() {
            continue;
        }
        let mut fonts = eframe::egui::FontDefinitions::default();
        fonts.font_data.insert(
            "cjk".to_owned(),
            Arc::new(eframe::egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(eframe::egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "cjk".to_owned());
        fonts
            .families
            .entry(eframe::egui::FontFamily::Monospace)
            .or_default()
            .push("cjk".to_owned());
        ctx.set_fonts(fonts);
        return Some((*path).to_string());
    }
    None
}
