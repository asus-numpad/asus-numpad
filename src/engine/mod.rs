//! 官方 Algo 状态机（纯逻辑，不依赖任何 Linux 系统调用）。
//!
//! ```text
//! IDLE ──触点落在 NumLock 区──> NUMLOCK_PENDING
//!      │                         ├─ 连续 62 帧仍在区内且漂移未超 2741µm → 切换开/关
//!      │                         └─ 漂移超限累计 3 次 → 取消
//!      ├─落在计算器图标区(LT)──> ADV_PENDING（40 帧 = 320 ms）→ 发 KEY_CALC
//!      └─落在键区────────────> KEY_DOWN
//!                                ├─ 移动 ≥ 4838µm → 取消按键、进入 GESTURE
//!                                └─ 按住 ≥ 25 帧 → 连续发键（repeat）
//! ```
//!
//! 点灯状态机（与 NumLock 联动）：
//! ```text
//! LIT ──120 s 无操作(NUMPAD_02)──> DIM（仅熄灯，enabled 保持，触摸板释放）
//! DIM ──任意触摸且 NumLock on──> LIT（0x60 → 0x01，重新接管）
//! LIT ──外部关闭 NumLock──> DIM ；DIM ──外部打开 NumLock──> LIT
//! ```
//!
//! 硬件读写全部通过 trait 注入，因此本模块可在任意平台跑单元测试。

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::brightness::BrightnessFollower;
use crate::config::{
    CalcMode, Config, MixedMode, NumLockMode, CALCULATOR_FEATURE, NUMLOCK_MAX_FAIL_DELTA_RATIO,
    VAL_OFF, VAL_ON, VAL_UNLOCK,
};
use crate::keycodes as kc;
use crate::layout::{Geometry, KeyMode};

mod recover;
mod report;

/// 与 Linux input 事件同构的最小事件（透传时需要原样回放）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub etype: u16,
    pub code: u16,
    pub value: i32,
}

impl Event {
    pub fn abs(code: u16, value: i32) -> Self {
        Self {
            etype: kc::EV_ABS,
            code,
            value,
        }
    }
    pub fn key(code: u16, value: i32) -> Self {
        Self {
            etype: kc::EV_KEY,
            code,
            value,
        }
    }
    pub fn led(code: u16, value: i32) -> Self {
        Self {
            etype: kc::EV_LED,
            code,
            value,
        }
    }
}

/// 时间源（毫秒）。测试注入假时钟。
pub trait Clock {
    fn now_ms(&self) -> f64;
}

/// 背光通道（hidraw 或 i2c）。
pub trait LedSink {
    fn send(&mut self, value: u8) -> bool;
    fn channel(&self) -> String;
    /// 该通道是否需要在点亮前先下发解锁字节 `0x60`。
    ///
    /// i2c 直写 EC 走官方两段式序列，需要；hidraw FEATURE 报告一步到位，不需要。
    fn needs_unlock(&self) -> bool {
        true
    }
    /// 已成功下发的取值序列（自检/测试用）。
    fn history(&self) -> Vec<u8> {
        Vec::new()
    }
}

/// 虚拟键盘。
pub trait KeySink {
    fn press(&mut self, code: u16);
    fn release(&mut self, code: u16);
}

/// 虚拟触摸板（把未被吞掉的触点还给桌面）。
pub trait PointerSink {
    fn feed(&mut self, ev: &Event);
}

/// 物理键盘 NumLock 指示灯读取。
pub trait NumLockSource {
    fn numlock_on(&self) -> Option<bool>;
}

/// 触摸板 grab/ungrab。
pub trait GrabSink {
    fn grab(&mut self) -> bool;
    fn ungrab(&mut self);
}

/// 依赖集合。
pub struct Deps {
    pub led: Box<dyn LedSink>,
    pub keys: Box<dyn KeySink>,
    pub pointer: Option<Box<dyn PointerSink>>,
    pub numlock: Option<Box<dyn NumLockSource>>,
    pub grabber: Option<Box<dyn GrabSink>>,
    pub clock: Box<dyn Clock>,
}

/// 触点状态机的当前模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    None,
    Numlock,
    Numadv,
    Armed,
    Key,
    Gesture,
    Scroll,
    Pinch,
    AppGesture,
    /// 已触发（NumLock/计算器），等待抬手，避免重复触发。
    Fired,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::None => "idle",
            Mode::Numlock => "numlock",
            Mode::Numadv => "numadv",
            Mode::Armed => "armed",
            Mode::Key => "key",
            Mode::Gesture => "gesture",
            Mode::Scroll => "scroll",
            Mode::Pinch => "pinch",
            Mode::AppGesture => "appgesture",
            Mode::Fired => "fired",
        }
    }

    fn is_gesture_family(self) -> bool {
        matches!(
            self,
            Mode::Gesture | Mode::Scroll | Mode::Pinch | Mode::AppGesture
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    WasOff,
    Toggled,
}

pub struct Engine {
    pub cfg: Config,
    pub geometry: Geometry,

    led: Box<dyn LedSink>,
    /// 键盘背光跟随器：读 sysfs 换算数字区亮度档
    brightness: BrightnessFollower,
    keys: Box<dyn KeySink>,
    pointer: Option<Box<dyn PointerSink>>,
    numlock: Option<Box<dyn NumLockSource>>,
    grabber: Option<Box<dyn GrabSink>>,
    clock: Box<dyn Clock>,

    tol_ux: f64,
    tol_uy: f64,

    pub enabled: bool,
    pub lit: bool,
    numlock_restore: Option<Restore>,
    /// 刚由我们主动点亮 NumLock，LED 事件尚未回流
    numlock_forced: bool,
    idle_blanked: bool,
    grabbed: bool,
    running: bool,

    slot: i32,
    primary: Option<i32>,
    has_mt_tracking: bool,
    last_activity_ms: f64,
    /// 上次尝试 grab 的时刻：失败后按 `lifecycle::GRAB_RETRY_MS` 节奏重试
    grab_retry_at: f64,
    last_tap: HashMap<u16, f64>,

    gesture_seen: bool,
    gesture_end_ms: f64,
    gesture_end_xy: Option<(i32, i32)>,

    active_slots: BTreeSet<i32>,
    slot_pos: HashMap<i32, (Option<i32>, Option<i32>)>,

    // ---- 当前触点 ----
    down: bool,
    pub mode: Mode,
    scroll_spread_mm: f64,
    t0: f64,
    x0: i32,
    y0: i32,
    pub cur_x: i32,
    pub cur_y: i32,
    fail_count: u32,
    pub moved_units: f64,
    pressed_key: Option<u16>,
    repeat_next: f64,
    fired: bool,
    pending_down: bool,
    got_x: bool,
    got_y: bool,
    pending_since: f64,
    armed_key: Option<u16>,
    armed_since: f64,
    armed_delay_frames: u32,
    pending_events: Vec<Event>,
    forwarding: bool,
    /// 两段式点灯里「第二段亮度档」的待下发值与最早可下发时刻。
    ///
    /// 早期实现直接在主循环里 `thread::sleep(15ms)`，等于每个事件都可能卡掉一帧半，
    /// 触摸响应肉眼可见地拖后。改成记下时刻、由 `tick` 在到期后的第一帧补发，
    /// 既保住 EC 需要的间隔，又不再阻塞事件读取。
    pending_level: Option<u8>,
    pending_level_at: f64,

    // ---- 自定义扩展 ----
    /// 当前布局的键表：自定义布局用配置里的，其余用内置表。
    ///
    /// 内置表是 5 列固定表，自定义布局的列数由配置决定（贴纸方案常见 4 列），
    /// 所以键表不能再当全局常量用，必须跟着引擎走。
    table: crate::custom::KeyTable,
    /// 压感触摸板的「用力按 = 切换」状态机。
    pressure: crate::custom::PressureGate,
    /// 压力轴满量程（`EVIOCGABS` 读出；`0` = 该触摸板没有压力轴）。
    pressure_max: i32,
    /// 最近一次压力采样值。
    pressure_now: i32,
    /// 最近一次压感切换的时刻（状态行/日志用）。
    pressure_last_ms: f64,
    /// 物理键盘上的全局组合键状态机（`custom::HotkeySpec` 解析结果）。
    hotkeys: crate::custom::HotkeyState,
    /// 最近一次热键触发的动作名（状态行用）。
    hotkey_last: Option<&'static str>,

    echo: bool,
    logs: VecDeque<String>,
}

/// 判定前的暂存上限：一次触点最多几十个事件，超过这个量只能是事件风暴或判定卡住。
const MAX_PENDING_EVENTS: usize = 4096;

/// 日志缓冲上限：够看清一次触摸到按键的完整判定链，又不会随运行时长增长。
const MAX_LOG_LINES: usize = 256;

/// 参与指距计算的触点数上限。
///
/// 触摸板最多同时上报 5 个槽位，留一点余量；超过的部分不参与 GESTURE_08 判定，
/// 也不影响单指/双指的主流程。
const MAX_TOUCH_SLOTS: usize = 8;

/// 两段式点灯中，点亮命令与亮度档之间的最小间隔（毫秒）。
///
/// 取值来自真机调试：低于 15 ms 时部分机型的 EC 会丢掉紧跟其后的档位写入。
const STAGED_LEVEL_GAP_MS: f64 = 15.0;

impl Engine {
    pub fn new(cfg: Config, geometry: Geometry, deps: Deps, has_mt_tracking: bool) -> Self {
        let tol_ux = cfg.tap_threshold_mm * 1000.0 * geometry.ux;
        let tol_uy = cfg.tap_threshold_mm * 1000.0 * geometry.uy;
        let now = deps.clock.now_ms();
        // 必须在 cfg 被移入 Self 之前构造
        let brightness =
            BrightnessFollower::new(&cfg.keyboard_brightness_dir, cfg.effective_levels());
        let table = crate::custom::table_for(&cfg.layout_name);
        let pressure = crate::custom::PressureGate::new(cfg.pressure_switch);
        let hotkeys = crate::custom::HotkeyState::new(&cfg.hotkeys).0;
        Self {
            cfg,
            geometry,
            led: deps.led,
            brightness,
            keys: deps.keys,
            pointer: deps.pointer,
            numlock: deps.numlock,
            grabber: deps.grabber,
            clock: deps.clock,
            tol_ux,
            tol_uy,
            enabled: false,
            lit: false,
            numlock_restore: None,
            numlock_forced: false,
            idle_blanked: false,
            grabbed: false,
            grab_retry_at: now,
            running: true,
            slot: 0,
            primary: None,
            has_mt_tracking,
            last_activity_ms: now,
            last_tap: HashMap::new(),
            gesture_seen: false,
            gesture_end_ms: 0.0,
            gesture_end_xy: None,
            active_slots: BTreeSet::new(),
            slot_pos: HashMap::new(),
            down: false,
            mode: Mode::None,
            scroll_spread_mm: 0.0,
            t0: 0.0,
            x0: 0,
            y0: 0,
            cur_x: 0,
            cur_y: 0,
            fail_count: 0,
            moved_units: 0.0,
            pressed_key: None,
            repeat_next: 0.0,
            fired: false,
            pending_down: false,
            got_x: false,
            got_y: false,
            pending_since: 0.0,
            armed_key: None,
            armed_since: 0.0,
            armed_delay_frames: 0,
            pending_events: Vec::new(),
            forwarding: false,
            pending_level: None,
            pending_level_at: 0.0,
            table,
            pressure,
            pressure_max: 0,
            pressure_now: 0,
            pressure_last_ms: f64::NEG_INFINITY,
            hotkeys,
            hotkey_last: None,
            echo: true,
            logs: VecDeque::new(),
        }
    }

    /// 最近若干条日志（最新的在最后）。
    ///
    /// 用 `VecDeque` 而非 `Vec`：封顶淘汰走 `pop_front`，不再每次搬动整个数组。
    pub fn logs(&self) -> impl Iterator<Item = &String> {
        self.logs.iter()
    }

    fn log(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        if self.echo {
            println!("{m}");
        }
        self.remember(m);
    }

    fn warn(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        if self.echo {
            eprintln!("{m}");
        }
        self.remember(m);
    }

    /// 记一行日志，只留最近 [`MAX_LOG_LINES`] 条。
    ///
    /// 服务模式下 digit 区一整天不退出，日志必须封顶：早期版本无上限 push，
    /// 一块常亮几周的机器会慢慢把内存吃光。
    fn remember(&mut self, line: String) {
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }

    fn now(&self) -> f64 {
        self.clock.now_ms()
    }

    // ------------------------------------------------------------------ //
    // 基础换算
    // ------------------------------------------------------------------ //

    /// 自 `t0` 起经过的帧数（官方帧模型 8 ms/帧）。
    pub fn frames_since(&self, t0: f64) -> f64 {
        recover::frames_between(t0, self.now(), self.cfg.frame_ms)
    }

    pub fn numlock_frames(&self) -> u32 {
        self.cfg.numlock_frames()
    }

    /// 两点实际距离（µm）。
    pub fn dist_um(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> f64 {
        recover::dist_um(&self.geometry, x0, y0, x1, y1)
    }

    // ------------------------------------------------------------------ //
    // 混合模式判定
    // ------------------------------------------------------------------ //

    /// GESTURE_00 = dwSupport_MixedMode = 1。
    ///
    /// 官方语义是数字键与光标共用同一块触摸板；没有虚拟触摸板就没法把光标
    /// 事件还回去，此时退化为纯数字键盘，不假装支持。
    pub fn mixed_active(&self) -> bool {
        if !self.enabled {
            // 未 grab 时原始事件本来就会进桌面，再转发一次等于双倍光标
            return false;
        }
        if !self.cfg.mixed_mode.is_enabled() {
            return false;
        }
        self.pointer.is_some()
    }

    /// 数字键盘点亮接管期间，单指不再当光标。
    ///
    /// 官方 `dwSupport_MixedMode = 1` 要的是「按键与指针并存」，不是「键区外一律透传」：
    /// 触摸板四周留白、行列之间的缝隙落在格区之外，按不到键却会把光标拖走，
    /// 从边缘起步时尤其明显。点亮态下这些单指触点直接吞掉，双指滚动/手势仍在
    /// `enter_multi` 里走 [`Self::mixed_active`]，不受影响。
    /// `mixed_mode = on` 可恢复旧行为（键区外一律透传）。
    fn single_pointer_forward(&self) -> bool {
        self.mixed_active() && !(self.enabled && self.lit && self.cfg.mixed_mode != MixedMode::On)
    }

    /// GESTURE_03/04 = 350 ms：上一次手势结束后，原地再次按下的判定窗口。
    fn tap_window_open(&self) -> bool {
        if !self.gesture_seen {
            return false;
        }
        let Some((ex, ey)) = self.gesture_end_xy else {
            return false;
        };
        if (self.now() - self.gesture_end_ms) > self.cfg.tap_interval_min_ms {
            return false;
        }
        self.dist_um(ex, ey, self.cur_x, self.cur_y) <= self.cfg.tap_threshold_mm * 1000.0
    }

    /// GESTURE_01/02：按键不立即发出，先等 80 ms 确认这不是一次滑动。
    fn arm_key(&mut self, key: u16) {
        self.mode = Mode::Armed;
        self.armed_key = Some(key);
        self.armed_since = self.now();
        let mut frames = self.cfg.touch_sendkey_frames;
        if self.tap_window_open() {
            // 刚做完手势又在原地按：延迟放大 3 倍（GESTURE_02）
            frames = frames.saturating_mul(self.cfg.touch_sendkey_ratio);
        }
        self.armed_delay_frames = frames;
        // 注意：不在这里清 pending_events。按下点判定要等 X/Y 到齐，那时按下序列的
        // 头几个事件（SLOT/TRACKING_ID/首个坐标）已经在缓冲里，清掉会让手势补发缺帧。
    }

    /// 判为手势：把推迟的这段事件补发给桌面，光标不会丢掉起步位移。
    fn flush_pending(&mut self) {
        if let Some(ptr) = self.pointer.as_mut() {
            let mut events = std::mem::take(&mut self.pending_events);
            for ev in &events {
                ptr.feed(ev);
            }
            // 缓冲留着复用：手势判定的次数远高于按键确认，反复重新分配没有意义
            events.clear();
            self.pending_events = events;
        } else {
            self.pending_events.clear();
        }
    }

    /// GESTURE_05 = 4838 µm：位移超阈值即判手势，取消出键。
    fn promote_gesture(&mut self) {
        self.armed_key = None;
        self.mode = Mode::Gesture;
        self.gesture_seen = true;
        self.forwarding = self.single_pointer_forward();
        if self.forwarding {
            self.flush_pending();
        } else {
            // 数字键盘点亮期间：滑动只取消这次按键，不把触点交给桌面
            self.pending_events.clear();
        }
    }

    /// 多指间距（GESTURE_08 = 40 mm 只约束滚动，不约束单指手势）。
    fn slot_distance_um(&self) -> Option<f64> {
        // 真机 SLOT→TRACKING_ID→X→Y 逐条到达，X 有值时 Y 可能还没来，不能拿去算。
        // 这个函数每个 X/Y 事件都会被调用一次，旧实现先 collect 成 Vec 再两两比，
        // 每帧几十次小分配；这里用固定大小栈数组，零分配。
        let mut pts = [(0i32, 0i32); MAX_TOUCH_SLOTS];
        let mut n = 0usize;
        for p in self.slot_pos.values() {
            if n >= MAX_TOUCH_SLOTS {
                break;
            }
            if let (Some(x), Some(y)) = p {
                pts[n] = (*x, *y);
                n += 1;
            }
        }
        if n < 2 {
            return None;
        }
        let mut best = 0.0_f64;
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.dist_um(pts[i].0, pts[i].1, pts[j].0, pts[j].1);
                if d > best {
                    best = d;
                }
            }
        }
        Some(best)
    }

    // ------------------------------------------------------------------ //
    // 事件注入
    // ------------------------------------------------------------------ //

    fn press(&mut self, code: u16) {
        self.keys.press(code);
    }

    fn release(&mut self, code: u16) {
        self.keys.release(code);
    }

    fn click(&mut self, code: u16) {
        self.press(code);
        self.release(code);
    }

    // ------------------------------------------------------------------ //
    // NumLock 联动
    // ------------------------------------------------------------------ //

    fn numlock_on(&self) -> Option<bool> {
        self.numlock.as_ref().and_then(|n| n.numlock_on())
    }

    fn numlock_enable(&mut self) {
        match self.cfg.numlock {
            NumLockMode::Off => (),
            NumLockMode::Sync => {
                if self.numlock_on() == Some(false) {
                    self.click(kc::KEY_NUMLOCK);
                    self.numlock_restore = Some(Restore::WasOff);
                    // LED 状态要等内核回传，本次点灯先按「已开」处理
                    self.numlock_forced = true;
                }
            }
            NumLockMode::Toggle => {
                self.click(kc::KEY_NUMLOCK);
                self.numlock_restore = Some(Restore::Toggled);
                self.numlock_forced = true;
            }
        }
    }

    fn numlock_restore_state(&mut self) {
        if self.numlock_restore.take().is_some() {
            self.click(kc::KEY_NUMLOCK);
        }
    }

    // ------------------------------------------------------------------ //
    // grab / 背光
    // ------------------------------------------------------------------ //

    fn grab(&mut self) {
        if !self.cfg.grab || self.grabbed {
            return;
        }
        let ok = match self.grabber.as_mut() {
            Some(g) => g.grab(),
            None => false,
        };
        if ok {
            self.grabbed = true;
        } else {
            self.warn(crate::i18n::t("engine.warn.grab"));
        }
    }

    fn ungrab(&mut self) {
        if !self.grabbed {
            return;
        }
        if let Some(g) = self.grabber.as_mut() {
            g.ungrab();
        }
        self.grabbed = false;
    }

    /// 点灯取值：
    /// - 跟随键盘背光开启 → 键盘背光换算档（`0x41..=0x48`，读不到时 `0x01`）；
    /// - 跟随关闭 → 手动三档里选定的固定档（`bright 1|2|3`，`bright 0` 表示不点亮）。
    fn on_value(&mut self) -> u8 {
        if self.brightness.enabled() {
            self.brightness.on_value()
        } else {
            self.cfg.static_on_value()
        }
    }

    /// 点灯：i2c 通道先解锁 0x60 再下发亮度；hidraw FEATURE 通道直接下发。
    /// NumLock 为 off 时不点灯（官方联动）。
    ///
    /// 下发值优先取「跟随键盘背光」换算出的亮度档 `0x41..=0x48`；读不到键盘背光
    /// （非 ASUS 机器 / asus-wmi 未加载）时退回普通点亮值 `0x01`。
    fn led_on(&mut self, reason: &str) -> bool {
        if self.lit {
            return false;
        }
        let state = self.numlock_on();
        if state == Some(false) && !self.numlock_forced {
            return false;
        }
        if self.led.needs_unlock() {
            self.led.send(VAL_UNLOCK);
        }
        let value = self.on_value();
        // 两段式点亮：先发 0x01（官方两态里真机验证过的「亮」），再叠加亮度档。
        // 部分机型的 EC 在未点亮时不吃 0x41..=0x48 档位，只发档位就是「完全不亮」，
        // 而 0x01 是硬件层面无条件点亮的命令字，先拿它把灯打开再做调光最稳。
        let staged = value != VAL_ON && value != VAL_OFF;
        if staged && !self.led.send(VAL_ON) {
            return false;
        }
        let sent = if staged {
            // EC 处理 FEATURE 是异步的，紧接着写档位有概率被丢掉，需要隔开一小段。
            // 这里不睡眠：只记下待下发档位与最早可下发时刻，由 tick 在到期后的
            // 第一帧补发——主循环不必为一次点灯停摆 15 ms。
            self.pending_level = Some(value);
            self.pending_level_at = self.now() + STAGED_LEVEL_GAP_MS;
            true
        } else {
            self.led.send(value)
        };
        if !sent && !staged {
            // 通道不通时不置位：否则 lit=true 会让后续唤醒/联动全都跳过重试，
            // 表现为「灯永远不亮」。保持 lit=false，下次触摸或 NumLock 变化会再试。
            return false;
        }
        // 档位那一步失败时灯已经由 0x01 点亮，不把整次点灯判失败：否则 lit 保持 false，
        // 每个事件都会重复点亮，日志被刷爆，而用户看到的灯其实是亮的。
        self.lit = true;
        if !reason.is_empty() {
            let msg = crate::i18n::tf("engine.state.lit", &[("reason", reason)]);
            self.log(msg);
        }
        true
    }

    fn led_off(&mut self, reason: &str) -> bool {
        // 待下发的亮度档必须先撤掉：熄灭后再补发档位会把灯重新点亮
        self.pending_level = None;
        if !self.lit {
            return false;
        }
        self.led.send(VAL_OFF);
        self.lit = false;
        if !reason.is_empty() {
            let msg = crate::i18n::tf("engine.state.dark", &[("reason", reason)]);
            self.log(msg);
        }
        true
    }

    // ------------------------------------------------------------------ //
    // 启用 / 停用 / 空闲
    // ------------------------------------------------------------------ //

    pub fn enable(&mut self) {
        if self.enabled {
            self.led_on(&crate::i18n::t("engine.reason.enable"));
            return;
        }
        self.enabled = true;
        self.touch_activity();
        // 官方联动：先保证 NumLock on，再点灯
        self.numlock_enable();
        self.grab();
        self.led_on(&crate::i18n::t("engine.reason.enter"));
        self.log(crate::i18n::t("engine.state.enter"));
    }

    /// 挂起恢复（或长时间阻塞）之后重新对齐硬件状态。
    ///
    /// 内部状态和硬件在挂起期间会走散：EC 侧背光被清零、LED 类设备可能被重建，
    /// 而引擎里 `lit` 仍是 true。此时不能等下一次亮度变化——`lit` 为 true 会让
    /// 空闲判定继续按「已经亮着」走，用户看到的就是「合盖唤醒后数字区彻底不亮」。
    /// 这里主动作废亮度缓存并按当前键盘背光重发一次；本来是熄灭状态就保持熄灭。
    pub fn resync_after_resume(&mut self) {
        self.brightness.invalidate();
        if !self.lit {
            return;
        }
        // 硬件状态已不可信：把内部标记放掉，走一遍完整点亮流程（含两段式下发
        // 与 NumLock 判定），而不是只补一条档位命令
        self.lit = false;
        self.led_on(&crate::i18n::t("engine.state.resync"));
    }

    pub fn disable(&mut self) {
        if !self.enabled {
            return;
        }
        self.led_off(&crate::i18n::t("engine.reason.exit"));
        self.numlock_restore_state();
        self.numlock_forced = false;
        // 退出前把手里的键放开：uinput 节点销毁时内核也会补释放，但「先灭灯、
        // 再松 grab」之间还有几毫秒，这几毫秒里用户看到的是一颗卡住的键。
        if let Some(k) = self.pressed_key.take() {
            self.release(k);
        }
        self.ungrab();
        self.enabled = false;
        self.armed_key = None;
        self.pending_events.clear();
        self.forwarding = false;
        self.active_slots.clear();
        self.slot_pos.clear();
        self.reset_contact();
        self.log(crate::i18n::t("engine.state.exit"));
    }

    /// 官方 NUMPAD_02 = 120 s：只熄灭背光，**不退**出数字键盘。
    ///
    /// 灯灭期间触摸板可以当光标用；再次触摸且仍处于 NumLock on 时自动复亮。
    fn idle_blank(&mut self) {
        self.idle_blanked = true;
        // 熄灯后会 ungrab，该触点后续的抬手事件再也收不到；
        // 若此刻有键处于按下态必须立刻释放，否则该数字键永久卡在按下（系统持续连发）
        if let Some(k) = self.pressed_key.take() {
            self.release(k);
        }
        self.mode = Mode::None;
        self.armed_key = None;
        self.pending_events.clear();
        self.forwarding = false;
        let reason = crate::i18n::tf(
            "engine.reason.idle",
            &[("sec", &(self.cfg.max_work_time_ms / 1000).to_string())],
        );
        self.led_off(&reason);
        self.ungrab();
    }

    /// 熄灯态被触摸：NumLock 仍 on 就重新点灯并重新接管触摸板。
    fn wake(&mut self) {
        if !self.enabled || self.lit {
            return;
        }
        if self.numlock_on() == Some(false) {
            return;
        }
        self.grab();
        self.led_on(&crate::i18n::t("engine.reason.touch"));
    }

    pub fn toggle(&mut self) {
        if self.enabled {
            self.disable();
        } else {
            self.enable();
        }
    }

    pub fn fire_calc(&mut self) {
        if CALCULATOR_FEATURE == 0 {
            return;
        }
        self.click(kc::KEY_CALC);
        self.log(crate::i18n::t("engine.state.calc"));
    }

    fn touch_activity(&mut self) {
        self.last_activity_ms = self.now();
        self.idle_blanked = false;
    }

    fn reset_contact(&mut self) {
        self.down = false;
        self.mode = Mode::None;
        self.scroll_spread_mm = 0.0;
        self.t0 = 0.0;
        self.fail_count = 0;
        self.moved_units = 0.0;
        self.fired = false;
        self.pending_down = false;
        self.got_x = false;
        self.got_y = false;
        self.pending_since = 0.0;
        self.armed_key = None;
        self.armed_since = 0.0;
        self.armed_delay_frames = 0;
        self.forwarding = false;
        // 注：active_slots / slot_pos 是物理触点状态，由 track_slots 独立维护，
        // 不随单次触点重置而清空（否则双指滚动中先抬一指会把状态算错）
    }

    // ------------------------------------------------------------------ //
    // 触点状态机
    // ------------------------------------------------------------------ //

    fn begin_contact(&mut self) {
        self.down = true;
        self.t0 = self.now();
        self.x0 = self.cur_x;
        self.y0 = self.cur_y;
        self.fail_count = 0;
        self.moved_units = 0.0;
        self.fired = false;
        self.pressed_key = None;
        self.armed_key = None;
        self.forwarding = false;
        self.touch_activity();

        // 熄灯态下任何触摸即唤醒复亮（官方：空闲只熄灯，重新触摸恢复）
        self.wake();

        if self.geometry.in_numlock(self.cur_x, self.cur_y) {
            self.mode = Mode::Numlock;
            return;
        }
        if self.cfg.calc_gesture
            && self.cfg.calc_gesture_mode == CalcMode::Long
            && self.geometry.in_calc_zone(self.cur_x, self.cur_y)
            && !self.enabled
        {
            self.mode = Mode::Numadv;
            return;
        }
        if self.enabled && self.numlock_on() != Some(false) {
            // NumLock 被外部关掉时灯已灭，数字区不再出键
            if self.emit_at_down_point() {
                return;
            }
        }
        self.mode = Mode::None;
        // 混合模式：落在键区外的触点就是普通光标，整段还给桌面；
        // 数字键盘点亮期间不这么做——见 single_pointer_forward
        self.forwarding = self.single_pointer_forward();
        if self.forwarding {
            // 按下点判定等 X/Y 到齐，首个坐标可能已在缓冲里，补发掉
            self.flush_pending();
        }
    }

    /// 当前配置的命中容差（毫米阈值 + 两轴换算）。
    fn tap_tol(&self) -> crate::layout::Tol {
        crate::layout::Tol {
            mm: self.cfg.tap_threshold_mm,
            ux: self.tol_ux,
            uy: self.tol_uy,
        }
    }

    /// 按下点命中判定并按需发键。
    fn emit_at_down_point(&mut self) -> bool {
        let key = self.geometry.key_at_with(
            &self.table,
            self.x0,
            self.y0,
            self.cfg.key_mode,
            self.tap_tol(),
        );
        let Some(key) = key else { return false };
        if self.mixed_active() {
            // GESTURE_01：混合模式下先挂起 80 ms，确认不是滑动才真发键
            self.arm_key(key);
            return true;
        }
        self.mode = Mode::Key;
        self.pressed_key = Some(key);
        self.emit_key(key);
        true
    }

    /// 按下即发键（官方 ContSendKey 语义的前提）。
    ///
    /// GESTURE_03/04 的 350 ms 官方用于首/次 tap 判定，**不**用于吞掉快速重复按键，
    /// 否则 350 ms 内连按同一数字会丢键。此处仅记录时间戳供 `--dump` 观察。
    fn emit_key(&mut self, code: u16) {
        let now = self.now();
        self.last_tap.insert(code, now);
        self.press(code);
        self.repeat_next =
            now + self.cfg.cont_sendkey_trigger_frames as f64 * self.cfg.frame_ms as f64;
    }

    fn end_contact(&mut self) {
        // GESTURE_01：确认窗口内抬手就是一次 tap —— 必须出键。
        // 否则 80 ms 内的快速轻点会被吞掉，手感直接崩（这是最容易漏的一环）。
        if self.mode == Mode::Armed {
            if let Some(key) = self.armed_key.take() {
                self.pending_events.clear(); // 判键：暂存事件丢弃，不交给桌面
                self.click(key);
            }
        }
        if let Some(k) = self.pressed_key.take() {
            self.release(k);
        }
        // GESTURE_03/04 = 350 ms：手势结束时刻，供 tap_window_open 判定二次 tap
        if self.mode.is_gesture_family() {
            self.gesture_end_ms = self.now();
            self.gesture_end_xy = Some((self.cur_x, self.cur_y));
        }
        self.armed_key = None;
        self.pending_events.clear();
        self.forwarding = false;
        let primary = self.primary.unwrap_or(self.slot);
        self.active_slots.remove(&primary);
        self.primary = None;
        self.reset_contact();
        self.touch_activity();
    }

    fn moved(&mut self) {
        if !self.down {
            return;
        }
        let dist = self.dist_um(self.x0, self.y0, self.cur_x, self.cur_y);
        self.moved_units = dist;

        match self.mode {
            Mode::Armed => {
                // GESTURE_05 = 4838 µm：超阈值即判手势，放弃这次按键改走光标
                if dist >= self.cfg.gesture_trig_um {
                    self.promote_gesture();
                }
            }
            Mode::Scroll | Mode::Pinch | Mode::AppGesture => {
                // 滚动/缩放/应用手势整体交给桌面输入栈，驱动只做拦截不做合成
            }
            Mode::Numlock => {
                // 官方 NUMPAD_09/10：长按期间漂移 > 2741 µm 计一次失败，累计 3 次判废；
                // 官方 NUMPAD_08 Ratio=3：单次漂移超过 2741×3 µm 属明确滑动，立即判废
                if dist > self.cfg.numlock_max_valid_dist_um * NUMLOCK_MAX_FAIL_DELTA_RATIO {
                    self.mode = Mode::None;
                    return;
                }
                if dist > self.cfg.numlock_max_valid_dist_um {
                    self.fail_count += 1;
                    if self.fail_count >= self.cfg.numlock_max_fail_num {
                        self.mode = Mode::None;
                    }
                } else {
                    self.fail_count = 0;
                }
            }
            Mode::Numadv => {
                if dist > self.cfg.gesture_trig_um {
                    self.mode = Mode::None;
                }
            }
            Mode::Key
                // 官方 GESTURE_05：移动 ≥ 4838µm 由点击转手势，取消按键
                if dist >= self.cfg.gesture_trig_um => {
                    if let Some(k) = self.pressed_key.take() {
                        self.release(k);
                    }
                    // 必须走 promote_gesture：它同时把 forwarding 置成当前模式下的
                    // 正确值并补发起步位移。过去这里只改 mode，混合模式下滑动取消
                    // 按键后光标不动，起步那段事件也被丢掉。
                    self.promote_gesture();
                }
            _ => {}
        }
    }

    /// 周期性推进（与事件无关的时间相关状态）。
    pub fn tick(&mut self) {
        let now = self.now();

        // 两段式点灯的第二段：到期后补发亮度档。
        // 放在最前面，保证点灯只比按下晚一帧，而不是被 sleep 拖住整个事件循环。
        if let Some(level) = self.pending_level {
            if now >= self.pending_level_at {
                self.pending_level = None;
                self.led.send(level);
            }
        }

        // grab 运行期重试：启动那一下没抓住（被别的进程占着 / 权限刚放开）时，
        // 事件会同时打到桌面——按数字键既出数字又动光标。只在点亮期间重试，
        // 熄灭时事件本来就该让给桌面。
        if crate::lifecycle::should_retry_grab(
            self.cfg.grab,
            self.grabbed,
            self.lit,
            now - self.grab_retry_at,
        ) {
            self.grab_retry_at = now;
            self.grab();
        }

        // 跟随键盘背光：只读 sysfs，按 brightness_poll_ms 节流。
        // 数字区未点亮时不写 —— 否则会把已经熄灭的灯又点起来。
        if self.lit {
            if let Some(level) = self
                .brightness
                .poll(now, self.cfg.brightness_poll_ms as f64)
            {
                let kbd = self
                    .brightness
                    .last_kbd
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".to_string());
                eprintln!(
                    "{}",
                    crate::i18n::tf(
                        "engine.bright.line",
                        &[("kbd", &kbd), ("level", &format!("{level:02x}"))]
                    )
                );
                // 键盘背光变化优先：作废尚未下发的档位，避免旧值覆盖新值
                self.pending_level = None;
                self.led.send(level);
            }
        }

        // 官方 NUMPAD_02：无操作 120s 只熄灭背光，不退出数字键盘；
        // 之后再次触摸（NumLock 仍 on）会自动复亮
        if self.enabled
            && !self.idle_blanked
            && (now - self.last_activity_ms) >= self.cfg.max_work_time_ms as f64
        {
            self.idle_blank();
            return;
        }

        if !self.down {
            return;
        }

        // GESTURE_01/02：延迟期满且期间未被判为手势 → 确认这是一次按键
        if self.mode == Mode::Armed {
            if let Some(key) = self.armed_key {
                if self.frames_since(self.armed_since) >= self.armed_delay_frames as f64 {
                    self.armed_key = None;
                    self.pending_events.clear(); // 判键：暂存事件丢弃，不交给桌面
                    self.mode = Mode::Key;
                    self.pressed_key = Some(key);
                    self.emit_key(key);
                }
            }
            return;
        }

        // NumLock 长按（官方 62 帧 = 496 ms）
        if self.mode == Mode::Numlock && !self.fired {
            if self.frames_since(self.t0) >= self.numlock_frames() as f64
                && self.geometry.in_numlock(self.cur_x, self.cur_y)
            {
                self.fired = true;
                self.mode = Mode::Fired;
                self.toggle();
            }
            return;
        }

        // 计算器长按（官方 NUMADV_Trigger_FC = 40 帧）
        if self.mode == Mode::Numadv && !self.fired {
            if self.frames_since(self.t0) >= self.cfg.numadv_trigger_frames as f64 {
                self.fired = true;
                self.mode = Mode::Fired;
                self.fire_calc();
            }
            return;
        }

        // 连续发键（官方 NUMPAD_00 = 25 帧后开始）
        if self.mode == Mode::Key {
            if let Some(key) = self.pressed_key {
                if now >= self.repeat_next {
                    // 键已处于按下态，需先抬起再按下才能产生新的按键事件
                    self.release(key);
                    self.press(key);
                    self.repeat_next = now + self.cfg.repeat_interval_ms;
                }
            }
        }
    }

    /// 内核事件缓冲溢出（`SYN_DROPPED`）后的状态自愈。
    ///
    /// evdev 在缓冲区溢出时会插入 `SYN_DROPPED`，表示「前面丢了事件，当前状态不可信」。
    /// 丢掉的很可能是抬起事件：那会让内部一直以为手指还按着——轻则某个虚拟键卡在
    /// 按下态（用户眼里就是键盘失灵），重则混合模式下的光标一直「拖」着走。
    /// 这里把按键、触点、待发队列全部当作已失效清干净，宁可少认一次输入，
    /// 也不能留下按下去的键。
    pub fn handle_syn_dropped(&mut self) {
        if let Some(k) = self.pressed_key.take() {
            self.release(k);
        }
        // 混合模式：先给虚拟触摸板补一组「全部抬起」，否则桌面会留住按下的手指
        if self.forwarding {
            let slots: Vec<i32> = self.active_slots.iter().copied().collect();
            if let Some(p) = self.pointer.as_mut() {
                for ev in recover::release_all(&slots) {
                    p.feed(&ev);
                }
            }
        }
        self.active_slots.clear();
        self.slot_pos.clear();
        self.slot = 0;
        self.primary = None;
        self.pending_events.clear();
        self.pending_level = None;
        // 双击窗口跨过一段丢事件就不再可信，别把两次无关的点击配成双击
        self.last_tap.clear();
        self.reset_contact();
        self.log(crate::i18n::t("engine.state.syn_dropped"));
    }

    /// 统一入口：先维护多指集合，再跑官方状态机，最后决定事件去向。
    pub fn handle_event(&mut self, ev: &Event) {
        // 溢出标记必须最先处理：它前面的状态已经被内核宣告不可信
        if ev.etype == kc::EV_SYN && ev.code == kc::SYN_DROPPED {
            self.handle_syn_dropped();
            return;
        }
        self.track_slots(ev);
        self.handle_event_inner(ev);
        self.route(ev);
    }

    /// 维护在按触点集合（GESTURE_08 滚动 / NUMPAD_07 应用手势的判定基础）。
    fn track_slots(&mut self, ev: &Event) {
        if ev.etype != kc::EV_ABS {
            return;
        }
        match ev.code {
            kc::ABS_MT_SLOT => self.slot = ev.value,
            kc::ABS_MT_TRACKING_ID => {
                if ev.value >= 0 {
                    self.active_slots.insert(self.slot);
                    if self.active_slots.len() >= 2 {
                        self.enter_multi();
                    }
                } else {
                    self.active_slots.remove(&self.slot);
                    self.slot_pos.remove(&self.slot);
                }
            }
            kc::ABS_MT_POSITION_X => {
                let y = self.slot_pos.get(&self.slot).and_then(|p| p.1);
                self.slot_pos.insert(self.slot, (Some(ev.value), y));
                self.update_scroll_spread();
            }
            kc::ABS_MT_POSITION_Y => {
                let x = self.slot_pos.get(&self.slot).and_then(|p| p.0);
                self.slot_pos.insert(self.slot, (x, Some(ev.value)));
                self.update_scroll_spread();
            }
            _ => {}
        }
    }

    /// GESTURE_08 = 40 mm：双指指距超过这个值就不是滚动了，是张开放大。
    fn update_scroll_spread(&mut self) {
        if self.mode != Mode::Scroll {
            return;
        }
        let Some(dist_um) = self.slot_distance_um() else {
            return;
        };
        self.scroll_spread_mm = dist_um / 1000.0;
        if self.scroll_spread_mm > self.cfg.scroll_finger_dist_mm {
            // 张开超阈值：改记 pinch，避免后续再被当作滚动中的误触
            self.mode = Mode::Pinch;
        }
    }

    /// 第二指落下：数字键必须让位。
    ///
    /// GESTURE_08 = 40 mm 是滚动允许的最大指距，超了按「张开双指」交给桌面（缩放），
    /// 不当作滚动；三指以上按 NUMPAD_07 = 7000 µm 判应用手势。
    fn enter_multi(&mut self) {
        let n = self.active_slots.len();
        if matches!(self.mode, Mode::Armed | Mode::Key) {
            if let Some(k) = self.pressed_key.take() {
                self.release(k);
            }
            self.armed_key = None;
            self.mode = if n == 2 {
                Mode::Scroll
            } else {
                Mode::AppGesture
            };
            self.forwarding = self.mixed_active();
            if self.forwarding {
                // 让位给桌面时，起步那段得补发，否则光标会从第二个手指开始跳
                self.flush_pending();
            } else {
                self.pending_events.clear();
            }
            return;
        }
        if n >= 3 && matches!(self.mode, Mode::None | Mode::Gesture | Mode::Scroll) {
            // 三指：整段交给桌面，是否够 NUMPAD_07 阈值由 moved 记录
            self.mode = Mode::AppGesture;
            self.forwarding = self.mixed_active();
        }
    }

    /// 决定这个事件交给桌面还是喂给数字键状态机。
    fn route(&mut self, ev: &Event) {
        if self.pointer.is_none() {
            return;
        }
        if self.forwarding {
            if let Some(p) = self.pointer.as_mut() {
                p.feed(ev);
            }
            return;
        }
        if self.mode == Mode::None && self.enabled && self.lit {
            // 数字键盘点亮期间格区外的落点：不出键，也不排队等补发，直接丢
            return;
        }
        if matches!(self.mode, Mode::None | Mode::Armed) {
            // 未定：先存着，判键则丢、判手势则补发。
            // ABS_MT_SLOT 也一起存——它总在 TRACKING_ID 之前到达，按原序补发仍是合法的
            // 触点序列；提前放行会让「判为按键」的那次轻点在桌面上留下半个空槽位。
            if self.pending_events.len() >= MAX_PENDING_EVENTS {
                // 事件风暴或迟迟不判定时，缓存不能无限长；先按手势补发，触点序列仍保序
                self.promote_gesture();
                return;
            }
            self.pending_events.push(*ev);
            return;
        }
        if ev.etype == kc::EV_ABS && ev.code == kc::ABS_MT_SLOT && self.mixed_active() {
            // 已确认出键后再来第二指：槽位切换必须立刻到桌面，否则后续触点写到错槽
            if let Some(p) = self.pointer.as_mut() {
                p.feed(ev);
            }
        }
    }

    /// 当前事件是否来自非首个触点（第二个手指的坐标不该改动首触点的判键状态）。
    fn slot_is_primary(&self) -> bool {
        matches!(self.primary, Some(p) if self.slot == p)
    }

    fn handle_event_inner(&mut self, ev: &Event) {
        match ev.etype {
            kc::EV_ABS => {
                if ev.code == kc::ABS_PRESSURE || ev.code == kc::ABS_MT_PRESSURE {
                    // 压力轴仍然不参与判键（判键只看坐标格位），但可以拿来当
                    // 「用力按 = 切换」的输入：配置里 `pressure_switch.enabled` 关着
                    // 时这里只记一个采样值，行为与以前完全一致。
                    self.pressure_now = ev.value;
                    self.pressure_tick();
                    return;
                }
                if ev.code == kc::ABS_MT_SLOT {
                    self.slot = ev.value;
                    return;
                }
                if ev.code == kc::ABS_MT_TRACKING_ID {
                    if ev.value >= 0 {
                        if self.primary.is_none() {
                            // 真机事件序为 SLOT → TRACKING_ID → POSITION_X/Y，
                            // 此刻坐标仍是上一触点的，必须等本次坐标到齐再判定按下点
                            self.primary = Some(self.slot);
                            self.pending_down = true;
                            self.got_x = false;
                            self.got_y = false;
                            // 不清 pending_events：紧跟其前的 ABS_MT_SLOT 已进缓冲，
                            // 清掉会让判手势/光标时的补发序列缺槽位，桌面看到半个触点。
                            // 上一触点的残留由 end_contact 负责清空。
                            self.pending_since = self.now();
                        }
                    } else if Some(self.slot) == self.primary {
                        self.end_contact();
                    }
                    return;
                }
                if ev.code == kc::ABS_MT_POSITION_X || ev.code == kc::ABS_X {
                    if self.has_mt_tracking && !self.slot_is_primary() {
                        return;
                    }
                    self.cur_x = ev.value;
                    self.got_x = true;
                    self.maybe_begin();
                    if self.down {
                        self.touch_activity();
                        self.moved();
                    }
                } else if ev.code == kc::ABS_MT_POSITION_Y || ev.code == kc::ABS_Y {
                    if self.has_mt_tracking && !self.slot_is_primary() {
                        return;
                    }
                    self.cur_y = ev.value;
                    self.got_y = true;
                    self.maybe_begin();
                    if self.down {
                        self.touch_activity();
                        self.moved();
                    }
                }
            }
            // 没有 MT 跟踪时，手指按下/抬起只能靠 BTN_TOOL_FINGER 或 BTN_TOUCH，
            // 两者语义一致，合并成同一个 arm
            kc::EV_KEY
                if (ev.code == kc::BTN_TOOL_FINGER || ev.code == kc::BTN_TOUCH)
                    && !self.has_mt_tracking =>
            {
                if ev.value == 1 && !self.down {
                    self.begin_contact();
                } else if ev.value == 0 && self.down {
                    self.end_contact();
                }
            }
            _ => {}
        }
    }

    fn maybe_begin(&mut self) {
        if !self.pending_down {
            return;
        }
        if !(self.got_x && self.got_y) {
            return;
        }
        self.pending_down = false;
        if !self.down {
            self.begin_contact();
        }
    }

    /// 物理键盘 LED 事件 = 系统 NumLock 真实状态变化（官方联动来源）。
    pub fn handle_kb_event(&mut self, ev: &Event) {
        if ev.etype != kc::EV_LED || ev.code != kc::LED_NUML {
            return;
        }
        self.numlock_forced = false;
        if ev.value != 0 {
            if self.enabled && !self.lit {
                // 熄灯期间触摸板已释放，NumLock 重新打开必须重新接管，
                // 否则灯亮了但点击会穿透到桌面，等于错误逻辑
                self.grab();
                self.led_on(&crate::i18n::t("engine.reason.numlock_on"));
            }
        } else if self.lit {
            self.led_off(&crate::i18n::t("engine.reason.numlock_off"));
            self.ungrab();
        }
    }

    // ------------------------------------------------------------------ //
    // 生命周期
    // ------------------------------------------------------------------ //

    pub fn is_running(&self) -> bool {
        self.running
    }

    /// 运行结束时收尾：熄灯 + 释放触摸板。
    pub fn shutdown(&mut self) {
        self.disable();
    }

    /// `--dump` 用的一行状态。
    pub fn state_line(&self) -> String {
        report::state_line(self)
    }
    /// 告诉引擎压力轴的满量程（由 Linux 层 `EVIOCGABS` 读出后调用）。
    ///
    /// `0`（或负数）表示该触摸板没有压力轴，压感切换自动失效——不需要用户
    /// 去配置里关掉它，避免「换了一台没压感的机器就忘了改配置」。
    pub fn set_pressure_max(&mut self, max: i32) {
        self.pressure_max = max.max(0);
    }

    /// 压力轴满量程（`0` = 无压感）。
    pub fn pressure_max(&self) -> i32 {
        self.pressure_max
    }

    /// 压感切换是否已启用且当前硬件支持。
    pub fn pressure_available(&self) -> bool {
        self.pressure.enabled() && self.pressure_max > 0
    }

    /// 压感切换的一行状态（GUI / `--status` 展示用）。
    pub fn pressure_report(&self) -> String {
        let cfg = self.pressure.config();
        format!(
            "enabled={} max={} now={} threshold={}% frames={} fired={}",
            self.pressure_available(),
            self.pressure_max,
            self.pressure_now,
            cfg.percent,
            cfg.frames,
            self.pressure.fired()
        )
    }

    /// 喂一次压力判定；命中则切换数字键盘开关。
    fn pressure_tick(&mut self) {
        if !self.pressure_available() {
            return;
        }
        let now = self.now();
        let contact = self.primary.is_some() || !self.active_slots.is_empty();
        let value = self.pressure_now;
        let max = self.pressure_max;
        if self.pressure.feed(value, max, contact, now) {
            self.pressure_last_ms = now;
            self.log(crate::i18n::tf(
                "engine.pressure.toggle",
                &[("value", &value.to_string()), ("max", &max.to_string())],
            ));
            self.toggle();
        }
    }

    /// 喂一次物理键盘事件；命中绑定的组合键就执行对应动作。
    ///
    /// 只旁听键盘事件（键盘设备不 grab），因此热键不会影响正常输入。
    /// 返回本次是否真的触发了动作，方便 `--verbose` 或 GUI 打点。
    pub fn handle_hotkey_event(&mut self, ev: &Event) -> bool {
        if ev.etype != kc::EV_KEY {
            return false;
        }
        let Some(action) = self.hotkeys.feed(ev.code, ev.value) else {
            return false;
        };
        self.hotkey_last = Some(action.as_str());
        self.log(crate::i18n::tf(
            "engine.hotkey.fired",
            &[("action", action.as_str()), ("key", kc::name_of(ev.code))],
        ));
        match action {
            crate::custom::HotkeyAction::Toggle => self.toggle(),
            crate::custom::HotkeyAction::On => self.enable(),
            crate::custom::HotkeyAction::Off => self.disable(),
        }
        true
    }

    /// 已装载的热键绑定（`组合键 -> 动作`），GUI 列表与 `--status` 用。
    pub fn hotkey_labels(&self) -> Vec<String> {
        self.hotkeys.describe()
    }

    /// 键状态复位：切 tty / 重新 grab 后键盘可能残留按下态。
    pub fn clear_hotkey_state(&mut self) {
        self.hotkeys.clear();
    }

    /// 配置里的布局/键表换了之后重新取一次（GUI 热切换用）。
    pub fn refresh_table(&mut self) {
        self.table = crate::custom::table_for(&self.cfg.layout_name);
    }

    /// 当前键表（GUI 展示自定义网格用）。
    pub fn key_table(&self) -> &crate::custom::KeyTable {
        &self.table
    }

    /// `--check` 用：把几何与关键判定打印出来。
    pub fn geometry_report(&self) -> Vec<String> {
        report::geometry_lines(self)
    }

    /// 供测试读取：当前 armed 延迟帧数。
    pub fn armed_delay_frames(&self) -> u32 {
        self.armed_delay_frames
    }

    pub fn key_mode(&self) -> KeyMode {
        self.cfg.key_mode
    }

    pub fn mixed_mode(&self) -> MixedMode {
        self.cfg.mixed_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::GH3XLNDZ;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeKeys {
        log: Vec<(u16, i32)>,
    }
    impl KeySink for Rc<RefCell<FakeKeys>> {
        fn press(&mut self, code: u16) {
            self.borrow_mut().log.push((code, 1));
        }
        fn release(&mut self, code: u16) {
            self.borrow_mut().log.push((code, 0));
        }
    }

    impl PointerSink for Rc<RefCell<Vec<Event>>> {
        fn feed(&mut self, ev: &Event) {
            self.borrow_mut().push(*ev);
        }
    }

    struct NumLockOn;
    impl NumLockSource for NumLockOn {
        fn numlock_on(&self) -> Option<bool> {
            Some(true)
        }
    }

    struct FakeGrab;
    impl GrabSink for FakeGrab {
        fn grab(&mut self) -> bool {
            true
        }
        fn ungrab(&mut self) {}
    }

    /// 记录每次下发的 LED 字节，用于检查点灯时序。
    struct SpyLed {
        log: Rc<RefCell<Vec<u8>>>,
    }
    impl LedSink for SpyLed {
        fn send(&mut self, v: u8) -> bool {
            self.log.borrow_mut().push(v);
            true
        }
        fn channel(&self) -> String {
            "spy".to_string()
        }
    }

    /// 可手动拨动的时钟：验证「推迟到 tick 补发」这类时序行为必须能控制时间。
    struct ManualClock {
        now: Rc<std::cell::Cell<f64>>,
    }
    impl Clock for ManualClock {
        fn now_ms(&self) -> f64 {
            self.now.get()
        }
    }

    /// 造一台「有虚拟触摸板 + 小键盘模组」的引擎，时钟、LED 与配置调用方可控。
    /// 不调用 `enable()`，便于从「未点亮」状态验证点灯时序。
    fn engine_raw(
        cfg: Config,
        now: Rc<std::cell::Cell<f64>>,
        led_log: Rc<RefCell<Vec<u8>>>,
    ) -> (Engine, Rc<RefCell<Vec<Event>>>) {
        engine_raw_numlock(cfg, now, led_log, Some(Box::new(NumLockOn))).0
    }

    /// 同上，但 NumLock 源由调用方给：None 表示「读不到 NumLock 状态」。
    #[allow(clippy::type_complexity)]
    fn engine_raw_numlock(
        cfg: Config,
        now: Rc<std::cell::Cell<f64>>,
        led_log: Rc<RefCell<Vec<u8>>>,
        numlock: Option<Box<dyn NumLockSource>>,
    ) -> ((Engine, Rc<RefCell<Vec<Event>>>), Rc<RefCell<FakeKeys>>) {
        let geom = Geometry::new(0, 3000, 0, 2000, 10.0, 10.0, GH3XLNDZ);
        let pointer: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let keys: Rc<RefCell<FakeKeys>> = Rc::new(RefCell::new(FakeKeys::default()));
        let deps = Deps {
            led: Box::new(SpyLed { log: led_log }),
            keys: Box::new(keys.clone()),
            pointer: Some(Box::new(pointer.clone())),
            numlock,
            grabber: Some(Box::new(FakeGrab)),
            clock: Box::new(ManualClock { now }),
        };
        ((Engine::new(cfg, geom, deps, true), pointer), keys)
    }

    /// NumLock 状态可随时翻转的源（`None` = 这台机器读不到）。
    struct SwitchableNumLock(Rc<std::cell::Cell<Option<bool>>>);
    impl NumLockSource for SwitchableNumLock {
        fn numlock_on(&self) -> Option<bool> {
            self.0.get()
        }
    }

    /// 造一台已进入小键盘态的引擎（时钟与 LED 调用方可控）。
    fn engine_with(
        mixed: MixedMode,
        now: Rc<std::cell::Cell<f64>>,
        led_log: Rc<RefCell<Vec<u8>>>,
    ) -> (Engine, Rc<RefCell<Vec<Event>>>) {
        let cfg = Config {
            mixed_mode: mixed,
            ..Config::default()
        };
        let (mut e, pointer) = engine_raw(cfg, now, led_log);
        e.enable();
        assert!(e.lit, "小键盘态应已点亮");
        (e, pointer)
    }

    /// 造一台「有虚拟触摸板 + 小键盘模组」的引擎。
    fn engine(mixed: MixedMode) -> (Engine, Rc<RefCell<Vec<Event>>>) {
        let now = Rc::new(std::cell::Cell::new(0.0));
        engine_with(mixed, now, Rc::new(RefCell::new(Vec::new())))
    }

    // ---------------- grab 运行期重试 ----------------

    /// 前 N 次 grab 失败，之后成功——模拟「启动时被别的进程占着」。
    struct FlakyGrab {
        fails: Rc<std::cell::Cell<usize>>,
        calls: Rc<std::cell::Cell<usize>>,
    }
    impl GrabSink for FlakyGrab {
        fn grab(&mut self) -> bool {
            self.calls.set(self.calls.get() + 1);
            if self.fails.get() > 0 {
                self.fails.set(self.fails.get() - 1);
                false
            } else {
                true
            }
        }
        fn ungrab(&mut self) {}
    }

    #[test]
    fn grab_retries_while_lit_and_succeeds_later() {
        // 启动时 grab 失败（别的进程占着触摸板）：不能一直等到用户重启服务，
        // 否则数字键会同时打到桌面——按下出数字又移动光标。
        let now = Rc::new(std::cell::Cell::new(0.0));
        let led: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let fails = Rc::new(std::cell::Cell::new(2));
        let calls = Rc::new(std::cell::Cell::new(0));
        let geom = Geometry::new(0, 3000, 0, 2000, 10.0, 10.0, GH3XLNDZ);
        let deps = Deps {
            led: Box::new(SpyLed { log: led }),
            keys: Box::new(Rc::new(RefCell::new(FakeKeys::default()))),
            pointer: Some(Box::new(Rc::new(RefCell::new(Vec::new())))),
            numlock: Some(Box::new(NumLockOn)),
            grabber: Some(Box::new(FlakyGrab {
                fails: fails.clone(),
                calls: calls.clone(),
            })),
            clock: Box::new(ManualClock { now: now.clone() }),
        };
        let mut e = Engine::new(Config::default(), geom, deps, true);
        e.enable();
        assert!(e.lit);
        assert!(!e.grabbed, "启动时抓不住");
        let after_enable = calls.get();

        // 未到重试间隔：不该反复砸同一个失败调用
        now.set(now.get() + 100.0);
        e.tick();
        assert_eq!(calls.get(), after_enable, "间隔没到不该重试");

        // 到点了：重试一次，仍然失败
        now.set(now.get() + crate::lifecycle::GRAB_RETRY_MS);
        e.tick();
        assert_eq!(calls.get(), after_enable + 1);
        assert!(!e.grabbed);

        // 再过一轮：这次成功
        now.set(now.get() + crate::lifecycle::GRAB_RETRY_MS);
        e.tick();
        assert!(e.grabbed, "占用释放后应当抓住");

        // 抓住之后不再无谓重试
        let settled = calls.get();
        now.set(now.get() + crate::lifecycle::GRAB_RETRY_MS * 3.0);
        e.tick();
        assert_eq!(calls.get(), settled);
    }

    #[test]
    fn grab_retry_waits_for_dark_state() {
        // 熄灯期间事件本就该给桌面，不该去抢
        let now = Rc::new(std::cell::Cell::new(0.0));
        let led: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let calls = Rc::new(std::cell::Cell::new(0));
        let geom = Geometry::new(0, 3000, 0, 2000, 10.0, 10.0, GH3XLNDZ);
        let deps = Deps {
            led: Box::new(SpyLed { log: led }),
            keys: Box::new(Rc::new(RefCell::new(FakeKeys::default()))),
            pointer: Some(Box::new(Rc::new(RefCell::new(Vec::new())))),
            numlock: Some(Box::new(NumLockOn)),
            grabber: Some(Box::new(FlakyGrab {
                fails: Rc::new(std::cell::Cell::new(usize::MAX)),
                calls: calls.clone(),
            })),
            clock: Box::new(ManualClock { now: now.clone() }),
        };
        let mut e = Engine::new(Config::default(), geom, deps, true);
        e.enable();
        e.disable();
        assert!(!e.lit);
        let before = calls.get();
        now.set(now.get() + crate::lifecycle::GRAB_RETRY_MS * 2.0);
        e.tick();
        assert_eq!(calls.get(), before, "熄灭状态不该去抢设备");
    }

    // ---------------- 事件丢失自愈：SYN_DROPPED ----------------

    fn syn_dropped() -> Event {
        Event {
            etype: kc::EV_SYN,
            code: kc::SYN_DROPPED,
            value: 0,
        }
    }

    #[test]
    fn syn_dropped_releases_stuck_key() {
        // 抬起事件被内核丢掉 = 用户眼里「键盘卡住了」。自愈必须补一次抬起。
        let now = Rc::new(std::cell::Cell::new(0.0));
        let led: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let ((mut e, _p), keys) =
            engine_raw_numlock(Config::default(), now, led, Some(Box::new(NumLockOn)));
        e.enable();
        // 手动摆成「正按着 KP1」的现场：按下事件已发，抬起丢了
        e.down = true;
        e.mode = Mode::Key;
        e.pressed_key = Some(kc::KEY_KP1);
        e.active_slots.insert(0);
        e.press(kc::KEY_KP1);
        let before = keys.borrow().log.len();

        e.handle_event(&syn_dropped());

        let released = keys.borrow().log[before..].to_vec();
        assert_eq!(
            released,
            vec![(kc::KEY_KP1, 0)],
            "丢事件后必须把按下的键抬起来"
        );
        assert!(!e.down, "触点状态应复位");
        assert!(e.pressed_key.is_none());
        assert_eq!(e.mode, Mode::None);
        assert!(e.active_slots.is_empty(), "触点集合要清干净");
    }

    #[test]
    fn syn_dropped_flushes_forwarded_contacts() {
        // 混合模式正在把触点转给虚拟触摸板时丢事件：不补抬起，光标会一直「拖着」走
        let now = Rc::new(std::cell::Cell::new(0.0));
        let led: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let cfg = Config {
            mixed_mode: MixedMode::Auto,
            ..Config::default()
        };
        let ((mut e, pointer), _keys) =
            engine_raw_numlock(cfg, now, led, Some(Box::new(NumLockOn)));
        e.enable();
        e.forwarding = true;
        e.active_slots.insert(2);
        let before = pointer.borrow().len();

        e.handle_event(&syn_dropped());

        let tail: Vec<(u16, i32)> = pointer.borrow()[before..]
            .iter()
            .map(|ev| (ev.code, ev.value))
            .collect();
        assert_eq!(
            tail,
            vec![(kc::ABS_MT_SLOT, 2), (kc::ABS_MT_TRACKING_ID, -1), (0, 0),],
            "要给虚拟触摸板补 SLOT/TRACKING_ID=-1/SYN 三件套"
        );
        assert!(!e.forwarding, "转发态应结束");
    }

    #[test]
    fn syn_dropped_without_state_is_harmless() {
        // 空闲时（没有触点、没有按键）收到溢出标记不应发任何东西
        let now = Rc::new(std::cell::Cell::new(0.0));
        let led: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let ((mut e, pointer), keys) =
            engine_raw_numlock(Config::default(), now, led, Some(Box::new(NumLockOn)));
        e.enable();
        let kb = keys.borrow().log.len();
        let pt = pointer.borrow().len();
        e.handle_event(&syn_dropped());
        assert_eq!(keys.borrow().log.len(), kb);
        assert_eq!(pointer.borrow().len(), pt);
        assert!(!e.down);
    }

    // ---------------- 挂起恢复：resync_after_resume ----------------

    #[test]
    fn resume_while_lit_replays_full_lighting() {
        // 合盖唤醒后 EC 已把背光清零，引擎内部却还认为亮着：
        // resync 必须重新走一遍点亮流程，否则数字区再也亮不起来
        let now = Rc::new(std::cell::Cell::new(0.0));
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let (mut e, _p) = engine_raw(Config::default(), now, log.clone());
        e.enable();
        assert!(e.lit);
        let before = log.borrow().len();
        e.resync_after_resume();
        assert!(e.lit, "重发后仍应是点亮状态");
        assert!(log.borrow().len() > before, "挂起恢复必须重新下发点亮命令");
        // 幂等：再挂起一次仍然要再补一次灯（每次唤醒后 EC 都可能重新清零）
        let before2 = log.borrow().len();
        e.resync_after_resume();
        assert!(log.borrow().len() > before2, "每次唤醒都应补灯");
        assert!(e.lit);
    }

    #[test]
    fn resume_while_dark_stays_dark() {
        let now = Rc::new(std::cell::Cell::new(0.0));
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let (mut e, _p) = engine_raw(Config::default(), now, log.clone());
        e.enable();
        e.disable();
        assert!(!e.lit);
        let before = log.borrow().len();
        e.resync_after_resume();
        assert!(!e.lit);
        assert_eq!(
            log.borrow().len(),
            before,
            "原本就暗着，唤醒后不该自己亮起来"
        );
    }

    #[test]
    fn resume_respects_numlock_off() {
        // 挂起恢复时 NumLock 是关的：不能把灯硬点起来（那会把用户的开关键状态也改掉）
        let now = Rc::new(std::cell::Cell::new(0.0));
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let state = Rc::new(std::cell::Cell::new(Some(true)));
        let ((mut e, _p), _k) = engine_raw_numlock(
            Config::default(),
            now,
            log.clone(),
            Some(Box::new(SwitchableNumLock(state.clone()))),
        );
        e.enable();
        assert!(e.lit);
        state.set(Some(false));
        e.resync_after_resume();
        // 内部标记要与硬件一致：否则下一次唤醒会误以为「还亮着」而不再补灯
        assert!(!e.lit, "NumLock 已关，唤醒后不应点灯");
    }

    /// 用力按触摸板 = 切换小键盘：从真实事件入口喂压力，而不是直接调压力闸。
    #[test]
    fn hard_press_toggles_through_event_path() {
        let now = Rc::new(std::cell::Cell::new(0.0));
        let cfg = Config {
            pressure_switch: crate::custom::PressureSwitch {
                enabled: true,
                percent: 50,
                frames: 2,
                cooldown_ms: 600.0,
            },
            ..Config::default()
        };
        let (mut e, _p) = engine_raw(cfg, now.clone(), Rc::new(RefCell::new(Vec::new())));
        e.set_pressure_max(255);
        e.enable();
        assert!(e.lit);
        // 有触点才认压力：没有触点时压力再大也不算「按」
        e.handle_event(&Event::abs(kc::ABS_PRESSURE, 250));
        assert!(e.lit, "没有触点时不该触发");

        e.primary = Some(0);
        e.handle_event(&Event::abs(kc::ABS_PRESSURE, 250));
        e.handle_event(&Event::abs(kc::ABS_PRESSURE, 250));
        assert!(!e.lit, "按住两帧后应切到触控板态");
        assert_eq!(e.pressure.fired(), 1);

        // 一直按着不放：冷却内不许连续翻转
        now.set(400.0);
        for _ in 0..4 {
            e.handle_event(&Event::abs(kc::ABS_PRESSURE, 250));
        }
        assert!(!e.lit, "冷却内不该翻转");
        // 抬起触点再按一次可以重新触发
        e.primary = None;
        e.handle_event(&Event::abs(kc::ABS_PRESSURE, 0));
        now.set(700.0);
        e.primary = Some(0);
        e.handle_event(&Event::abs(kc::ABS_PRESSURE, 250));
        e.handle_event(&Event::abs(kc::ABS_PRESSURE, 250));
        assert!(e.lit, "冷却结束后再按应切回小键盘态");
        assert_eq!(e.pressure.fired(), 2);
    }

    /// 压感关掉时，同样的事件序列必须完全没反应（默认配置不能有副作用）。
    #[test]
    fn pressure_switch_off_ignores_hard_press() {
        let now = Rc::new(std::cell::Cell::new(0.0));
        let (mut e, _p) = engine_raw(Config::default(), now, Rc::new(RefCell::new(Vec::new())));
        e.set_pressure_max(255);
        e.enable();
        e.primary = Some(0);
        for _ in 0..10 {
            e.handle_event(&Event::abs(kc::ABS_PRESSURE, 255));
        }
        assert!(e.lit, "压感关着就不该被压力事件切换");
        assert_eq!(e.pressure.fired(), 0);
    }

    /// 全局热键：ctrl+alt+n 切换，键盘事件从 handle_hotkey_event 进来。
    #[test]
    fn hotkey_ctrl_alt_n_toggles_through_event_path() {
        let now = Rc::new(std::cell::Cell::new(0.0));
        let cfg = Config {
            hotkeys: vec![crate::custom::HotkeySpec {
                gesture: "ctrl+alt+n".to_string(),
                action: "toggle".to_string(),
            }],
            ..Config::default()
        };
        let (mut e, _p) = engine_raw(cfg, now, Rc::new(RefCell::new(Vec::new())));
        e.enable();
        assert!(e.lit);

        // 没按修饰键时单独按 n 不触发
        assert!(!e.handle_hotkey_event(&Event::key(kc::KEY_N, 1)));
        assert!(e.lit);

        // ctrl + alt + n
        assert!(!e.handle_hotkey_event(&Event::key(kc::KEY_LEFTCTRL, 1)));
        assert!(!e.handle_hotkey_event(&Event::key(kc::KEY_LEFTALT, 1)));
        assert!(e.handle_hotkey_event(&Event::key(kc::KEY_N, 1)));
        assert!(!e.lit, "组合键应切到触控板态");

        // 按住产生的重复事件（value=2）不该重复触发
        assert!(!e.handle_hotkey_event(&Event::key(kc::KEY_N, 2)));
        assert!(!e.lit);
        // 松开再按一次切回去
        e.handle_hotkey_event(&Event::key(kc::KEY_N, 0));
        assert!(e.handle_hotkey_event(&Event::key(kc::KEY_N, 1)));
        assert!(e.lit);
        assert_eq!(e.hotkey_last, Some("toggle"));
    }

    /// `action = "on"` 的热键只开不翻：已经是开的时候再按也不能关掉。
    #[test]
    fn hotkey_action_on_is_idempotent() {
        let now = Rc::new(std::cell::Cell::new(0.0));
        let cfg = Config {
            hotkeys: vec![crate::custom::HotkeySpec {
                gesture: "ctrl+shift+o".to_string(),
                action: "on".to_string(),
            }],
            ..Config::default()
        };
        let (mut e, _p) = engine_raw(cfg, now, Rc::new(RefCell::new(Vec::new())));
        e.enable();
        for kind in [1, 0, 1] {
            e.handle_hotkey_event(&Event::key(kc::KEY_LEFTCTRL, kind));
            e.handle_hotkey_event(&Event::key(kc::KEY_LEFTSHIFT, kind));
            let fired = e.handle_hotkey_event(&Event::key(kc::KEY_O, kind));
            if kind == 1 {
                assert!(fired, "按下应触发（第 {kind} 次）");
                assert!(e.lit, "on 只能开，不能关");
            }
        }
    }

    /// 数字键盘点亮时，从触摸板边缘起步的单指滑动不该把光标拖走。
    #[test]
    fn edge_slide_does_not_move_pointer_in_digit_mode() {
        let (mut e, pointer) = engine(MixedMode::Auto);
        assert!(e.mixed_active(), "混合模式本身仍然生效");

        e.cur_x = e.geometry.xmin + 1;
        e.cur_y = e.geometry.ymin + 1;
        e.begin_contact();
        assert!(!e.forwarding, "点亮态下键区外落点不交给桌面");

        // 沿边缘滑动并抬手
        e.cur_x = e.geometry.xmin + 400;
        e.cur_y = e.geometry.ymin + 40;
        e.route(&Event::abs(kc::ABS_MT_POSITION_X, e.cur_x));
        e.cur_x = e.geometry.xmin + 900;
        e.route(&Event::abs(kc::ABS_MT_POSITION_Y, e.cur_y));
        e.end_contact();

        assert!(pointer.borrow().is_empty(), "光标不该收到任何触点事件");
    }

    /// `mixed_mode = on` 保留旧行为：键区外一律透传，用户可自行选回。
    #[test]
    fn edge_slide_forwarded_when_mixed_mode_forced_on() {
        let (mut e, pointer) = engine(MixedMode::On);
        e.cur_x = e.geometry.xmin + 1;
        e.cur_y = e.geometry.ymin + 1;
        e.begin_contact();
        assert!(e.forwarding, "强制混合模式仍把键区外触点还给桌面");
        e.route(&Event::abs(kc::ABS_MT_POSITION_X, e.geometry.xmin + 300));
        assert!(!pointer.borrow().is_empty());
    }

    /// 纯数字键盘（mixed_mode = off）本来就没有虚拟触摸板，别回归。
    #[test]
    fn no_pointer_when_mixed_mode_off() {
        let (e, pointer) = engine(MixedMode::Off);
        assert!(!e.mixed_active());
        assert!(pointer.borrow().is_empty());
    }

    // ---------------- NumLock 三模式（自动联动的核心） ----------------

    /// 造一台 NumLock 状态可随时翻转的引擎。
    #[allow(clippy::type_complexity)]
    fn engine_numlock(
        mode: NumLockMode,
        state: Option<bool>,
    ) -> (
        Engine,
        Rc<RefCell<Vec<u8>>>,
        Rc<RefCell<FakeKeys>>,
        Rc<std::cell::Cell<Option<bool>>>,
        Rc<std::cell::Cell<f64>>,
    ) {
        let cfg = Config {
            numlock: mode,
            ..Config::default()
        };
        let now = Rc::new(std::cell::Cell::new(1000.0));
        let led_log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let cell = Rc::new(std::cell::Cell::new(state));
        let ((eng, _pointer), keys) = engine_raw_numlock(
            cfg,
            now.clone(),
            led_log.clone(),
            Some(Box::new(SwitchableNumLock(cell.clone()))),
        );
        (eng, led_log, keys, cell, now)
    }

    /// 日志里有没有发过 NumLock 按键（按 + 放）。
    fn numlock_clicks(keys: &Rc<RefCell<FakeKeys>>) -> usize {
        keys.borrow()
            .log
            .iter()
            .filter(|(code, _)| *code == kc::KEY_NUMLOCK)
            .count()
    }

    #[test]
    fn sync_mode_turns_numlock_on_when_it_is_off() {
        let (mut eng, led, keys, _cell, _now) = engine_numlock(NumLockMode::Sync, Some(false));
        eng.enable();
        // 先补按 NumLock（按下 + 抬起），再点灯
        assert_eq!(numlock_clicks(&keys), 2);
        assert!(!led.borrow().is_empty(), "补按后应当点灯");
        // 退出时把 NumLock 恢复成关：又按一次
        eng.disable();
        assert_eq!(numlock_clicks(&keys), 4);
    }

    #[test]
    fn sync_mode_leaves_numlock_on_untouched() {
        let (mut eng, led, keys, _cell, _now) = engine_numlock(NumLockMode::Sync, Some(true));
        eng.enable();
        assert_eq!(numlock_clicks(&keys), 0, "NumLock 已开就不该再按");
        assert!(!led.borrow().is_empty());
        eng.disable();
        assert_eq!(numlock_clicks(&keys), 0, "本来就没按过，退出不应还原");
    }

    #[test]
    fn toggle_mode_always_clicks_and_restores() {
        // 已经是 on 也要切（这才是 Toggle 的语义），退出时切回去
        let (mut eng, _led, keys, _cell, _now) = engine_numlock(NumLockMode::Toggle, Some(true));
        eng.enable();
        assert_eq!(numlock_clicks(&keys), 2);
        eng.disable();
        assert_eq!(numlock_clicks(&keys), 4);
    }

    #[test]
    fn off_mode_keeps_light_off_when_numlock_is_off() {
        // 不动 NumLock，也不该在 NumLock 关着时点灯（否则打出来的不是数字）
        let (mut eng, led, keys, _cell, _now) = engine_numlock(NumLockMode::Off, Some(false));
        eng.enable();
        assert_eq!(numlock_clicks(&keys), 0);
        assert!(led.borrow().is_empty(), "NumLock 关着时不应点灯");
    }

    #[test]
    fn unknown_numlock_state_still_lights() {
        // 读不到 NumLock 时不能全禁：很多机型没有键盘灯节点，
        // 一律按关处理会让整个数字区永远不亮
        let (mut eng, led, keys, _cell, _now) = engine_numlock(NumLockMode::Off, None);
        eng.enable();
        assert_eq!(numlock_clicks(&keys), 0);
        assert!(!led.borrow().is_empty(), "读不到状态时应当照常点灯");
    }

    #[test]
    fn numlock_flipping_off_blocks_wake_up() {
        // 熄灯后（空闲只熄灯、不退出）能否复亮取决于 NumLock：
        // 关着就保持熄灭，否则用户以为在打数字、实际在移动光标。
        let (mut eng, led, _keys, cell, now) = engine_numlock(NumLockMode::Sync, Some(true));
        eng.enable();
        let after_enable = led.borrow().len();
        assert!(after_enable > 0);

        // 推进过空闲阈值：tick 只熄灯，仍处于「已启用」
        now.set(now.get() + eng.cfg.max_work_time_ms as f64 + 10.0);
        eng.tick();
        assert_eq!(led.borrow().last().copied(), Some(0), "空闲后应下发熄灭");
        let before_touch = led.borrow().len();

        // 动作区外点一下：真实事件序是 TRACKING_ID → POSITION_X/Y，缺坐标不会开始触点
        let tap = |e: &mut Engine, x: i32, y: i32, id: i32| {
            e.handle_event(&Event::abs(kc::ABS_MT_TRACKING_ID, id));
            e.handle_event(&Event::abs(kc::ABS_MT_POSITION_X, x));
            e.handle_event(&Event::abs(kc::ABS_MT_POSITION_Y, y));
            e.handle_event(&Event::abs(kc::ABS_MT_TRACKING_ID, -1));
        };

        // NumLock 被关掉 → 触摸不复亮
        cell.set(Some(false));
        tap(&mut eng, 100, 100, 3);
        assert_eq!(led.borrow().len(), before_touch, "NumLock 关着不应复亮");

        // NumLock 又开了 → 触摸复亮
        cell.set(Some(true));
        tap(&mut eng, 100, 100, 4);
        assert!(
            led.borrow().len() > before_touch,
            "NumLock 开着时触摸应自动复亮"
        );
    }

    // ---------------- LED 通道失败与解锁序列 ----------------

    /// 可选：前 `fail_times` 次 send 全部失败，之后成功。用来验证「通道不通不置位」。
    struct FlakyLed {
        log: Rc<RefCell<Vec<u8>>>,
        fail_times: Rc<std::cell::Cell<usize>>,
        unlock: bool,
    }
    impl LedSink for FlakyLed {
        fn send(&mut self, v: u8) -> bool {
            self.log.borrow_mut().push(v);
            let left = self.fail_times.get();
            if left > 0 {
                self.fail_times.set(left - 1);
                return false;
            }
            true
        }
        fn channel(&self) -> String {
            "flaky".to_string()
        }
        fn needs_unlock(&self) -> bool {
            self.unlock
        }
    }

    #[allow(clippy::type_complexity)]
    fn engine_flaky(
        cfg: Config,
        fail_times: usize,
        unlock: bool,
    ) -> (Engine, Rc<RefCell<Vec<u8>>>, Rc<std::cell::Cell<usize>>) {
        let now = Rc::new(std::cell::Cell::new(1000.0));
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let left = Rc::new(std::cell::Cell::new(fail_times));
        let geom = Geometry::new(0, 3000, 0, 2000, 10.0, 10.0, GH3XLNDZ);
        let deps = Deps {
            led: Box::new(FlakyLed {
                log: log.clone(),
                fail_times: left.clone(),
                unlock,
            }),
            keys: Box::new(Rc::new(RefCell::new(FakeKeys::default()))),
            pointer: None,
            numlock: Some(Box::new(NumLockOn)),
            grabber: Some(Box::new(FakeGrab)),
            clock: Box::new(ManualClock { now }),
        };
        (Engine::new(cfg, geom, deps, true), log, left)
    }

    #[test]
    fn failed_lighting_keeps_lit_false_and_retries_next_time() {
        // 通道第一次全失败（亮 + 档位都发不出去）：不能置 lit，
        // 否则后续唤醒/联动全部跳过重试，表现为「灯永远不亮」
        let cfg = Config {
            follow_keyboard_brightness: false,
            ..Config::default()
        };
        let (mut eng, log, left) = engine_flaky(cfg, 2, false);
        eng.enable();
        assert!(!eng.lit, "下发失败时不应标记为已点亮");
        let after_first = log.borrow().len();
        assert!(after_first >= 1);
        left.set(0);
        // 再触发一次点灯，这次成功
        eng.enable();
        assert!(eng.lit, "通道恢复后应点亮");
        assert!(log.borrow().len() > after_first);
    }

    #[test]
    fn i2c_channel_sends_unlock_before_level() {
        // i2c 直写 EC 走官方两段式：先 0x60 解锁，再下档位/亮
        let cfg = Config {
            follow_keyboard_brightness: false,
            manual_brightness_step: 4,
            ..Config::default()
        };
        let (mut eng, log, _left) = engine_flaky(cfg, 0, true);
        eng.enable();
        let sent = log.borrow().clone();
        assert_eq!(
            sent.first().copied(),
            Some(VAL_UNLOCK),
            "首个字节应为解锁字"
        );
        assert!(sent.contains(&VAL_ON), "解锁后应下发点亮");
    }

    #[test]
    fn hidraw_channel_skips_unlock() {
        let cfg = Config {
            follow_keyboard_brightness: false,
            ..Config::default()
        };
        let (mut eng, log, _left) = engine_flaky(cfg, 0, false);
        eng.enable();
        assert!(
            !log.borrow().contains(&VAL_UNLOCK),
            "hidraw FEATURE 通道不需要 0x60"
        );
    }

    #[test]
    fn off_send_failure_still_marks_dark() {
        // 熄灭无论成不成功都必须转成「未点亮」：卡在 lit=true 会让
        // 唤醒逻辑永远提前 return，用户再摸也不会亮
        let cfg = Config {
            follow_keyboard_brightness: false,
            ..Config::default()
        };
        let (mut eng, _log, left) = engine_flaky(cfg, 0, false);
        eng.enable();
        assert!(eng.lit);
        left.set(1);
        eng.disable();
        assert!(!eng.lit);
    }

    #[test]
    fn staged_level_deferred_to_tick_and_cancelled_on_off() {
        let now = Rc::new(std::cell::Cell::new(0.0));
        let log: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        // 不跟随键盘背光 + 手动第 2 档 = 0x45，点灯才会走两段式
        let cfg = Config {
            follow_keyboard_brightness: false,
            manual_brightness_levels: Some(vec![0x43, 0x45, 0x48]),
            manual_brightness_step: 2,
            ..Config::default()
        };
        let (mut e, _p) = engine_raw(cfg, now.clone(), log.clone());

        assert!(e.led_on("test"), "点亮请求本身要立即成功");
        assert_eq!(*log.borrow().last().unwrap(), 0x01, "先只发点亮命令");
        assert!(!log.borrow().contains(&0x45), "档位此刻还不该下发");

        // 间隔未满：tick 不发
        now.set(STAGED_LEVEL_GAP_MS - 1.0);
        e.tick();
        assert!(!log.borrow().contains(&0x45), "间隔未到不该补发");

        // 到期：tick 补发档位
        now.set(STAGED_LEVEL_GAP_MS);
        e.tick();
        assert_eq!(
            log.borrow().last().copied(),
            Some(0x45),
            "到期后应补发亮度档"
        );

        // 待下发期间熄灭：档位作废
        e.led_off("test");
        log.borrow_mut().clear();
        assert!(e.led_on("test"));
        e.led_off("test");
        now.set(STAGED_LEVEL_GAP_MS * 10.0);
        e.tick();
        assert!(
            !log.borrow().contains(&0x45),
            "熄灭后不该再把暂存的亮度档补发出去"
        );
    }

    /// 帧节拍换算：8 ms/帧时 1 s 应恰好 125 帧；`frame_ms` 为 0 时按 1 ms 兜底，不能算出 inf。
    #[test]
    fn frames_between_follows_frame_model() {
        let f = recover::frames_between(0.0, 1000.0, 8);
        assert!(
            (f - 125.0).abs() < 1e-9,
            "8 ms/帧下 1 s 应为 125 帧，实际 {f}"
        );
        let guard = recover::frames_between(0.0, 1000.0, 0);
        assert!(guard.is_finite() && guard > 0.0, "帧长缺失时必须兜底且有限");
    }

    /// 丢事件自愈：每个在按触点都要补 SLOT + TRACKING_ID(-1)，末尾补一个 SYN 收尾。
    #[test]
    fn release_all_covers_every_slot() {
        let evs = recover::release_all(&[0, 3]);
        assert_eq!(evs.len(), 5, "两个触点 = 4 条抬起 + 1 条 SYN");
        assert_eq!(
            evs.iter()
                .filter(|e| e.code == kc::ABS_MT_TRACKING_ID)
                .count(),
            2
        );
        assert!(
            evs.iter()
                .filter(|e| e.code == kc::ABS_MT_TRACKING_ID)
                .all(|e| e.value == -1),
            "抬起必须用 -1 表示离开"
        );
        let last = evs.last().unwrap();
        assert_eq!((last.etype, last.code), (kc::EV_SYN, 0));
    }

    /// 距离换算走 geometry 的 µm 单位：单轴运动只在该轴上放大，单位缺失的轴按 0 算。
    #[test]
    fn dist_um_uses_geometry_units() {
        let mut g = Geometry::new(0, 3000, 0, 2000, 10.0, 10.0, GH3XLNDZ);
        g.ux = 0.5; // 1 个原始单位 = 2 µm
        g.uy = 0.0; // 无 Y 单位
        assert!((recover::dist_um(&g, 0, 0, 10, 0) - 20.0).abs() < 1e-9);
        assert!(recover::dist_um(&g, 0, 0, 0, 50).abs() < 1e-9);
    }
}
