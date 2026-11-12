//! `bright`：亮度读写。

use std::path::Path;

use anyhow::Result;
use numdeck::config::{Config, VAL_OFF};
use numdeck::i18n::{self};

use super::*;

/// 键盘背光 → 数字区亮度档的当前换算结果（`check` / `run` 启动横幅用）。
pub(crate) fn brightness_report(cfg: &Config) -> String {
    let levels = cfg.effective_levels();
    if levels.is_empty() {
        return i18n::tf(
            "bright.report.static",
            &[
                ("step", &cfg.manual_brightness_step.to_string()),
                ("level", &format!("{:02x}", cfg.static_on_value())),
            ],
        );
    }
    let mut follower = numdeck::brightness::BrightnessFollower::new(
        &cfg.keyboard_brightness_dir,
        levels.clone(),
    );
    let level = follower.on_value();
    match follower.last_kbd {
        Some(kbd) => i18n::tf(
            "bright.report.follow",
            &[
                ("kbd", &kbd.to_string()),
                ("max", &follower.max_kbd().max(1).to_string()),
                ("level", &format!("{level:02x}")),
                ("n", &levels.len().to_string()),
                ("ms", &cfg.brightness_poll_ms.to_string()),
                ("dir", &follower.dir().display().to_string()),
            ],
        ),
        None => i18n::tf(
            "bright.report.follow_nodir",
            &[
                ("dir", &follower.dir().display().to_string()),
                ("level", &format!("{level:02x}")),
            ],
        ),
    }
}

/// 打印三档亮度 / 跟随开关的当前状态。
pub(crate) fn print_bright_status(cfg: &Config) {
    println!(
        "{}",
        i18n::tf(
            "bright.follow_switch",
            &[(
                "state",
                &i18n::t(if cfg.follow_keyboard_brightness {
                    "bright.follow_on"
                } else {
                    "bright.follow_off"
                }),
            )]
        )
    );
    println!(
        "{}",
        i18n::tf(
            "bright.status_dir",
            &[
                ("dir", &cfg.keyboard_brightness_dir),
                ("ms", &cfg.brightness_poll_ms.to_string()),
            ]
        )
    );
    println!(
        "{}",
        i18n::tf(
            "bright.status_step",
            &[
                ("step", &cfg.manual_brightness_step.to_string()),
                ("steps", &cfg.manual_steps().to_string()),
            ]
        )
    );

    let mut table = Vec::new();
    for step in 1..=cfg.manual_steps() {
        let v = cfg.level_for_step(step).unwrap_or(VAL_OFF);
        table.push(format!("{step}→0x{v:02x}"));
    }
    println!(
        "{}",
        i18n::tf(
            "bright.status_map",
            &[
                ("off", &format!("{VAL_OFF:02x}")),
                ("table", &table.join(&i18n::t("common.list_sep")))
            ]
        )
    );
    println!(
        "{}",
        i18n::tf(
            "bright.status_effective",
            &[("text", &brightness_report(cfg))]
        )
    );

    let mut follower = numdeck::brightness::BrightnessFollower::new(
        &cfg.keyboard_brightness_dir,
        cfg.manual_levels(),
    );
    let cur = follower.on_value();
    match follower.last_kbd {
        Some(kbd) => println!(
            "{}",
            i18n::tf(
                "bright.measured",
                &[
                    ("kbd", &kbd.to_string()),
                    ("max", &follower.max_kbd().max(1).to_string()),
                    ("cur", &format!("{cur:02x}")),
                ]
            )
        ),
        None => println!(
            "{}",
            i18n::tf(
                "bright.measure_failed",
                &[("dir", &follower.dir().display().to_string())]
            )
        ),
    }
}

/// `bright`：手动三档调光 + 是否跟随键盘背光。
pub(crate) fn cmd_bright(
    cfg: &Config,
    cfg_path: &Path,
    value: Option<&str>,
    mode: Option<&str>,
) -> Result<i32> {
    let value = value.map(|v| v.trim().to_ascii_lowercase());
    match value.as_deref() {
        None | Some("status") => {
            print_bright_status(cfg);
            Ok(0)
        }
        Some("follow") => {
            let on = match mode.map(|m| m.trim().to_ascii_lowercase()).as_deref() {
                Some("on") | Some("true") | Some("1") | Some("yes") => true,
                Some("off") | Some("false") | Some("0") | Some("no") => false,
                _ => {
                    println!("{}", i18n::t("bright.follow_usage"));
                    return Ok(2);
                }
            };
            let backup = persist_config_keys(
                cfg_path,
                &[("follow_keyboard_brightness", serde_json::json!(on))],
            )?;
            println!(
                "{}",
                i18n::tf(
                    "bright.wrote_follow",
                    &[
                        ("path", &cfg_path.display().to_string()),
                        ("on", if on { "true" } else { "false" }),
                        ("backup", &backup.display().to_string()),
                    ]
                )
            );
            if on {
                println!("{}", i18n::t("bright.effect_follow_on"));
            } else {
                println!(
                    "{}",
                    i18n::tf(
                        "bright.effect_follow_off",
                        &[
                            ("step", &cfg.manual_brightness_step.to_string()),
                            ("level", &format!("{:02x}", cfg.static_on_value())),
                        ]
                    )
                );
            }
            try_restart_service();
            Ok(0)
        }
        Some("cycle") | Some("next") => {
            let cur = cfg.manual_brightness_step;
            let steps = cfg.manual_steps();
            let next = if cur == 0 || cur >= steps { 1 } else { cur + 1 };
            println!(
                "{}",
                i18n::tf(
                    "bright.switch",
                    &[("from", &cur.to_string()), ("to", &next.to_string())]
                )
            );
            apply_step(cfg, cfg_path, next)
        }
        Some("up") | Some("+") => {
            let steps = cfg.manual_steps();
            let cur = cfg.manual_brightness_step.clamp(1, steps);
            let next = (cur + 1).min(steps);
            println!(
                "{}",
                i18n::tf(
                    "bright.switch_up",
                    &[("from", &cur.to_string()), ("to", &next.to_string())]
                )
            );
            apply_step(cfg, cfg_path, next)
        }
        Some("down") | Some("-") => {
            let steps = cfg.manual_steps();
            let cur = cfg.manual_brightness_step.clamp(1, steps);
            let next = cur.saturating_sub(1).max(1);
            println!(
                "{}",
                i18n::tf(
                    "bright.switch_down",
                    &[("from", &cur.to_string()), ("to", &next.to_string())]
                )
            );
            apply_step(cfg, cfg_path, next)
        }
        Some(other) => {
            let steps = cfg.manual_steps();
            let step: u8 = match other.parse::<u8>() {
                Ok(v) if v <= steps => v,
                _ => {
                    println!("{}", i18n::t("bright.usage"));
                    return Ok(2);
                }
            };
            apply_step(cfg, cfg_path, step)
        }
    }
}
