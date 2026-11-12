//! numdeck —— ASUS 笔记本触摸板数字小键盘驱动的 Linux 用户态实现。
//!
//! 结构与官方驱动一一对应：
//!
//! | 本工程 | 官方对应 | 职责 |
//! |---|---|---|
//! | `keycodes` | `include/uapi/linux/input-event-codes.h` | 事件码常量 |
//! | `layout` | `NUMPADLAYOUT_GH3XLNDZ_00..14` | 0.1 mm 布局 → 绝对坐标矩形 → 键码 |
//! | `config` | `AsusNUMPADFilter.inf` 的 `AlgoParameters` | 帧数 / 阈值 / 点灯取值 |
//! | `engine` | `AsusNUMPADFilter` 的 Algo 状态机 | 触点状态机、点灯联动、混合模式 |
//! | `linux` | — | evdev 抓取、uinput 虚拟设备、hidraw/i2c 点灯、热插拔 |
//!
//! 纯逻辑部分（`layout` / `engine` / `hid_desc` / `led_policy`）不依赖 Linux，可在任意平台跑单测；
//! 硬件相关代码全部用 trait 注入，接入设备只需实现对应 trait。
//!
//! ## 一处来源差异
//!
//! 官方 INF 的 `NUMPAD_09 iNumLockMaxVaildDist = 2741` 在 `iUnit = 100 µm` 下只有
//! 0.27 mm，与「长按按住不动」的物理语义不符。本实现按字面值使用；若长按被判定成
//! 滑动、NumLock 切不动，先排查此处。

pub mod brightness;
pub mod compat;
pub mod config;
pub mod custom;
pub mod engine;
pub mod hid_desc;
pub mod i18n;
pub mod keycodes;
pub mod layout;
pub mod led_policy;
pub mod led_proto;
pub mod lifecycle;
pub mod partition;
pub mod sku;

#[cfg(target_os = "linux")]
pub mod linux;

pub use config::{Config, VERSION};
pub use engine::Engine;
pub use i18n::Lang;
pub use layout::{Geometry, KeyMode, Layout, GH3XLNDZ};

/// 与上游一致的启动横幅（随界面语言变化）。
pub fn banner() -> String {
    i18n::tf("app.banner", &[("version", VERSION)])
}

/// 测试专用：自定义布局注册表是进程级全局状态，多个测试并发读写会互相清空。
#[cfg(test)]
pub(crate) mod test_lock {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// 拿住这把锁的测试之间互斥，直到 guard 离开作用域。
    pub fn registry() -> MutexGuard<'static, ()> {
        lock().lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 语言表也是进程级全局态，改它的测试同样要互斥。
    pub fn i18n() -> MutexGuard<'static, ()> {
        lock().lock().unwrap_or_else(|e| e.into_inner())
    }
}
