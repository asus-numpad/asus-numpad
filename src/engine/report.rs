//! 展示文本拼装：`--dump` 状态行、`--check` 几何报告。
//!
//! 只读 `Engine` 的字段做格式化，不改变任何状态，也不参与状态机判定；
//! 因此可以整体替换（换语种、换格式）而不影响逻辑。

use super::Engine;
use crate::keycodes as kc;

pub(super) fn state_line(e: &Engine) -> String {
    format!(
            "x={} y={} dist={:.0}µm spread={:.1}mm numlock={} cell={} key={} mode={} enabled={} fwd={} pressure={} hotkey={}",
            e.cur_x,
            e.cur_y,
            e.moved_units,
            e.scroll_spread_mm,
            e.geometry.in_numlock(e.cur_x, e.cur_y),
            e.geometry
                .cell_of(e.cur_x, e.cur_y)
                .map(|(r, c)| format!("({r},{c})"))
                .unwrap_or_else(|| "None".into()),
            e.geometry
                .key_at_with(
                    &e.table,
                    e.cur_x,
                    e.cur_y,
                    e.cfg.key_mode,
                    e.tap_tol()
                )
                .map(|k| kc::name_of(k).to_string())
                .unwrap_or_else(|| "None".into()),
            e.mode.as_str(),
            e.enabled,
            e.forwarding,
            if e.pressure_available() {
                e.pressure_now
            } else {
                0
            },
            e.hotkey_last.unwrap_or("-")
        )
}

pub(super) fn geometry_lines(e: &Engine) -> Vec<String> {
    let mut out = Vec::new();
    out.push(crate::i18n::tf(
        "engine.geo.range",
        &[
            ("x0", &e.geometry.xmin.to_string()),
            ("x1", &e.geometry.xmax.to_string()),
            ("y0", &e.geometry.ymin.to_string()),
            ("y1", &e.geometry.ymax.to_string()),
        ],
    ));
    out.push(crate::i18n::tf(
        "engine.geo.unit",
        &[
            (
                "ux",
                &format!(
                    "{:.1}",
                    if e.geometry.ux > 0.0 {
                        1.0 / e.geometry.ux
                    } else {
                        0.0
                    }
                ),
            ),
            (
                "uy",
                &format!(
                    "{:.1}",
                    if e.geometry.uy > 0.0 {
                        1.0 / e.geometry.uy
                    } else {
                        0.0
                    }
                ),
            ),
        ],
    ));
    out.push(crate::i18n::tf(
        "engine.geo.official",
        &[("text", &e.geometry.describe())],
    ));
    out
}
