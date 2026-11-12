//! 交互式校准采集：让用户按住触摸板四角与两个已知格中心，采回原始落点。
//!
//! 只做采集，不做判断——解算全在 [`crate::partition::solve_calibration`] 里，
//! 这样 macOS 上能用样本文件跑同一套算法，真机上只差「点从哪来」。
//!
//! 采集期间会 `grab` 设备：光标不动，避免用户按角时把鼠标拖到屏幕角落。

use std::io::{self, Write};

use super::dev::InputDevice;
use crate::i18n;
use crate::keycodes as kc;

/// 落点稳定多久算「按住不动」
const STABLE_MS: i32 = 400;
/// 单个落点最长等待（含用户反应时间）
const POINT_TIMEOUT_MS: i32 = 20_000;
/// 轮询粒度
const POLL_MS: i32 = 40;

fn say(msg: &str) {
    println!("{msg}");
    let _ = io::stdout().flush();
}

fn ask(msg: &str) {
    print!("{msg}");
    let _ = io::stdout().flush();
}

/// 采集一个落点：等手指按下 → 收集 `STABLE_MS` 内的点 → 取中位数。
///
/// 中位数而不是平均值：手指按下的瞬间常带一段从别处滑过来的轨迹，
/// 均值会被那段拖偏，中位数不会。
fn sample_point(dev: &InputDevice, prompt: &str) -> io::Result<(i32, i32)> {
    ask(prompt);
    let mut xs: Vec<i32> = Vec::new();
    let mut ys: Vec<i32> = Vec::new();
    let mut cur = (0i32, 0i32);
    let mut down = false;
    let mut collecting_since: Option<(i64, i64)> = None;
    let mut waited = 0;

    while waited < POINT_TIMEOUT_MS {
        if !dev.wait_readable(POLL_MS)? {
            waited += POLL_MS;
            continue;
        }
        for ev in dev.read_events()? {
            match (ev.etype, ev.code) {
                (kc::EV_ABS, kc::ABS_MT_POSITION_X) => cur.0 = ev.value,
                (kc::EV_ABS, kc::ABS_MT_POSITION_Y) => cur.1 = ev.value,
                (kc::EV_ABS, kc::ABS_MT_TRACKING_ID) => {
                    let touched = ev.value >= 0;
                    if touched && !down {
                        // 追踪 ID 出现才是真正的一次新按下
                        down = true;
                        collecting_since = Some((ev.sec, ev.usec));
                        xs.clear();
                        ys.clear();
                    } else if !touched {
                        down = false;
                        collecting_since = None;
                    }
                }
                (kc::EV_KEY, kc::BTN_TOUCH) => {
                    if ev.value != 0 && !down {
                        down = true;
                        collecting_since = Some((ev.sec, ev.usec));
                        xs.clear();
                        ys.clear();
                    } else if ev.value == 0 {
                        down = false;
                        collecting_since = None;
                    }
                }
                _ => {}
            }
            if down {
                xs.push(cur.0);
                ys.push(cur.1);
                if let Some((sec, usec)) = collecting_since {
                    let elapsed_ms = (ev.sec - sec) * 1000 + (ev.usec - usec) / 1000;
                    if elapsed_ms >= STABLE_MS as i64 && !xs.is_empty() {
                        println!();
                        return Ok((median(&mut xs), median(&mut ys)));
                    }
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        i18n::t("calibrate.timeout"),
    ))
}

fn median(v: &mut [i32]) -> i32 {
    v.sort_unstable();
    v[v.len() / 2]
}

/// 四角 + 两个格中心的完整采集。
///
/// 角点顺序按「左上 → 右上 → 左下 → 右下」，和 `CornerSamples` 的字段一一对应。
/// 顺序搞反是最常见的操作失误，所以解算侧还会再查一次（见 `solve_calibration`）。
pub fn collect(dev: &InputDevice) -> io::Result<crate::partition::RawSamples> {
    dev.grab().ok();
    say(&i18n::t("calibrate.intro"));
    say("");
    let tl = sample_point(dev, &i18n::t("calibrate.prompt.tl"))?;
    let tr = sample_point(dev, &i18n::t("calibrate.prompt.tr"))?;
    let bl = sample_point(dev, &i18n::t("calibrate.prompt.bl"))?;
    let br = sample_point(dev, &i18n::t("calibrate.prompt.br"))?;
    say("");
    say(&i18n::t("calibrate.intro_cells"));
    let first = sample_point(dev, &i18n::t("calibrate.prompt.cell_first"))?;
    let last = sample_point(dev, &i18n::t("calibrate.prompt.cell_last"))?;
    dev.ungrab().ok();
    Ok(crate::partition::RawSamples {
        corners: crate::partition::CornerSamples {
            top_left: tl,
            top_right: tr,
            bottom_left: bl,
            bottom_right: br,
        },
        first_point: first,
        last_point: last,
    })
}
