//! 状态恢复：丢事件补偿、帧节拍换算、距离换算。
//!
//! 这里的函数都不碰 `Engine` 的字段状态，只做「给定输入算出结果」的纯换算，
//! 方便单测覆盖；需要改 Engine 状态的动作仍旧留在 `Engine` 的方法里。

use super::Event;
use crate::keycodes as kc;
use crate::layout::Geometry;

/// 自 `t0` 起经过的帧数（官方帧模型默认 8 ms/帧）。
///
/// `frame_ms` 为 0 时按 1 ms 处理，避免除零得到 inf。
pub(super) fn frames_between(t0: f64, now: f64, frame_ms: u32) -> f64 {
    (now - t0) * 1000.0 / (frame_ms.max(1) as f64 * 1000.0)
}

/// 两点实际距离（µm）。单位换算取自 `Geometry`，缺一项就按 0 算，不放大误差。
pub(super) fn dist_um(geometry: &Geometry, x0: i32, y0: i32, x1: i32, y1: i32) -> f64 {
    let dx = if geometry.ux > 0.0 {
        (x1 - x0) as f64 / geometry.ux
    } else {
        0.0
    };
    let dy = if geometry.uy > 0.0 {
        (y1 - y0) as f64 / geometry.uy
    } else {
        0.0
    };
    (dx * dx + dy * dy).sqrt()
}

/// 给虚拟触摸板补的「全部抬起」序列：每个在按触点一个 SLOT + TRACKING_ID(-1)，
/// 最后补一个 SYN 让内核把这一批当成完整报文收下。
pub(super) fn release_all(slots: &[i32]) -> Vec<Event> {
    let mut out = Vec::with_capacity(slots.len() * 2 + 1);
    for slot in slots {
        out.push(Event {
            etype: kc::EV_ABS,
            code: kc::ABS_MT_SLOT,
            value: *slot,
        });
        out.push(Event {
            etype: kc::EV_ABS,
            code: kc::ABS_MT_TRACKING_ID,
            value: -1,
        });
    }
    out.push(Event {
        etype: kc::EV_SYN,
        code: 0,
        value: 0,
    });
    out
}
