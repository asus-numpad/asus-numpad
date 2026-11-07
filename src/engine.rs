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
    has_pressure: bool,
    last_activity_ms: f64,
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
    pub pressure: Option<i32>,
    await_pressure: bool,
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
    pub fn new(
        cfg: Config,
        geometry: Geometry,
        deps: Deps,
        has_mt_tracking: bool,
        has_pressure: bool,
    ) -> Self {
        let tol_ux = cfg.tap_threshold_mm * 1000.0 * geometry.ux;
        let tol_uy = cfg.tap_threshold_mm * 1000.0 * geometry.uy;
        let now = deps.clock.now_ms();
        // 必须在 cfg 被移入 Self 之前构造
        let brightness =
            BrightnessFollower::new(&cfg.keyboard_brightness_dir, cfg.effective_levels());
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
            running: true,
            slot: 0,
            primary: None,
            has_mt_tracking,
            has_pressure,
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
            pressure: None,
            await_pressure: false,
            armed_key: None,
            armed_since: 0.0,
            armed_delay_frames: 0,
            pending_events: Vec::new(),
            forwarding: false,
            pending_level: None,
            pending_level_at: 0.0,
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
        (self.now() - t0) * 1000.0 / (self.cfg.frame_ms.max(1) as f64 * 1000.0)
    }

    pub fn numlock_frames(&self) -> u32 {
        self.cfg.numlock_frames()
    }

    /// 两点实际距离（µm）。
    pub fn dist_um(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> f64 {
        let dx = if self.geometry.ux > 0.0 {
            (x1 - x0) as f64 / self.geometry.ux
        } else {
            0.0
        };
        let dy = if self.geometry.uy > 0.0 {
            (y1 - y0) as f64 / self.geometry.uy
        } else {
            0.0
        };
        (dx * dx + dy * dy).sqrt()
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

    pub fn disable(&mut self) {
        if !self.enabled {
            return;
        }
        self.led_off(&crate::i18n::t("engine.reason.exit"));
        self.numlock_restore_state();
        self.numlock_forced = false;
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
        self.pressure = None;
        self.await_pressure = false;
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

    /// 小键盘态按键是否达到官方要求的下压档位。
    ///
    /// 官方 FORCE_07 Normal = 1、FORCE_08 Particular（小键盘模式）= 2，即小键盘态
    /// 需要更实的下压才能出键，用于抑制轻扫/手掌误触。INF 只给档位号、未给原始压力值，
    /// 阈值由 `pressure_numpad_min` 校准；未配置或设备无压力轴时旁路。
    fn pressure_ready(&self) -> bool {
        let thr = self.cfg.pressure_numpad_min;
        if thr <= 0 {
            return true;
        }
        if !self.has_pressure {
            // 设备根本不上报压力轴 → 只能旁路，否则所有键都不出
            return true;
        }
        match self.pressure {
            None => false, // 本次触点的压力尚未上报，先挂起等它到齐
            Some(p) => p >= thr,
        }
    }

    /// 按下点命中判定并按需发键；压力尚未上报时挂起，等压力到齐再判定。
    fn emit_at_down_point(&mut self) -> bool {
        let key = self.geometry.key_at(
            self.x0,
            self.y0,
            self.cfg.key_mode,
            self.cfg.tap_threshold_mm,
            self.tol_ux,
            self.tol_uy,
        );
        let Some(key) = key else { return false };
        if !self.pressure_ready() {
            self.await_pressure = true;
            return true;
        }
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

    /// 统一入口：先维护多指集合，再跑官方状态机，最后决定事件去向。
    pub fn handle_event(&mut self, ev: &Event) {
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

    /// 当前事件是否来自非首个触点（第二个手指的坐标/压力不该改动首触点的判键状态）。
    fn slot_is_primary(&self) -> bool {
        matches!(self.primary, Some(p) if self.slot == p)
    }

    fn handle_event_inner(&mut self, ev: &Event) {
        match ev.etype {
            kc::EV_ABS => {
                if ev.code == kc::ABS_PRESSURE || ev.code == kc::ABS_MT_PRESSURE {
                    if self.has_mt_tracking && !self.slot_is_primary() {
                        return;
                    }
                    self.pressure = Some(ev.value);
                    // 按下判定时压力还没到齐 → 此刻补判，避免把小键盘态变成「按了没反应」。
                    // 压力不达标时保持等待：同一触点后续压力上升仍可出键；始终不达标则整触点不出键
                    if self.await_pressure && self.down && self.pressure_ready() {
                        self.await_pressure = false;
                        let key = self.geometry.key_at(
                            self.x0,
                            self.y0,
                            self.cfg.key_mode,
                            self.cfg.tap_threshold_mm,
                            self.tol_ux,
                            self.tol_uy,
                        );
                        if let Some(key) = key {
                            if self.pressed_key.is_none() {
                                self.mode = Mode::Key;
                                self.pressed_key = Some(key);
                                self.emit_key(key);
                            }
                        }
                    }
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
        format!(
            "x={} y={} p={} dist={:.0}µm spread={:.1}mm numlock={} cell={} key={} mode={} enabled={} fwd={}",
            self.cur_x,
            self.cur_y,
            self.pressure.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
            self.moved_units,
            self.scroll_spread_mm,
            self.geometry.in_numlock(self.cur_x, self.cur_y),
            self.geometry
                .cell_of(self.cur_x, self.cur_y)
                .map(|(r, c)| format!("({r},{c})"))
                .unwrap_or_else(|| "None".into()),
            self.geometry
                .key_at(
                    self.cur_x,
                    self.cur_y,
                    self.cfg.key_mode,
                    self.cfg.tap_threshold_mm,
                    self.tol_ux,
                    self.tol_uy
                )
                .map(|k| kc::name_of(k).to_string())
                .unwrap_or_else(|| "None".into()),
            self.mode.as_str(),
            self.enabled,
            self.forwarding
        )
    }

    /// `--check` 用：把几何与关键判定打印出来。
    pub fn geometry_report(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(crate::i18n::tf(
            "engine.geo.range",
            &[
                ("x0", &self.geometry.xmin.to_string()),
                ("x1", &self.geometry.xmax.to_string()),
                ("y0", &self.geometry.ymin.to_string()),
                ("y1", &self.geometry.ymax.to_string()),
            ],
        ));
        out.push(crate::i18n::tf(
            "engine.geo.unit",
            &[
                (
                    "ux",
                    &format!(
                        "{:.1}",
                        if self.geometry.ux > 0.0 {
                            1.0 / self.geometry.ux
                        } else {
                            0.0
                        }
                    ),
                ),
                (
                    "uy",
                    &format!(
                        "{:.1}",
                        if self.geometry.uy > 0.0 {
                            1.0 / self.geometry.uy
                        } else {
                            0.0
                        }
                    ),
                ),
            ],
        ));
        out.push(crate::i18n::tf(
            "engine.geo.official",
            &[("text", &self.geometry.describe())],
        ));
        out.push(crate::i18n::tf(
            "engine.geo.pressure",
            &[
                (
                    "state",
                    &crate::i18n::t(if self.has_pressure {
                        "engine.geo.pressure_yes"
                    } else {
                        "engine.geo.pressure_no"
                    }),
                ),
                ("min", &self.cfg.pressure_numpad_min.to_string()),
            ],
        ));
        out
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
        let geom = Geometry::new(0, 3000, 0, 2000, 10.0, 10.0, GH3XLNDZ);
        let pointer: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));
        let keys: Rc<RefCell<FakeKeys>> = Rc::new(RefCell::new(FakeKeys::default()));
        let deps = Deps {
            led: Box::new(SpyLed { log: led_log }),
            keys: Box::new(keys),
            pointer: Some(Box::new(pointer.clone())),
            numlock: Some(Box::new(NumLockOn)),
            grabber: Some(Box::new(FakeGrab)),
            clock: Box::new(ManualClock { now }),
        };
        (Engine::new(cfg, geom, deps, true, false), pointer)
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

    /// 两段式点灯：亮度档推迟到 tick 补发，主循环不再被 sleep 卡住；
    /// 期间若熄灭，未下发的档位必须作废，否则灯会被重新点亮。
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
}
