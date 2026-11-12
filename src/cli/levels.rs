//! `levels`：档位表、按档试亮、恢复默认四档。

use std::path::Path;

use anyhow::Result;
use numdeck::config::{self, Config, VAL_OFF};
use numdeck::i18n::{self};

use super::*;

/// 解析一个亮度值：十进制（`65`）或十六进制（`0x41`）。
pub(crate) fn parse_level_value(raw: &str) -> Option<u8> {
    let t = raw.trim().to_ascii_lowercase();
    match t.strip_prefix("0x") {
        Some(hex) => u8::from_str_radix(hex, 16).ok(),
        None => t.parse::<u8>().ok(),
    }
}

/// 直接下发一个亮度字节到背光通道；返回是否成功。
pub(crate) fn apply_led_level(cfg: &Config, level: u8) -> bool {
    #[cfg(target_os = "linux")]
    {
        let mut led = numdeck::linux::led::build_led(cfg);
        let ok = led.send(level);
        if ok {
            println!(
                "{}",
                i18n::tf(
                    "bright.sent",
                    &[("level", &format!("{level:02x}")), ("kind", &cfg.led_kind)]
                )
            );
        } else {
            println!(
                "{}",
                i18n::tf("bright.send_failed", &[("level", &format!("{level:02x}"))])
            );
        }
        ok
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (cfg, level);
        println!("{}", i18n::t("bright.nonlinux"));
        false
    }
}

/// 落盘 + 立即下发 + 让服务按新档重跑（`bright 1|2|3`、`up|down|cycle` 共用）。
pub(crate) fn apply_step(cfg: &Config, cfg_path: &Path, step: u8) -> Result<i32> {
    let Some(level) = cfg.level_for_step(step) else {
        println!(
            "{}",
            i18n::tf("bright.err_unmappable", &[("step", &step.to_string())])
        );
        return Ok(2);
    };
    let ok = apply_led_level(cfg, level);
    // 手动选档等价于「不跟随键盘背光」：否则键盘背光一变就会把手动档覆盖掉
    let backup = persist_config_keys(
        cfg_path,
        &[
            ("follow_keyboard_brightness", serde_json::json!(false)),
            ("manual_brightness_step", serde_json::json!(step)),
        ],
    )?;
    println!(
        "{}",
        i18n::tf(
            "bright.wrote_step",
            &[
                ("path", &cfg_path.display().to_string()),
                ("step", &step.to_string()),
                ("backup", &backup.display().to_string()),
            ]
        )
    );
    if level == VAL_OFF {
        println!("{}", i18n::t("bright.effect_off"));
    } else {
        println!(
            "{}",
            i18n::tf("bright.effect_fixed", &[("level", &format!("{level:02x}"))])
        );
    }
    try_restart_service();
    Ok(if ok { 0 } else { 1 })
}

/// 打印当前档位表（每档 → 实际下发的字节）。
pub(crate) fn print_levels_table(cfg: &Config) {
    let mut table = Vec::new();
    for step in 1..=cfg.manual_steps() {
        let v = cfg.level_for_step(step).unwrap_or(VAL_OFF);
        table.push(format!("{step}→0x{v:02x}"));
    }
    println!(
        "{}",
        i18n::tf(
            "levels.table",
            &[
                ("steps", &cfg.manual_steps().to_string()),
                ("table", &table.join(&i18n::t("common.list_sep"))),
            ]
        )
    );
    println!(
        "{}",
        i18n::tf("levels.range", &[("src", &cfg.keyboard_brightness_dir)])
    );
}

/// `levels`：查看档位表 / 给某一档指定任意支持亮度 / 恢复默认四档。
///
/// 需求是「每个档位都能指定任意一个支持的亮度，现在的线性四档只作默认」，所以：
/// * `numdeck levels`            → 打印当前档表（要看得出每档到底发多少字节）
/// * `numdeck levels 2 0x46`     → 把第 2 档设成 0x46，其余档不动
/// * `numdeck levels 3 71`       → 十进制写法，等价 0x47
/// * `numdeck levels reset`      → 恢复默认四档 `[0, 0x43, 0x45, 0x48]`
///
/// 合法值只有硬件的 `0x00`（熄灭）与 `0x41..=0x48`（八档亮度），改完仍要求非递减，
/// 否则 `bright up` 会越按越黑。
pub(crate) fn cmd_levels(
    cfg: &Config,
    cfg_path: &Path,
    step: Option<&str>,
    value: Option<&str>,
) -> Result<i32> {
    let step = step.map(|s| s.trim().to_ascii_lowercase());
    let step = step.as_deref();
    match (step, value) {
        (None, None) => {
            print_levels_table(cfg);
            Ok(0)
        }
        (Some("reset") | Some("default"), None) => {
            let levels = config::DEFAULT_MANUAL_LEVELS.to_vec();
            let backup = persist_config_keys(
                cfg_path,
                &[("manual_brightness_levels", serde_json::json!(levels))],
            )?;
            println!(
                "{}",
                i18n::tf(
                    "levels.reset",
                    &[
                        ("path", &cfg_path.display().to_string()),
                        ("backup", &backup.display().to_string()),
                    ]
                )
            );
            print_levels_table(&Config {
                manual_brightness_levels: Some(levels),
                ..cfg.clone()
            });
            try_restart_service();
            Ok(0)
        }
        (Some(s), Some(raw)) => {
            let Ok(step_no) = s.parse::<usize>() else {
                println!(
                    "{}",
                    i18n::tf(
                        "levels.err_step",
                        &[("step", s), ("steps", &cfg.manual_steps().to_string()),]
                    )
                );
                println!("{}", i18n::t("levels.usage"));
                return Ok(2);
            };
            let Some(level) = parse_level_value(raw) else {
                println!("{}", i18n::tf("levels.err_value", &[("value", raw)]));
                return Ok(2);
            };
            let mut next = cfg.clone();
            match next.set_manual_level(step_no, level) {
                Ok(levels) => {
                    let backup = persist_config_keys(
                        cfg_path,
                        &[("manual_brightness_levels", serde_json::json!(levels))],
                    )?;
                    println!(
                        "{}",
                        i18n::tf(
                            "levels.wrote",
                            &[
                                ("step", &step_no.to_string()),
                                ("level", &format!("{level:02x}")),
                                ("path", &cfg_path.display().to_string()),
                                ("backup", &backup.display().to_string()),
                            ]
                        )
                    );
                    print_levels_table(&next);
                    // 档表变了，常驻服务要重新读配置才生效
                    try_restart_service();
                    Ok(0)
                }
                Err(config::LevelSetError::Value) => {
                    println!("{}", i18n::tf("levels.err_value", &[("value", raw)]));
                    Ok(2)
                }
                Err(config::LevelSetError::Step) => {
                    println!(
                        "{}",
                        i18n::tf(
                            "levels.err_step",
                            &[
                                ("step", &step_no.to_string()),
                                ("steps", &cfg.manual_steps().to_string()),
                            ]
                        )
                    );
                    Ok(2)
                }
                Err(config::LevelSetError::Order) => {
                    println!(
                        "{}",
                        i18n::tf("levels.err_order", &[("step", &step_no.to_string())])
                    );
                    Ok(2)
                }
            }
        }
        _ => {
            println!("{}", i18n::t("levels.usage"));
            Ok(2)
        }
    }
}
