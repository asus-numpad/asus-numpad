//! 压感切换：用力按 = 开关数字键盘。

use serde::{Deserialize, Serialize};

/// 压感触摸板的「用力按 = 切换」配置。
///
/// 只有上报 `ABS_PRESSURE` / `ABS_MT_PRESSURE` 的模组才谈得上压感；没有压力轴
/// 的机型开了本项不会有任何效果（也不会报错），所以默认关闭。
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PressureSwitch {
    pub enabled: bool,
    /// 触发阈值：压力轴满量程的百分比（1..=100）。
    pub percent: u8,
    /// 需要连续保持多少帧（一帧 = `frame_ms`，默认 8 ms）。
    pub frames: u32,
    /// 两次切换之间的最小间隔（ms），防止按住不放连续翻转。
    pub cooldown_ms: f64,
}

impl Default for PressureSwitch {
    fn default() -> Self {
        Self {
            enabled: false,
            percent: 80,
            frames: 6,
            cooldown_ms: 600.0,
        }
    }
}

impl PressureSwitch {
    pub fn sanitized(mut self) -> Self {
        self.percent = self.percent.clamp(1, 100);
        self.frames = self.frames.clamp(1, 200);
        if !(self.cooldown_ms.is_finite() && self.cooldown_ms >= 0.0) {
            self.cooldown_ms = 600.0;
        }
        self
    }
}

/// 压感切换状态机（纯逻辑，可单测）。
#[derive(Debug, Clone)]
pub struct PressureGate {
    cfg: PressureSwitch,
    run: u32,
    armed: bool,
    last_ms: f64,
    fired: u64,
}

impl PressureGate {
    pub fn new(cfg: PressureSwitch) -> Self {
        Self {
            cfg: cfg.sanitized(),
            run: 0,
            armed: true,
            last_ms: f64::NEG_INFINITY,
            fired: 0,
        }
    }

    pub fn enabled(&self) -> bool {
        self.cfg.enabled
    }

    pub fn config(&self) -> PressureSwitch {
        self.cfg
    }

    /// 累计触发次数（测试与日志用）。
    pub fn fired(&self) -> u64 {
        self.fired
    }

    /// 喂一个压力采样，返回是否应当在本次触发切换。
    ///
    /// * `value` 压力原始值，`max` 该轴的 `maximum`（<=0 表示没有压力轴）；
    /// * `contact` 当前是否有触点，抬起即清零；
    /// * 触发后必须等压力回落到阈值以下才会重新武装，避免一次长按翻转多次。
    pub fn feed(&mut self, value: i32, max: i32, contact: bool, now_ms: f64) -> bool {
        if !self.cfg.enabled || max <= 0 || !contact {
            self.run = 0;
            self.armed = true;
            return false;
        }
        let threshold = ((max as i64 * self.cfg.percent as i64) / 100).max(1) as i32;
        if value < threshold {
            self.run = 0;
            self.armed = true;
            return false;
        }
        if !self.armed {
            return false;
        }
        self.run += 1;
        if self.run < self.cfg.frames {
            return false;
        }
        if now_ms - self.last_ms < self.cfg.cooldown_ms {
            return false;
        }
        self.run = 0;
        self.armed = false;
        self.last_ms = now_ms;
        self.fired += 1;
        true
    }
}
