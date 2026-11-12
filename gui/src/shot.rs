//! 开发辅助：把当前画面存成 PPM，用来更新文档和官网截图。
//!
//! 只有设置了环境变量 `NUMDECK_GUI_SHOT=<路径>` 才生效，正常启动完全不走这里。
//! 之所以不用系统截图工具：一是 macOS 屏幕录制权限在新系统上经常拿不到，
//! 二是系统截图还会带窗口阴影和桌面背景，而这里拿到的是渲染出来的原始像素，1:1、干净。
//!
//!   NUMDECK_GUI_FULL=1 NUMDECK_GUI_SHOT=full.ppm ./target/debug/numdeck-gui
//!
//! 存完即退出，方便脚本化；PPM 交给 `PIL`/`sips` 转 PNG（不为此给 GUI 加图片依赖）。
//!
//! 离屏渲染没有真实鼠标，`on_hover_ui` 的气泡永远拍不到。设了
//! `NUMDECK_GUI_HOVER=1`（或显式坐标 `NUMDECK_GUI_HOVER=640,320`）后，
//! 本模块会在每帧的 `RawInput` 里塞一个 `PointerMoved`，停在第一个（或
//! `NUMDECK_GUI_HOVER_INDEX=n` 指定的第 n 个）挂了说明的控件上，气泡便随帧渲染出来。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use eframe::egui;

/// 截图前先空转几帧：字体、面板宽度和滚动区域都要几帧才稳定。
///
/// 开了悬停（`NUMDECK_GUI_HOVER`）时要多等：egui 的气泡有 0.5 s 的停留延迟，
/// 而离屏每帧只有几毫秒，不把时间推上去永远等不到那个阈值。
const WARMUP_FRAMES: u32 = 16;
const WARMUP_FRAMES_HOVER: u32 = 40;
/// 悬停模式下每帧推进的「虚拟时间」，让 40 帧足够盖过 0.5 s 的 tooltip 延迟。
const HOVER_STEP_SECS: f64 = 0.1;

static FRAMES: AtomicU32 = AtomicU32::new(0);
static REQUESTED: AtomicBool = AtomicBool::new(false);
/// 本帧已经画过多少个挂说明的控件（每帧开头清零，见 [`tick`]）
static TIPS_SEEN: AtomicU32 = AtomicU32::new(0);
/// 「只给了 HOVER_TEXT / HOVER_INDEX」的提示只打一次
static WARNED_HOVER_SWAP: AtomicBool = AtomicBool::new(false);
/// 停靠点：`tip` 命中目标序号时记下控件中心，之后每帧注入到 `RawInput`
static HOVER_AT: OnceLock<Mutex<Option<egui::Pos2>>> = OnceLock::new();

fn hover_at() -> &'static Mutex<Option<egui::Pos2>> {
    HOVER_AT.get_or_init(|| Mutex::new(None))
}

/// 截图模式下的悬停设置：`None` = 不注入指针（默认，纯静态图）。
///
/// 只给 `HOVER_TEXT` / `HOVER_INDEX` 也算开——初期一定要写成
/// `NUMDECK_GUI_HOVER=1 NUMDECK_GUI_HOVER_TEXT=…`，少写前一半就静默拍一张
/// 没有气泡的静态图，图看着「正常」，其实白渲染一轮（踩过）。现在缺那一半会在这里
/// 补上并往 stderr 说一声，几轮跑下来四张图的 md5 一模一样就是这种情况。
fn hover_setting() -> Option<HoverSetting> {
    let index = std::env::var("NUMDECK_GUI_HOVER_INDEX")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0u32);
    let text = std::env::var("NUMDECK_GUI_HOVER_TEXT")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let raw = match std::env::var("NUMDECK_GUI_HOVER") {
        Ok(v) if !v.trim().is_empty() => v,
        _ if index != 0 || text.is_some() => {
            // 只提示一次：这函数每帧都会被问一遍
            if !WARNED_HOVER_SWAP.swap(true, Ordering::Relaxed) {
                eprintln!("提示：没设 NUMDECK_GUI_HOVER，按已开启悬停处理");
            }
            String::from("1")
        }
        _ => return None,
    };
    let raw = raw.trim();
    // `640,320` 这种写法直接指定坐标；其余非空值一律当作「自动停在第一个说明控件上」
    let explicit = raw
        .split_once(',')
        .and_then(|(x, y)| Some(egui::pos2(x.trim().parse().ok()?, y.trim().parse().ok()?)));
    Some(HoverSetting {
        explicit,
        index,
        text,
    })
}

struct HoverSetting {
    explicit: Option<egui::Pos2>,
    index: u32,
    /// 按说明正文里的关键字挑控件（优先于 `index`）
    text: Option<String>,
}

/// `tip` 每次落笔都报一次位置：命中目标（序号或文本关键字）且还没记过，就当作停靠点。
///
/// 序号按本帧绘制顺序数，跨页跨主题都不稳定；想拍某一句话的气泡，用
/// `NUMDECK_GUI_HOVER_TEXT=<正文里的一段>` 更靠谱。
pub fn note_tip_rect(rect: egui::Rect, text: &str) {
    let Some(cfg) = hover_setting() else {
        return;
    };
    if cfg.explicit.is_some() {
        return;
    }
    if let Some(needle) = cfg.text.as_deref() {
        if !text.contains(needle) {
            return;
        }
    } else {
        let seen = TIPS_SEEN.fetch_add(1, Ordering::Relaxed);
        if seen != cfg.index {
            return;
        }
    }
    if let Ok(mut guard) = hover_at().lock() {
        if guard.is_none() {
            *guard = Some(rect.center());
        }
    }
}

/// 装在 `eframe::App::raw_input_hook` 上：先把页面滚到指定位置，再把指针挪到停靠点。
pub fn raw_input_hook(raw_input: &mut egui::RawInput) {
    let frame = FRAMES.load(Ordering::Relaxed);
    scroll_hook(raw_input, frame);
    let Some(cfg) = hover_setting() else {
        return;
    };
    let pos = cfg
        .explicit
        .or_else(|| hover_at().lock().ok().and_then(|g| *g));
    let Some(pos) = pos else {
        return;
    };
    // 指针只递两帧：egui 认为「每帧都在动」就不会弹气泡（time_since_last_movement
    // 一直归零），挪到位之后必须让它静止，位置本身在 PointerState 里跨帧保留。
    if frame <= 2 {
        raw_input.events.push(egui::Event::PointerMoved(pos));
    }
    // 离屏渲染几十帧只花几十毫秒，egui 的气泡要「静止 0.5 s」才弹，不推时间就永远等不到。
    raw_input.time = Some(frame as f64 * HOVER_STEP_SECS);
}

/// 截图前要预先滚动的距离（逻辑点，正数 = 内容往下走，看到更靠后的部分）。
fn scroll_setting() -> f32 {
    std::env::var("NUMDECK_GUI_SCROLL")
        .ok()
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v != 0.0)
        .unwrap_or(0.0)
}

/// 把滚动拆进前若干帧逐帧注入。
///
/// 三个都是踩过的点：① egui 的滚动带平滑，一帧塞一大坨会被速度曲线吃掉大半，
/// 拍出来只滚了一屏的一角；② 滚轮事件不带坐标，靠指针所在位置决定滚哪个区域，
/// 所以得先把指针放到内容区中间；③ 等滚完要留几帧让它停稳，滚动惯性未落时截图会糊。
fn scroll_hook(raw_input: &mut egui::RawInput, frame: u32) {
    let scroll = scroll_setting();
    if scroll == 0.0 {
        return;
    }
    const STEPS: u32 = 8;
    if frame <= 1 {
        if let Some(rect) = raw_input.screen_rect {
            raw_input
                .events
                .push(egui::Event::PointerMoved(rect.center()));
        }
    }
    if frame < STEPS {
        raw_input.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -scroll / STEPS as f32),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Unknown,
            modifiers: egui::Modifiers::NONE,
        });
    }
}

fn target_path() -> Option<PathBuf> {
    std::env::var_os("NUMDECK_GUI_SHOT").map(PathBuf::from)
}

/// 每帧调用一次。到点就请求截图，回调里落盘并退出。
pub fn tick(ctx: &egui::Context) {
    let Some(path) = target_path() else {
        return;
    };
    // 新的一帧：`tip` 的序号从头数
    TIPS_SEEN.store(0, Ordering::Relaxed);
    let frames = FRAMES.fetch_add(1, Ordering::Relaxed) + 1;
    let warmup = if hover_setting().is_some() {
        WARMUP_FRAMES_HOVER
    } else {
        WARMUP_FRAMES
    };
    if frames < warmup {
        ctx.request_repaint();
        return;
    }
    if REQUESTED.swap(true, Ordering::Relaxed) {
        return;
    }
    ctx.request_screenshot(move |image| {
        match write_ppm(&path, &image) {
            Ok(()) => eprintln!("已保存截图 {}", path.display()),
            Err(err) => eprintln!("保存截图失败：{err}"),
        }
        std::process::exit(0);
    });
}

/// 二进制 PPM（P6）：三字节一像素，任何图像工具都能读，不需要额外依赖。
fn write_ppm(path: &Path, image: &egui::ColorImage) -> std::io::Result<()> {
    let [width, height] = image.size;
    let mut bytes = Vec::with_capacity(width * height * 3 + 32);
    bytes.extend_from_slice(format!("P6\n{width} {height}\n255\n").as_bytes());
    for pixel in &image.pixels {
        bytes.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
    }
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(path, bytes)
}
