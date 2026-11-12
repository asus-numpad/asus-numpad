//! 面板参数与坐标轴：绝对范围、分辨率、单位换算，以及逐轴校准与提示。
use serde::{Deserialize, Serialize};

/// 触摸板面板参数（绝对坐标范围 + 两轴分辨率），来自 evdev 或校准。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
    /// units per µm
    pub ux: f64,
    pub uy: f64,
}

impl Panel {
    /// 毫米 → 采样单位（相对面板左上角）。
    pub fn mm_to_x(&self, mm: f64) -> f64 {
        self.xmin as f64 + mm * 1000.0 * self.ux
    }

    pub fn mm_to_y(&self, mm: f64) -> f64 {
        self.ymin as f64 + mm * 1000.0 * self.uy
    }
}

// --------------------------------------------------------------------------- //
// 手动校准
// --------------------------------------------------------------------------- //

/// 单轴校准：留空的项保持内核上报值。
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AxisCal {
    pub min: Option<i32>,
    pub max: Option<i32>,
    /// units per mm（与 evdev `resolution` 同一量纲）。
    pub units_per_mm: Option<f64>,
}

impl AxisCal {
    pub fn is_unset(&self) -> bool {
        self.min.is_none() && self.max.is_none() && self.units_per_mm.is_none()
    }
}

/// 手动校准段。
///
/// 只做全局线性映射：改范围、改分辨率。**不做**轴交换与翻转——那要改事件流，
/// 且真实机型里极少见；宁可让用户换 `--device`，也不摆一个配了没反应的开关。
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalibrationSpec {
    pub x: AxisCal,
    pub y: AxisCal,
}

impl CalibrationSpec {
    pub fn is_unset(&self) -> bool {
        self.x.is_unset() && self.y.is_unset()
    }

    /// 只靠配置本身就能判定的问题（不依赖面板实际范围）。
    ///
    /// `sanitize` 阶段还没摸到设备，拿不到 `ABS_MT_POSITION_*`；这里只查「写的范围反了」
    /// 和「单位换算是垃圾值」，好让配置一加载就把问题说出来。
    pub fn notes(&self) -> Vec<CalNote> {
        fn one(cal: &AxisCal, axis: &'static str, out: &mut Vec<CalNote>) {
            if let (Some(a), Some(b)) = (cal.min, cal.max) {
                if a >= b {
                    out.push(CalNote::Inverted(axis));
                }
            }
            if let Some(u) = cal.units_per_mm {
                if !(u > 0.0 && u.is_finite()) {
                    out.push(CalNote::BadUnitsPerMm(axis));
                }
            }
        }
        let mut out = Vec::new();
        one(&self.x, "x", &mut out);
        one(&self.y, "y", &mut out);
        out
    }

    /// 校正到面板参数上；返回 `(panel, notes)`。
    ///
    /// `notes` 里是「写了但不生效」的项（例如只给了 `min` 没给 `max`、范围反了），
    /// 由上层转成提示语——静默丢弃是最难查的那类问题。
    pub fn apply(&self, base: Panel) -> (Panel, Vec<CalNote>) {
        let mut notes = Vec::new();
        let mut p = base;
        let (nx, xnotes) = apply_axis(base.xmin, base.xmax, &self.x, "x");
        p.xmin = nx.0;
        p.xmax = nx.1;
        notes.extend(xnotes);
        let (ny, ynotes) = apply_axis(base.ymin, base.ymax, &self.y, "y");
        p.ymin = ny.0;
        p.ymax = ny.1;
        notes.extend(ynotes);
        if let Some(upm) = self.x.units_per_mm {
            if upm > 0.0 && upm.is_finite() {
                p.ux = upm / 1000.0;
            } else {
                notes.push(CalNote::BadUnitsPerMm("x"));
            }
        }
        if let Some(upm) = self.y.units_per_mm {
            if upm > 0.0 && upm.is_finite() {
                p.uy = upm / 1000.0;
            } else {
                notes.push(CalNote::BadUnitsPerMm("y"));
            }
        }
        (p, notes)
    }
}

pub(super) fn apply_axis(
    min: i32,
    max: i32,
    cal: &AxisCal,
    axis: &'static str,
) -> ((i32, i32), Vec<CalNote>) {
    let mut notes = Vec::new();
    let (mut lo, mut hi) = (min, max);
    match (cal.min, cal.max) {
        (Some(a), Some(b)) => {
            if a < b {
                lo = a;
                hi = b;
            } else {
                notes.push(CalNote::Inverted(axis));
            }
        }
        (Some(a), None) => {
            // 只给下界的常见意图是「左边多留一点」，上界保持内核值
            if a < hi {
                lo = a;
            } else {
                notes.push(CalNote::Inverted(axis));
            }
        }
        (None, Some(b)) => {
            if b > lo {
                hi = b;
            } else {
                notes.push(CalNote::Inverted(axis));
            }
        }
        (None, None) => {}
    }
    ((lo, hi), notes)
}

/// 校准段的提示项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalNote {
    /// 该轴写的范围是反的（下界不小于上界），整轴忽略
    Inverted(&'static str),
    /// 该轴的单位换算值非正/非有限，忽略
    BadUnitsPerMm(&'static str),
}

impl CalNote {
    /// i18n 键。
    pub fn key(self) -> &'static str {
        match self {
            CalNote::Inverted(_) => "note.calibration_inverted",
            CalNote::BadUnitsPerMm(_) => "note.calibration_bad_units",
        }
    }

    pub fn axis(self) -> &'static str {
        match self {
            CalNote::Inverted(a) | CalNote::BadUnitsPerMm(a) => a,
        }
    }
}

// --------------------------------------------------------------------------- //
// 自定义分区
// --------------------------------------------------------------------------- //
