//! Linux 运行时：设备探测、背光、uinput 输出、主循环。

pub mod dev;
pub mod led;
pub use self::led::{
    probe_candidates, probe_feature_report, probe_i2c, probe_numpad_presence, FeatureProbe,
    LedCandidate, NumpadPresence,
};

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Context, Result};

use crate::config::Config;
use crate::engine::{Clock, Deps, Engine, Event, GrabSink, KeySink, NumLockSource, PointerSink};
use crate::keycodes as kc;
use crate::layout::Geometry;

use dev::{AbsInfo, InputDevice, RawInputEvent, VirtualDevice, MAX_EVENTS_PER_READ};

// --------------------------------------------------------------------------- //
// 时钟
// --------------------------------------------------------------------------- //

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> f64 {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
        ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1_000_000.0
    }
}

// --------------------------------------------------------------------------- //
// 输出侧
// --------------------------------------------------------------------------- //

struct UInputKeys {
    dev: Rc<RefCell<VirtualDevice>>,
}

impl KeySink for UInputKeys {
    fn press(&mut self, code: u16) {
        let dev = self.dev.borrow();
        let _ = dev.write_event(&Event::key(code, 1));
        let _ = dev.syn();
    }
    fn release(&mut self, code: u16) {
        let dev = self.dev.borrow();
        let _ = dev.write_event(&Event::key(code, 0));
        let _ = dev.syn();
    }
}

struct UInputPointer {
    dev: Rc<RefCell<VirtualDevice>>,
}

impl PointerSink for UInputPointer {
    fn feed(&mut self, ev: &Event) {
        // 只转发触摸板真实具备的码，避免 uinput 报 EINVAL
        let allowed = match ev.etype {
            kc::EV_ABS => kc::FORWARD_ABS_CODES.contains(&ev.code),
            kc::EV_KEY => kc::FORWARD_KEY_CODES.contains(&ev.code),
            kc::EV_SYN => true,
            _ => false,
        };
        if !allowed {
            return;
        }
        let dev = self.dev.borrow();
        let _ = dev.write_event(ev);
        if ev.etype == kc::EV_SYN {
            let _ = dev.syn();
        }
    }
}

struct DevGrab {
    dev: Rc<RefCell<InputDevice>>,
}

impl GrabSink for DevGrab {
    fn grab(&mut self) -> bool {
        match self.dev.borrow().grab() {
            Ok(()) => true,
            Err(err) => {
                eprintln!(
                    "{}",
                    crate::i18n::tf("dev.warn.grab", &[("err", &err.to_string())])
                );
                false
            }
        }
    }
    fn ungrab(&mut self) {
        let _ = self.dev.borrow().ungrab();
    }
}

struct LedBits {
    dev: Rc<RefCell<InputDevice>>,
}

impl NumLockSource for LedBits {
    fn numlock_on(&self) -> Option<bool> {
        self.dev.borrow().read_led_numl().ok()
    }
}

// --------------------------------------------------------------------------- //
// 设备探测
// --------------------------------------------------------------------------- //

fn is_numpad_candidate(dev: &InputDevice) -> bool {
    // 没有多点触控轴的一定不是触摸板（键盘 / 电源键 / 厂商控制设备全部排除）
    let has_mt = dev.supports(kc::EV_ABS, kc::ABS_MT_POSITION_X)
        && dev.supports(kc::EV_ABS, kc::ABS_MT_POSITION_Y);
    if !has_mt {
        return false;
    }
    // 0B05:1ACE 是 ROG 无线鼠标接收器，名字里也带 ROG，必须显式排除
    if dev.id.vendor == 0x0b05 && dev.id.product == 0x1ace {
        return false;
    }
    // 官方 NumberPad 触摸板模组的 HID 名：ASUE/ASUF/ASCP/ASUP/ASUX + 4 位序号，
    // 或 ELAN 前缀。命中即认为是目标触摸板——这一条让驱动跨机型通用，
    // 不依赖具体 vendor:product。
    if crate::layout::hid_id_of(&dev.name).is_some() {
        return true;
    }
    // HID 名字被裁剪的情况：ELAN（04F3）/ PixArt（093A，ASCP1400 模组）
    // 触摸板直接按 vendor 放行
    if dev.id.vendor == 0x04f3 || dev.id.vendor == 0x093a {
        return true;
    }
    let name = dev.name.to_lowercase();
    name.contains("numpad")
        || name.contains("numberpad")
        || name.contains("touchpad")
        || name.contains("asuf")
}

/// 布局解析结果：最终采用的布局、识别出的 HID 硬件 ID、以及依据来源。
#[derive(Debug, Clone)]
pub struct ResolvedLayout {
    pub entry: &'static crate::layout::LayoutEntry,
    /// 设备名里解析出的官方 HID 硬件 ID（如 `ASUE1416`），没识别到为 None。
    pub hid: Option<String>,
    /// `auto` 按 HID ID 识别 / `fixed` 配置写死 / `fallback` 未识别回落默认
    pub source: &'static str,
}

/// 决定这台机器该用哪套布局。
///
/// 判据优先级：配置写死的布局 > 设备名里的 HID 硬件 ID > 默认 GH3XLNDZ。
/// 布局由触摸板模组决定（ASUE1416 等），与整机型号无关——这就是同模组机型通用的原因。
pub fn resolve_layout(cfg: &Config, device_name: &str) -> ResolvedLayout {
    use crate::layout;
    let hid = layout::hid_id_of(device_name);
    if !cfg.layout_name.eq_ignore_ascii_case("auto") {
        if let Some(entry) = layout::by_name(&cfg.layout_name) {
            return ResolvedLayout {
                entry,
                hid,
                source: "fixed",
            };
        }
    }
    if let Some(entry) = layout::for_device_name(device_name) {
        return ResolvedLayout {
            entry,
            hid,
            source: "auto",
        };
    }
    // 认出了 NumberPad 模组 ID 但官方表里没有对应布局：绝不静默套用 GH3XLNDZ，
    // 否则其它机型会按错键。交给闸门拦下并要求反馈 `check` 输出。
    if hid.is_some() {
        let entry = layout::by_name("UNKNOWN").expect("内置布局表必须包含 UNKNOWN 占位");
        return ResolvedLayout {
            entry,
            hid,
            source: "unknown-hid",
        };
    }
    // 设备名里没有模组 ID：若 DMI 已把机型定下来，用该机型的模组表反查唯一键表，
    // 比直接套默认键表（那是本机验证过的 G634 布局）更贴近实际硬件。
    if let Some(name) = crate::sku::layout_name_for_local_machine(cfg.sku.as_deref()) {
        if let Some(entry) = layout::by_name(name) {
            return ResolvedLayout {
                entry,
                hid,
                source: "sku",
            };
        }
    }
    let entry = layout::by_name(layout::DEFAULT_LAYOUT_NAME).expect("内置布局表必须包含默认布局");
    ResolvedLayout {
        entry,
        hid,
        source: "fallback",
    }
}

/// 布局来源 → 文案键（`auto` / `fixed` 这类内部标识不直接展示给用户）。
fn layout_source_key(source: &str) -> &'static str {
    match source {
        "auto" => "layout.source.auto",
        "fixed" => "layout.source.fixed",
        "sku" => "layout.source.sku",
        "unknown-hid" => "layout.source.unknown_hid",
        _ => "layout.source.fallback",
    }
}

/// 未验证布局闸门：返回 Some(说明) 表示应停止，不再往触摸板上套键表。
///
/// `Family`（同结构、参数已照录但没上过真机）与 `Unsupported`（无键位依据）都会被拦，
/// 放行方式：配置 `allow_unverified_layout: true` 或命令行 `--allow-unverified`。
pub fn layout_gate(cfg: &Config, rl: &ResolvedLayout) -> Option<String> {
    use crate::layout::LayoutSupport;
    if !rl.entry.support.needs_confirm() {
        return None;
    }
    if cfg.allow_unverified_layout {
        return None;
    }
    let support = rl.entry.support.as_str();
    let hint = crate::i18n::t(if matches!(rl.entry.support, LayoutSupport::Unsupported) {
        "err.layout_unsupported.hint"
    } else {
        "err.layout_unverified.hint"
    });
    Some(crate::i18n::tf(
        "err.layout_unverified",
        &[
            ("layout", rl.entry.name),
            ("support", support),
            ("hint", &hint),
        ],
    ))
}

/// 找触摸板：优先配置覆盖，其次按名字/vendor + MT 能力匹配。
pub fn find_touchpad(cfg: &Config) -> Result<InputDevice> {
    if let Some(path) = &cfg.device_override {
        return InputDevice::open(path)
            .with_context(|| crate::i18n::tf("dev.open_failed", &[("path", path.as_str())]));
    }
    let mut seen = Vec::new();
    for path in dev::list_event_devices() {
        let Ok(d) = InputDevice::open(&path) else {
            continue;
        };
        seen.push(format!("{} ({})", path, d.name));
        if is_numpad_candidate(&d) {
            return Ok(d);
        }
    }
    let scanned = if seen.is_empty() {
        crate::i18n::t("dev.no_permission")
    } else {
        seen.join(", ")
    };
    Err(anyhow!(crate::i18n::tf(
        "dev.no_touchpad",
        &[("seen", &scanned)]
    )))
}

/// 找键盘：需要能读 LED 位（NumLock 指示灯）。
pub fn find_keyboard(cfg: &Config) -> Option<InputDevice> {
    if let Some(path) = &cfg.keyboard_override {
        return InputDevice::open(path).ok();
    }
    for path in dev::list_event_devices() {
        let Ok(d) = InputDevice::open(&path) else {
            continue;
        };
        if d.has_event_type(kc::EV_LED) && d.read_led_numl().is_ok() {
            return Some(d);
        }
    }
    None
}

/// ABS 轴 → 每微米 units 数。
fn abs_units_per_um(axis: &AbsInfo, nominal_mm: f64) -> f64 {
    if axis.resolution > 0 {
        return axis.resolution as f64 / 1000.0;
    }
    let span = (axis.maximum - axis.minimum).max(1) as f64;
    let span_um = (nominal_mm * 1000.0).max(1.0);
    span / span_um
}

fn geometry_from_abs(
    xi: &AbsInfo,
    yi: &AbsInfo,
    cfg: &Config,
    layout_name: &'static str,
) -> Geometry {
    // 内核 resolution 单位是 units/mm，换成 units/µm；没报 resolution 的模组按标称尺寸折算。
    // 分母一律 max(1)，避免 ABS 范围退化成单点时除零。
    let ux = abs_units_per_um(&xi, cfg.touchpad_width_mm);
    let uy = abs_units_per_um(&yi, cfg.touchpad_height_mm);
    Geometry::new(
        xi.minimum,
        xi.maximum,
        yi.minimum,
        yi.maximum,
        ux,
        uy,
        cfg.layout(),
    )
    .with_layout_name(layout_name)
}

/// 自检用：同时给出几何对象与探测结果字符串。
pub fn probe_report(cfg: &Config) -> Result<(Geometry, Vec<String>)> {
    let dev = find_touchpad(cfg)?;
    let mut cfg = cfg.clone();
    let rl = resolve_layout(&cfg, &dev.name);
    // 凡是自动判定的结果（auto / sku / fallback / unknown-hid）都要写回：后续几何与
    // 键表都从 cfg.layout() 取，不回写就会静默用上默认布局。
    if rl.source != "fixed" {
        cfg.layout_name = rl.entry.name.to_string();
    }
    let xi = dev
        .abs_info(kc::ABS_MT_POSITION_X)
        .context(crate::i18n::t("dev.no_abs_x"))?;
    let yi = dev
        .abs_info(kc::ABS_MT_POSITION_Y)
        .context(crate::i18n::t("dev.no_abs_y"))?;
    let geo = geometry_from_abs(&xi, &yi, &cfg, rl.entry.name);
    let mut out = vec![
        crate::i18n::tf(
            "dev.report.device",
            &[("path", &dev.path), ("name", &dev.name)],
        ),
        format!(
            "vendor:product = {:04x}:{:04x}",
            dev.id.vendor, dev.id.product
        ),
        format!(
            "x: {}..{} res={} fuzz={}",
            xi.minimum, xi.maximum, xi.resolution, xi.fuzz
        ),
        format!(
            "y: {}..{} res={} fuzz={}",
            yi.minimum, yi.maximum, yi.resolution, yi.fuzz
        ),
        crate::i18n::tf(
            "dev.report.touch_axes",
            &[
                (
                    "x",
                    &dev.supports(kc::EV_ABS, kc::ABS_MT_POSITION_X).to_string(),
                ),
                (
                    "y",
                    &dev.supports(kc::EV_ABS, kc::ABS_MT_POSITION_Y).to_string(),
                ),
                (
                    "tid",
                    &dev.supports(kc::EV_ABS, kc::ABS_MT_TRACKING_ID).to_string(),
                ),
                (
                    "p",
                    &dev.supports(kc::EV_ABS, kc::ABS_MT_PRESSURE).to_string(),
                ),
            ],
        ),
        crate::i18n::tf(
            "dev.report.keyboard_led",
            &[(
                "state",
                &crate::i18n::t(if find_keyboard(&cfg).is_some() {
                    "dev.report.keyboard_ok"
                } else {
                    "dev.report.keyboard_ng"
                }),
            )],
        ),
        crate::i18n::tf(
            "dev.report.backlight",
            &[("kind", &led::build_led(&cfg).channel())],
        ),
    ];
    out.push(crate::i18n::tf(
        "dev.report.layout",
        &[
            ("layout", rl.entry.name),
            ("support", rl.entry.support.as_str()),
            ("source", crate::i18n::t(layout_source_key(rl.source))),
            ("hid", rl.hid.as_deref().unwrap_or("-")),
        ],
    ));
    if let Some(warn) = layout_gate(&cfg, &rl) {
        out.push(warn);
    }
    out.push(geo.describe());
    Ok((geo, out))
}

/// 读失败到第几次才认定设备已经不在：偶发的 `EINTR` / `EAGAIN` 会被内核自己吸收，
/// 真到 `ENODEV`（拔掉、驱动卸载）时没必要再等——继续空转只会刷日志。
fn device_gone(err: &std::io::Error, failures: u32) -> bool {
    use std::io::ErrorKind;
    matches!(
        err.kind(),
        ErrorKind::NotFound | ErrorKind::BrokenPipe | ErrorKind::UnexpectedEof
    ) || err.raw_os_error() == Some(libc::ENODEV)
        || err.raw_os_error() == Some(libc::ENXIO)
        || failures >= 8
}

/// 打印原始事件（排查用）。
pub fn dump_events(cfg: &Config) -> Result<()> {
    let dev = find_touchpad(cfg)?;
    println!(
        "{}",
        crate::i18n::tf("dump.header", &[("path", &dev.path), ("name", &dev.name)])
    );
    let running = &STOP;
    while !running.load(Ordering::Relaxed) {
        dev.wait_readable(200)?;
        for raw in dev.read_events()? {
            let ev = raw.to_event();
            println!(
                "type=0x{:02x} code=0x{:03x} value={} {}",
                ev.etype,
                ev.code,
                ev.value,
                kc::name_of(ev.code)
            );
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------- //
// 主循环
// --------------------------------------------------------------------------- //

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_sig: libc::c_int) {
    // 只置位，真正的清理在主循环里做（signal handler 内不能分配内存）
    STOP.store(true, Ordering::SeqCst);
}

fn install_signals() {
    unsafe {
        let h = on_signal as *const () as libc::sighandler_t;
        libc::signal(libc::SIGINT, h);
        libc::signal(libc::SIGTERM, h);
        libc::signal(libc::SIGHUP, h);
    }
}

pub fn run_service(cfg: Config, verbose: bool) -> Result<()> {
    install_signals();
    STOP.store(false, Ordering::SeqCst);

    let tpad = Rc::new(RefCell::new(find_touchpad(&cfg)?));
    let kb = find_keyboard(&cfg).map(|d| Rc::new(RefCell::new(d)));

    // 布局先定死：`auto` 时按触摸板 HID 硬件 ID 自动识别，未验证的布局默认拦下
    let mut cfg = cfg;
    let rl = resolve_layout(&cfg, &tpad.borrow().name);
    if let Some(msg) = layout_gate(&cfg, &rl) {
        return Err(anyhow!(msg));
    }
    if rl.source != "fixed" {
        cfg.layout_name = rl.entry.name.to_string();
    }
    let layout_name = rl.entry.name;

    let xi = tpad
        .borrow()
        .abs_info(kc::ABS_MT_POSITION_X)
        .context(crate::i18n::t("dev.no_abs_x"))?;
    let yi = tpad
        .borrow()
        .abs_info(kc::ABS_MT_POSITION_Y)
        .context(crate::i18n::t("dev.no_abs_y"))?;
    let has_mt = tpad.borrow().supports(kc::EV_ABS, kc::ABS_MT_TRACKING_ID);
    let has_pressure = tpad.borrow().supports(kc::EV_ABS, kc::ABS_MT_PRESSURE);
    let geometry = geometry_from_abs(&xi, &yi, &cfg, layout_name);

    let keyboard_dev = Rc::new(RefCell::new(VirtualDevice::create_keyboard("asus-numpad")?));

    // 混合模式需要把未吞掉的触点转发回去，否则开数字键盘后光标就没了
    let pointer_dev = if cfg.mixed_mode.is_enabled() && has_mt {
        let ranges = vec![
            (
                kc::ABS_MT_SLOT,
                AbsInfo {
                    minimum: 0,
                    maximum: 4,
                    ..Default::default()
                },
            ),
            (
                kc::ABS_MT_TRACKING_ID,
                AbsInfo {
                    minimum: -1,
                    maximum: 65535,
                    ..Default::default()
                },
            ),
            (kc::ABS_MT_POSITION_X, xi),
            (kc::ABS_MT_POSITION_Y, yi),
            (kc::ABS_X, xi),
            (kc::ABS_Y, yi),
        ];
        let mut keys = vec![kc::BTN_TOUCH, kc::BTN_TOOL_FINGER, kc::BTN_TOOL_DOUBLETAP];
        keys.extend_from_slice(&kc::ALL_KEYS);
        match VirtualDevice::create_touchpad("asus-numpad-touchpad", &ranges, &keys) {
            Ok(d) => Some(Rc::new(RefCell::new(d))),
            Err(err) => {
                eprintln!(
                    "{}",
                    crate::i18n::tf("dev.warn.uinput", &[("err", &err.to_string())])
                );
                None
            }
        }
    } else {
        None
    };

    let deps = Deps {
        clock: Box::new(SystemClock),
        led: led::build_led(&cfg),
        keys: Box::new(UInputKeys { dev: keyboard_dev }),
        pointer: pointer_dev
            .clone()
            .map(|d| Box::new(UInputPointer { dev: d }) as Box<dyn PointerSink>),
        numlock: kb
            .clone()
            .map(|d| Box::new(LedBits { dev: d }) as Box<dyn NumLockSource>),
        grabber: Some(Box::new(DevGrab { dev: tpad.clone() }) as Box<dyn GrabSink>),
    };

    let mut engine = Engine::new(cfg.clone(), geometry, deps, has_mt, has_pressure);
    if verbose {
        println!("{}", crate::banner());
        for line in engine.geometry_report() {
            println!("  {line}");
        }
        println!(
            "{}",
            crate::i18n::tf(
                "run.device",
                &[("path", &tpad.borrow().path), ("name", &tpad.borrow().name)]
            )
        );
    }

    let frame_ms = cfg.frame_ms.max(1) as i32;
    let mut last_state = String::new();
    // 事件缓冲跨帧复用：主循环每秒跑 125 次，不必每帧重新分配
    let mut tpad_events: Vec<RawInputEvent> = Vec::with_capacity(MAX_EVENTS_PER_READ);
    let mut kb_events: Vec<RawInputEvent> = Vec::with_capacity(8);
    // 连续读失败次数：偶发 EINTR/EIO 不该退出，设备真没了才收工
    let mut read_failures = 0u32;

    while engine.is_running() && !STOP.load(Ordering::Relaxed) {
        // 1) 触摸板事件
        let ready = match tpad.borrow().wait_readable(frame_ms) {
            Ok(ready) => ready,
            Err(err) => {
                read_failures += 1;
                if device_gone(&err, read_failures) {
                    println!(
                        "{}",
                        crate::i18n::tf("run.device_lost", &[("err", &err.to_string())])
                    );
                    break;
                }
                false
            }
        };
        if ready {
            match tpad.borrow().read_events_into(&mut tpad_events) {
                Ok(()) => {
                    read_failures = 0;
                    for raw in &tpad_events {
                        engine.handle_event(&raw.to_event());
                    }
                }
                Err(err) => {
                    read_failures += 1;
                    if device_gone(&err, read_failures) {
                        println!(
                            "{}",
                            crate::i18n::tf("run.device_lost", &[("err", &err.to_string())])
                        );
                        break;
                    }
                }
            }
        }

        // 2) 物理键盘 LED 变化（NumLock 按下）
        if let Some(kb_dev) = &kb {
            // 键盘只用来读 NumLock 灯，读失败（例如键盘被热插拔换掉）不影响数字区
            if kb_dev.borrow().read_events_into(&mut kb_events).is_ok() {
                for raw in &kb_events {
                    let ev = raw.to_event();
                    if ev.etype == kc::EV_LED
                        || (ev.etype == kc::EV_KEY && ev.code == kc::KEY_NUMLOCK)
                    {
                        engine.handle_kb_event(&ev);
                    }
                }
            }
        }

        // 3) 帧驱动
        engine.tick();

        if verbose {
            let line = engine.state_line();
            if line != last_state {
                println!("{line}");
                last_state = line;
            }
        }
    }

    engine.shutdown();
    if verbose {
        for line in engine.logs() {
            println!("  {line}");
        }
    }
    Ok(())
}
