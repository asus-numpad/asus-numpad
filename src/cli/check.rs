//! `check`：只读体检，把识别结果、几何、键表、描述符、服务状态摆成一份报告。

use anyhow::Result;
use numdeck::banner;
use numdeck::config::{self, CalcMode, Config};
use numdeck::i18n::{self, LangSource};
use numdeck::keycodes as kc;
use numdeck::layout::{Tol, COLS, ROWS, UM_PER_LAYOUT_UNIT};

use super::*;
use crate::LANG_SOURCE;

pub(crate) fn cmd_check(cfg: &Config, notes: &[String]) -> Result<i32> {
    println!("{}", banner());
    println!();
    println!("{}", sku_block(cfg));
    println!();
    if !notes.is_empty() {
        println!("{}", i18n::t("check.notes_title"));
        for n in notes {
            println!("  - {n}");
        }
        println!();
    }

    println!(
        "{}",
        i18n::tf("check.summary", &[("summary", &cfg.summary())])
    );
    println!();
    println!("{}", i18n::t("check.official_title"));
    for (name, value) in config::official_parameters() {
        println!("  {name:<32} {value}");
    }
    println!();

    let geometry = {
        #[cfg(target_os = "linux")]
        {
            match numdeck::linux::probe_report(cfg) {
                Ok((g, lines)) => {
                    println!("{}", i18n::t("check.device.probed"));
                    for line in lines {
                        println!("  {line}");
                    }
                    g
                }
                Err(e) => {
                    println!(
                        "{}",
                        i18n::tf(
                            "check.device.fallback",
                            &[
                                ("err", &e.to_string()),
                                ("w", &format!("{:.0}", cfg.touchpad_width_mm)),
                                ("h", &format!("{:.0}", cfg.touchpad_height_mm)),
                            ]
                        )
                    );
                    fallback_geometry(cfg)
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            println!(
                "{}",
                i18n::tf(
                    "check.device.nonlinux",
                    &[
                        ("w", &format!("{:.0}", cfg.touchpad_width_mm)),
                        ("h", &format!("{:.0}", cfg.touchpad_height_mm)),
                    ]
                )
            );
            fallback_geometry(cfg)
        }
    };

    println!();
    println!(
        "{}",
        i18n::tf(
            "check.geometry_title",
            &[("unit", &UM_PER_LAYOUT_UNIT.to_string())]
        )
    );
    println!("  {}", geometry.describe());
    println!(
        "  {}",
        i18n::tf(
            "check.geometry_usable",
            &[
                ("x0", &format!("{:.0}", geometry.usable_x0)),
                ("x1", &format!("{:.0}", geometry.usable_x1)),
                ("y0", &format!("{:.0}", geometry.usable_y0)),
                ("y1", &format!("{:.0}", geometry.usable_y1)),
            ]
        )
    );
    println!();
    println!(
        "{}",
        i18n::tf(
            "check.keygrid_title",
            &[
                ("cols", &COLS.to_string()),
                ("rows", &ROWS.to_string()),
                ("mode", cfg.key_mode.as_str()),
            ]
        )
    );
    print!("{}", render_keygrid(cfg));

    println!();
    println!("{}", i18n::t("check.cells_title"));
    for r in 0..ROWS {
        for c in 0..COLS {
            let (cx, cy) = geometry.cell_center(r, c);
            let key = geometry
                .key_at(
                    cx.round() as i32,
                    cy.round() as i32,
                    cfg.key_mode,
                    Tol::new(
                        cfg.tap_threshold_mm,
                        1000.0 * geometry.ux,
                        1000.0 * geometry.uy,
                    ),
                )
                .map(|k| kc::name_of(k).to_string())
                .unwrap_or_else(|| "None".into());
            println!(
                "  {}",
                i18n::tf(
                    "check.cell_line",
                    &[
                        ("r", &r.to_string()),
                        ("c", &c.to_string()),
                        ("cx", &format!("{cx:.0}")),
                        ("cy", &format!("{cy:.0}")),
                        ("key", &key),
                    ]
                )
            );
        }
    }

    println!();
    println!("{}", i18n::t("check.behavior_title"));
    println!(
        "  {}",
        i18n::tf(
            "check.b_numlock",
            &[
                ("mode", cfg.numlock.as_str()),
                ("frames", &cfg.numlock_frames().to_string()),
                ("ms", &(cfg.numlock_frames() * cfg.frame_ms).to_string()),
            ]
        )
    );
    println!(
        "  {}",
        i18n::tf("check.b_mixed", &[("mode", cfg.mixed_mode.as_str())])
    );
    println!(
        "  {}",
        i18n::tf(
            "check.b_calc",
            &[
                (
                    "state",
                    &i18n::t(
                        if cfg.calc_gesture && cfg.calc_gesture_mode == CalcMode::Long {
                            "common.enabled"
                        } else {
                            "common.disabled"
                        }
                    ),
                ),
                ("frames", &cfg.numadv_trigger_frames.to_string()),
                (
                    "ms",
                    &(cfg.numadv_trigger_frames * cfg.frame_ms).to_string()
                ),
            ]
        )
    );
    println!(
        "  {}",
        i18n::tf(
            "check.b_grab",
            &[(
                "state",
                &i18n::t(if cfg.grab {
                    "check.b_grab_yes"
                } else {
                    "check.b_grab_no"
                }),
            )]
        )
    );
    println!(
        "  {}",
        i18n::tf(
            "check.b_idle",
            &[("sec", &(cfg.max_work_time_ms / 1000).to_string())]
        )
    );
    let auto = i18n::t("common.auto_detect");
    let led_where = if cfg.led_kind == "hidraw" {
        cfg.hidraw.clone().unwrap_or(auto)
    } else {
        match (cfg.i2c_bus, cfg.i2c_addr) {
            (Some(b), Some(a)) => format!("bus={b} addr=0x{a:02x}"),
            _ => auto,
        }
    };
    println!(
        "  {}",
        i18n::tf(
            "check.b_led",
            &[("kind", &cfg.led_kind), ("where", &led_where)]
        )
    );

    // 跨机型自检：列出全部背光候选节点，便于在陌生机型上一眼定位该选谁。
    #[cfg(target_os = "linux")]
    {
        let cands = numdeck::linux::probe_candidates();
        if !cands.is_empty() {
            println!("{}", i18n::t("check.led_probe_title"));
            for c in &cands {
                let state = if c.rank == u8::MAX {
                    i18n::t("check.led_probe_skip")
                } else if c.is_selected(&cands) {
                    i18n::t("check.led_probe_selected")
                } else {
                    i18n::t("check.led_probe_idle")
                };
                let feature = match c.probe {
                    numdeck::linux::FeatureProbe::Has => i18n::t("common.yes"),
                    numdeck::linux::FeatureProbe::Missing => i18n::t("common.no"),
                    numdeck::linux::FeatureProbe::NoAccess => i18n::t("check.led_probe_noperm"),
                    numdeck::linux::FeatureProbe::Unknown => i18n::t("check.led_probe_unknown"),
                };
                println!(
                    "  {}",
                    i18n::tf(
                        "check.led_probe_line",
                        &[
                            ("dev", &c.dev),
                            ("vid", &format!("{:04x}", c.vid)),
                            ("pid", &format!("{:04x}", c.pid)),
                            ("feature", &feature),
                            ("rank", &c.rank.to_string()),
                            ("state", &state),
                        ]
                    )
                );
            }
            if cands
                .iter()
                .any(|c| c.probe == numdeck::linux::FeatureProbe::NoAccess)
            {
                println!("{}", i18n::t("check.led_probe_noperm_hint"));
            }
            match numdeck::linux::probe_i2c() {
                Some((bus, addr)) => println!(
                    "{}",
                    i18n::tf(
                        "check.led_probe_i2c",
                        &[("bus", &bus.to_string()), ("addr", &format!("{addr:02x}"))]
                    )
                ),
                None => println!("{}", i18n::t("check.led_probe_i2c_none")),
            }
        }
    }

    println!(
        "  {}",
        i18n::tf("check.b_bright", &[("text", &brightness_report(cfg))])
    );
    println!(
        "  {}",
        i18n::tf(
            "check.b_bright_cmd",
            &[("steps", &cfg.manual_steps().to_string())]
        )
    );
    let cur = i18n::current();
    println!(
        "  {}",
        i18n::tf(
            "check.lang_line",
            &[("name", cur.native_name()), ("code", cur.code())]
        )
    );
    println!(
        "  {}",
        i18n::tf("check.lang_source", &[("source", &lang_source_text())])
    );
    match i18n::current_source() {
        Some(p) => println!(
            "  {}",
            i18n::tf("check.lang_file", &[("path", &p.display().to_string())])
        ),
        None => println!("  {}", i18n::t("lang.file_builtin")),
    }
    // 扩展开关：真机排障时最先要确认的就是"我配的东西到底读进去了没有"。
    println!();
    println!("{}", i18n::t("check.ext_title"));
    let custom_all = numdeck::custom::list_custom();
    if custom_all.is_empty() {
        println!("  {}", i18n::t("check.ext_layouts_none"));
    } else {
        let names: Vec<&str> = custom_all.iter().map(|(n, _, _)| n.as_str()).collect();
        println!(
            "  {}",
            i18n::tf(
                "check.ext_layouts",
                &[
                    ("n", &custom_all.len().to_string()),
                    ("names", &names.join(", ")),
                ]
            )
        );
    }
    let proto_text = if cfg.led_proto.is_builtin() {
        i18n::t("check.ext_none")
    } else {
        i18n::tf(
            "check.ext_led_proto",
            &[
                (
                    "hex",
                    &cfg.led_proto
                        .template
                        .iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .join(" "),
                ),
                ("n", &cfg.led_proto.template.len().to_string()),
            ],
        )
    };
    println!("  {}", i18n::tf("check.ext_led", &[("proto", &proto_text)]));
    let press_text = if cfg.pressure_switch.enabled {
        i18n::tf(
            "check.ext_press_on",
            &[
                ("percent", &cfg.pressure_switch.percent.to_string()),
                ("frames", &cfg.pressure_switch.frames.to_string()),
                ("ms", &(cfg.pressure_switch.cooldown_ms as i64).to_string()),
            ],
        )
    } else {
        i18n::t("check.ext_none")
    };
    println!(
        "  {}",
        i18n::tf("check.ext_pressure", &[("text", &press_text)])
    );
    let (bind, _hot_notes) = numdeck::custom::HotkeyState::new(&cfg.hotkeys);
    let hot_text = if bind.is_empty() {
        i18n::t("check.ext_none")
    } else {
        bind.describe().join("; ")
    };
    println!(
        "  {}",
        i18n::tf("check.ext_hotkeys", &[("text", &hot_text)])
    );

    println!();
    println!("{}", i18n::t("check.tests"));
    println!("{}", i18n::t("check.conclusion"));
    Ok(0)
}

/// 把 `LangSource` 翻成一句可读的来源说明。
pub(crate) fn lang_source_text() -> String {
    let source = LANG_SOURCE.get().copied().unwrap_or(LangSource::Default);
    i18n::t(source.key())
}

/// 当前语言与来源的一行摘要。
pub(crate) fn lang_current_line() -> String {
    let cur = i18n::current();
    i18n::tf(
        "lang.current",
        &[
            ("name", cur.native_name()),
            ("code", cur.code()),
            ("source", &lang_source_text()),
        ],
    )
}
