//! 命令行入口：`run` / `check` / `dump` / `bright` / `lang` / `--dump-config`。
//!
//! 界面文案全部走 `i18n`（简中 / 繁中 / 英文），语言按
//! `ASUS_NUMPAD_LANG` > `--lang` > 配置 `language` > 系统 locale 的优先级确定。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use asus_numpad::config::{self, CalcMode, Config, MixedMode, NumLockMode, VAL_OFF};
use asus_numpad::i18n::{self, Lang, LangSource};
use asus_numpad::keycodes as kc;
use asus_numpad::layout::{Geometry, KeyMode, COLS, KEY_GRID, ROWS, UM_PER_LAYOUT_UNIT};
use asus_numpad::{banner, sku};
use clap::{Arg, ArgAction, ArgMatches, Command};

/// 本次生效语言的来源，供 `lang` 子命令交代清楚原因。
static LANG_SOURCE: OnceLock<LangSource> = OnceLock::new();

#[derive(Debug, Default)]
struct Opts {
    config: Option<String>,
    device: Option<String>,
    keyboard: Option<String>,
    led: Option<String>,
    i2c_bus: Option<i32>,
    i2c_addr: Option<String>,
    hidraw: Option<String>,
    i2c_transfer: Option<String>,
    key_mode: Option<String>,
    layout: Option<String>,
    /// 手动指定机型配置（`--sku`），只对本次生效、不写配置文件
    sku: Option<String>,
    allow_unverified: bool,
    numlock: Option<String>,
    mixed_mode: Option<String>,
    pressure_min: Option<i32>,
    no_follow_brightness: bool,
    grab: bool,
    no_grab: bool,
    dump_config: bool,
    verbose: bool,
    cmd: Cmd,
}

#[derive(Debug, Default)]
enum Cmd {
    #[default]
    Run,
    Check,
    Dump,
    Bright {
        value: Option<String>,
        mode: Option<String>,
    },
    /// 档位表：查看 / 按档指定任意支持亮度 / 恢复默认四档。
    Levels {
        step: Option<String>,
        value: Option<String>,
    },
    /// 直发背光命令字（排障用）：`on` / `off` / 任意十六进制档位。
    Led {
        value: Option<String>,
    },
    Lang {
        code: Option<String>,
    },
    Layout {
        name: Option<String>,
        ids: bool,
    },
    /// 机型配置：查看识别结果 / 列出可选机型 / 手动指定。
    Sku {
        id: Option<String>,
    },
}

/// 预先在原始参数里找某个全局选项的值（用于在 clap 解析前确定语言）。
fn raw_arg(argv: &[String], name: &str) -> Option<String> {
    let mut it = argv.iter().skip(1);
    let prefix = format!("{name}=");
    while let Some(a) = it.next() {
        if let Some(v) = a.strip_prefix(&prefix) {
            return Some(v.to_string());
        }
        if a == name {
            return it.next().cloned();
        }
    }
    None
}

/// 命令行骨架：所有说明文字在运行时按当前语言取。
fn build_cli() -> Command {
    let t = i18n::t;
    Command::new("asus-numpad")
        .version(env!("CARGO_PKG_VERSION"))
        .about(t("cli.about"))
        .arg(
            Arg::new("config")
                .long("config")
                .global(true)
                .value_name("PATH")
                .help(t("cli.config.help")),
        )
        .arg(
            Arg::new("lang")
                .long("lang")
                .global(true)
                .value_name("CODE")
                .help(t("cli.lang.help")),
        )
        .arg(
            Arg::new("device")
                .long("device")
                .global(true)
                .value_name("DEV")
                .help(t("cli.device.help")),
        )
        .arg(
            Arg::new("keyboard")
                .long("keyboard")
                .global(true)
                .value_name("DEV")
                .help(t("cli.keyboard.help")),
        )
        .arg(
            Arg::new("led")
                .long("led")
                .global(true)
                .value_name("KIND")
                .help(t("cli.led.help")),
        )
        .arg(
            Arg::new("i2c_bus")
                .long("i2c-bus")
                .global(true)
                .value_name("NUM")
                .value_parser(clap::value_parser!(i32))
                .help(t("cli.i2c_bus.help")),
        )
        .arg(
            Arg::new("i2c_addr")
                .long("i2c-addr")
                .global(true)
                .value_name("ADDR")
                .help(t("cli.i2c_addr.help")),
        )
        .arg(
            Arg::new("hidraw")
                .long("hidraw")
                .global(true)
                .value_name("PATH")
                .help(t("cli.hidraw.help")),
        )
        .arg(
            Arg::new("i2c_transfer")
                .long("i2c-transfer")
                .global(true)
                .value_name("CMD")
                .help(t("cli.i2c_transfer.help")),
        )
        .arg(
            Arg::new("key_mode")
                .long("key-mode")
                .global(true)
                .value_name("MODE")
                .help(t("cli.key_mode.help")),
        )
        .arg(
            Arg::new("layout")
                .long("layout")
                .global(true)
                .value_name("NAME")
                .help(t("cli.layout.help")),
        )
        .arg(
            Arg::new("numlock")
                .long("numlock")
                .global(true)
                .value_name("MODE")
                .help(t("cli.numlock.help")),
        )
        .arg(
            Arg::new("mixed_mode")
                .long("mixed-mode")
                .global(true)
                .value_name("MODE")
                .help(t("cli.mixed_mode.help")),
        )
        .arg(
            Arg::new("pressure_min")
                .long("pressure-min")
                .global(true)
                .value_name("PRESSURE")
                .value_parser(clap::value_parser!(i32))
                .help(t("cli.pressure_min.help")),
        )
        .arg(
            Arg::new("no_follow_brightness")
                .long("no-follow-brightness")
                .global(true)
                .action(ArgAction::SetTrue)
                .help(t("cli.no_follow_brightness.help")),
        )
        .arg(
            Arg::new("grab")
                .long("grab")
                .global(true)
                .action(ArgAction::SetTrue)
                .help(t("cli.grab.help")),
        )
        .arg(
            Arg::new("no_grab")
                .long("no-grab")
                .global(true)
                .action(ArgAction::SetTrue)
                .conflicts_with("grab")
                .help(t("cli.no_grab.help")),
        )
        .arg(
            Arg::new("dump_config")
                .long("dump-config")
                .global(true)
                .action(ArgAction::SetTrue)
                .help(t("cli.dump_config.help")),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .global(true)
                .action(ArgAction::SetTrue)
                .help(t("cli.verbose.help")),
        )
        .subcommand(Command::new("run").about(t("cli.cmd.run")))
        .subcommand(Command::new("check").about(t("cli.cmd.check")))
        .subcommand(Command::new("dump").about(t("cli.cmd.dump")))
        .subcommand(
            Command::new("bright")
                .about(t("cli.cmd.bright"))
                .long_about(t("cli.cmd.bright.long"))
                .arg(
                    Arg::new("value")
                        .value_name("LEVEL")
                        .help(t("cli.cmd.bright.value")),
                )
                .arg(
                    Arg::new("mode")
                        .value_name("MODE")
                        .help(t("cli.cmd.bright.mode")),
                ),
        )
        .subcommand(
            Command::new("levels")
                .about(t("cli.cmd.levels"))
                .long_about(t("cli.cmd.levels.long"))
                .arg(
                    Arg::new("step")
                        .value_name("STEP")
                        .help(t("cli.cmd.levels.step")),
                )
                .arg(
                    Arg::new("value")
                        .value_name("VALUE")
                        .help(t("cli.cmd.levels.value")),
                ),
        )
        .subcommand(
            Command::new("led")
                .about(t("cli.cmd.led"))
                .long_about(t("cli.cmd.led.long"))
                .arg(
                    Arg::new("value")
                        .value_name("VALUE")
                        .help(t("cli.cmd.led.value")),
                ),
        )
        .arg(
            Arg::new("allow_unverified")
                .long("allow-unverified")
                .global(true)
                .action(ArgAction::SetTrue)
                .help(t("cli.allow_unverified.help")),
        )
        .arg(
            Arg::new("sku")
                .long("sku")
                .global(true)
                .value_name("ID")
                .help(t("cli.sku.help")),
        )
        .subcommand(
            Command::new("layout")
                .about(t("cli.cmd.layout"))
                .long_about(t("cli.cmd.layout.long"))
                .arg(
                    Arg::new("name")
                        .value_name("NAME")
                        .help(t("cli.cmd.layout.name")),
                )
                .arg(
                    Arg::new("ids")
                        .long("ids")
                        .action(ArgAction::SetTrue)
                        .help(t("cli.cmd.layout.ids")),
                ),
        )
        .subcommand(
            Command::new("sku")
                .about(t("cli.cmd.sku"))
                .long_about(t("cli.cmd.sku.long"))
                .arg(Arg::new("id").value_name("ID").help(t("cli.cmd.sku.id"))),
        )
        .subcommand(
            Command::new("lang")
                .about(t("cli.cmd.lang"))
                .long_about(t("cli.cmd.lang.long"))
                .arg(
                    Arg::new("code")
                        .value_name("CODE")
                        .help(t("cli.cmd.lang.code")),
                ),
        )
}

fn opts_from(matches: &ArgMatches) -> Opts {
    let mut opts = Opts {
        config: matches.get_one::<String>("config").cloned(),
        device: matches.get_one::<String>("device").cloned(),
        keyboard: matches.get_one::<String>("keyboard").cloned(),
        led: matches.get_one::<String>("led").cloned(),
        i2c_bus: matches.get_one::<i32>("i2c_bus").copied(),
        i2c_addr: matches.get_one::<String>("i2c_addr").cloned(),
        hidraw: matches.get_one::<String>("hidraw").cloned(),
        i2c_transfer: matches.get_one::<String>("i2c_transfer").cloned(),
        key_mode: matches.get_one::<String>("key_mode").cloned(),
        layout: matches.get_one::<String>("layout").cloned(),
        sku: matches.get_one::<String>("sku").cloned(),
        allow_unverified: matches.get_flag("allow_unverified"),
        numlock: matches.get_one::<String>("numlock").cloned(),
        mixed_mode: matches.get_one::<String>("mixed_mode").cloned(),
        pressure_min: matches.get_one::<i32>("pressure_min").copied(),
        no_follow_brightness: matches.get_flag("no_follow_brightness"),
        grab: matches.get_flag("grab"),
        no_grab: matches.get_flag("no_grab"),
        dump_config: matches.get_flag("dump_config"),
        verbose: matches.get_flag("verbose"),
        cmd: Cmd::Run,
    };
    opts.cmd = match matches.subcommand() {
        Some(("check", _)) => Cmd::Check,
        Some(("dump", _)) => Cmd::Dump,
        Some(("bright", sub)) => Cmd::Bright {
            value: sub.get_one::<String>("value").cloned(),
            mode: sub.get_one::<String>("mode").cloned(),
        },
        Some(("levels", sub)) => Cmd::Levels {
            step: sub.get_one::<String>("step").cloned(),
            value: sub.get_one::<String>("value").cloned(),
        },
        Some(("led", sub)) => Cmd::Led {
            value: sub.get_one::<String>("value").cloned(),
        },
        Some(("lang", sub)) => Cmd::Lang {
            code: sub.get_one::<String>("code").cloned(),
        },
        Some(("layout", sub)) => Cmd::Layout {
            name: sub.get_one::<String>("name").cloned(),
            ids: sub.get_flag("ids"),
        },
        Some(("sku", sub)) => Cmd::Sku {
            id: sub.get_one::<String>("id").cloned(),
        },
        _ => Cmd::Run,
    };
    opts
}

fn detect_led_kind() -> String {
    // 无显式指定时：有 hidraw 节点就用 hidraw，否则 i2c
    let has_hidraw = std::fs::read_dir("/dev")
        .map(|d| {
            d.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with("hidraw"))
        })
        .unwrap_or(false);
    if has_hidraw {
        "hidraw".to_string()
    } else {
        "i2c".to_string()
    }
}

fn build_config(cli: &Opts) -> (Config, Vec<String>, std::path::PathBuf) {
    // 未显式指定 --config 时，先把旧版平铺路径的配置迁到 /etc/asus-numpad/ 下
    let mut notes: Vec<String> = Vec::new();
    if cli.config.is_none() {
        if let Some(msg) = Config::migrate_legacy_config() {
            notes.push(msg);
        }
    }
    let path = cli
        .config
        .clone()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(Config::default_path);
    let (mut cfg, load_notes) = Config::load(&path);
    notes.extend(load_notes);

    if let Some(v) = &cli.key_mode {
        match v.parse::<KeyMode>() {
            Ok(m) => cfg.key_mode = m,
            Err(_) => notes.push(i18n::tf("note.invalid_key_mode", &[("v", v)])),
        }
    }
    if let Some(v) = &cli.layout {
        cfg.layout_name = v.clone();
    }
    // `--sku` 只覆盖本次运行；要长期生效用 `asus-numpad sku <id>` 写进配置。
    // 取值非法时由后面的 sanitize 丢弃并提示，不让手误把驱动卡在启动阶段。
    if let Some(v) = &cli.sku {
        cfg.sku = Some(v.clone());
    }
    if cli.allow_unverified {
        cfg.allow_unverified_layout = true;
    }
    if let Some(v) = &cli.numlock {
        cfg.numlock = match v.as_str() {
            "sync" => NumLockMode::Sync,
            "toggle" => NumLockMode::Toggle,
            "off" => NumLockMode::Off,
            _ => {
                notes.push(i18n::tf("note.invalid_numlock", &[("v", v)]));
                cfg.numlock
            }
        };
    }
    if let Some(v) = &cli.mixed_mode {
        cfg.mixed_mode = match v.as_str() {
            "auto" => MixedMode::Auto,
            "on" | "true" => MixedMode::On,
            "off" | "false" => MixedMode::Off,
            _ => {
                notes.push(i18n::tf("note.invalid_mixed_mode", &[("v", v)]));
                cfg.mixed_mode
            }
        };
    }
    if let Some(v) = cli.pressure_min {
        cfg.pressure_numpad_min = v.max(0);
    }
    if cli.no_follow_brightness {
        cfg.follow_keyboard_brightness = false;
    }
    if cli.grab {
        cfg.grab = true;
    }
    if cli.no_grab {
        cfg.grab = false;
    }
    if let Some(v) = &cli.device {
        cfg.device_override = Some(v.clone());
    }
    if let Some(v) = &cli.keyboard {
        cfg.keyboard_override = Some(v.clone());
    }
    if let Some(v) = &cli.led {
        cfg.led_kind = v.clone();
    } else {
        cfg.led_kind = detect_led_kind();
    }
    if let Some(v) = cli.i2c_bus {
        cfg.i2c_bus = Some(v);
    }
    if let Some(v) = &cli.i2c_addr {
        cfg.i2c_addr = parse_addr(v).or_else(|| {
            notes.push(i18n::tf("note.invalid_i2c_addr", &[("v", v)]));
            cfg.i2c_addr
        });
    }
    if let Some(v) = &cli.hidraw {
        cfg.hidraw = Some(v.clone());
    }
    if let Some(v) = &cli.i2c_transfer {
        cfg.i2c_transfer = v.clone();
    }
    notes.extend(cfg.sanitize());
    (cfg, notes, path)
}

fn parse_addr(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u8>().ok()
    }
}

fn render_keygrid(cfg: &Config) -> String {
    let mut out = String::new();
    for (r, row) in KEY_GRID.iter().enumerate() {
        let mut line = format!(
            "  {}: ",
            i18n::tf("check.keygrid_row", &[("r", &r.to_string())])
        );
        for (c, cell) in row.iter().enumerate() {
            let text = match cell {
                None => i18n::t("check.keygrid_numlock"),
                Some(cell) => asus_numpad::layout::key_for_cell(*cell, cfg.key_mode)
                    .map(|k| kc::name_of(k).to_string())
                    .unwrap_or_else(|| "?".into()),
            };
            let col = i18n::tf("check.keygrid_col", &[("c", &c.to_string())]);
            line.push_str(&format!("[{col} {text}] "));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn fallback_geometry(cfg: &Config) -> Geometry {
    // 无真机时用配置里的物理尺寸构造一个 0..1000 的虚拟坐标系
    let width_um = cfg.touchpad_width_mm * 1000.0;
    let height_um = cfg.touchpad_height_mm * 1000.0;
    let ux = 1000.0 / width_um;
    let uy = 1000.0 / height_um;
    Geometry::new(0, 1000, 0, 1000, ux, uy, cfg.layout()).with_layout_name(cfg.layout_entry().name)
}

fn cmd_check(cfg: &Config, notes: &[String]) -> Result<i32> {
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
            match asus_numpad::linux::probe_report(cfg) {
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
                    cfg.tap_threshold_mm,
                    cfg.tap_threshold_mm * 1000.0 * geometry.ux,
                    cfg.tap_threshold_mm * 1000.0 * geometry.uy,
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
        let cands = asus_numpad::linux::probe_candidates();
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
                    asus_numpad::linux::FeatureProbe::Has => i18n::t("common.yes"),
                    asus_numpad::linux::FeatureProbe::Missing => i18n::t("common.no"),
                    asus_numpad::linux::FeatureProbe::NoAccess => i18n::t("check.led_probe_noperm"),
                    asus_numpad::linux::FeatureProbe::Unknown => i18n::t("check.led_probe_unknown"),
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
                .any(|c| c.probe == asus_numpad::linux::FeatureProbe::NoAccess)
            {
                println!("{}", i18n::t("check.led_probe_noperm_hint"));
            }
            match asus_numpad::linux::probe_i2c() {
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
    println!();
    println!("{}", i18n::t("check.tests"));
    println!("{}", i18n::t("check.conclusion"));
    Ok(0)
}

/// 把 `LangSource` 翻成一句可读的来源说明。
fn lang_source_text() -> String {
    let source = LANG_SOURCE.get().copied().unwrap_or(LangSource::Default);
    i18n::t(source.key())
}

/// 当前语言与来源的一行摘要。
fn lang_current_line() -> String {
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

/// 键盘背光 → 数字区亮度档的当前换算结果（`check` / `run` 启动横幅用）。
fn brightness_report(cfg: &Config) -> String {
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
    let mut follower = asus_numpad::brightness::BrightnessFollower::new(
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
fn print_bright_status(cfg: &Config) {
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

    let mut follower = asus_numpad::brightness::BrightnessFollower::new(
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

/// 只改配置文件里指定的键，其余字段原样保留；写前留 `.bak` 备份。
fn persist_config_keys(path: &Path, updates: &[(&str, serde_json::Value)]) -> Result<PathBuf> {
    // 普通用户写自己的 XDG 配置（免 sudo），只有 root 才落 /etc——见 Config::writable_path
    let target = Config::writable_path(path);
    let path = target.as_path();
    let base = match std::fs::read_to_string(path) {
        Ok(text) => text,
        // 配置文件不存在：以默认配置为底再写入差异项
        Err(_) => Config::default().to_json(),
    };
    let mut value: serde_json::Value = serde_json::from_str(&base)
        .with_context(|| i18n::tf("persist.bad_json", &[("path", &path.display().to_string())]))?;
    if !value.is_object() {
        value = serde_json::json!({});
    }
    for (key, v) in updates {
        value[*key] = v.clone();
    }
    let mut text = serde_json::to_string_pretty(&value)?;
    text.push('\n');

    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).with_context(|| {
                i18n::tf(
                    "persist.mkdir_failed",
                    &[("dir", &dir.display().to_string())],
                )
            })?;
        }
    }
    let backup = PathBuf::from(format!("{}.bak", path.display()));
    if path.exists() {
        std::fs::copy(path, &backup).with_context(|| {
            i18n::tf(
                "persist.backup_failed",
                &[("path", &path.display().to_string())],
            )
        })?;
    }
    std::fs::write(path, text).with_context(|| {
        i18n::tf(
            "persist.write_failed",
            &[("path", &path.display().to_string())],
        )
    })?;
    Ok(backup)
}

/// 直接下发一个亮度字节到背光通道；返回是否成功。
fn apply_led_level(cfg: &Config, level: u8) -> bool {
    #[cfg(target_os = "linux")]
    {
        let mut led = asus_numpad::linux::led::build_led(cfg);
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

/// 若 asus-numpad 服务正在运行，就重启它让新配置立即生效；未运行时只提示，不主动拉起。
/// 当前是否在图形/登录会话里（`systemd --user` 可用）。
fn has_user_bus() -> bool {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|v| !v.as_os_str().is_empty())
        .unwrap_or(false)
}

/// 调 systemctl：会话内走用户级实例（免 sudo），否则退系统级（root 场景）。
fn systemctl(args: &[&str]) -> bool {
    let mut cmd = std::process::Command::new("systemctl");
    if has_user_bus() {
        cmd.arg("--user");
    }
    cmd.args(args);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

fn try_restart_service() {
    if !systemctl(&["is-active", "--quiet", "asus-numpad"]) {
        println!("{}", i18n::t("svc.not_running"));
        return;
    }
    if systemctl(&["restart", "asus-numpad"]) {
        println!("{}", i18n::t("svc.restarted"));
    } else {
        println!("{}", i18n::t("svc.restart_failed"));
    }
}

/// 落盘 + 立即下发 + 让服务按新档重跑（`bright 1|2|3`、`up|down|cycle` 共用）。
fn apply_step(cfg: &Config, cfg_path: &Path, step: u8) -> Result<i32> {
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

/// 解析一个亮度值：十进制（`65`）或十六进制（`0x41`）。
fn parse_level_value(raw: &str) -> Option<u8> {
    let t = raw.trim().to_ascii_lowercase();
    match t.strip_prefix("0x") {
        Some(hex) => u8::from_str_radix(hex, 16).ok(),
        None => t.parse::<u8>().ok(),
    }
}

/// 打印当前档位表（每档 → 实际下发的字节）。
fn print_levels_table(cfg: &Config) {
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
/// * `asus-numpad levels`            → 打印当前档表（要看得出每档到底发多少字节）
/// * `asus-numpad levels 2 0x46`     → 把第 2 档设成 0x46，其余档不动
/// * `asus-numpad levels 3 71`       → 十进制写法，等价 0x47
/// * `asus-numpad levels reset`      → 恢复默认四档 `[0, 0x43, 0x45, 0x48]`
///
/// 合法值只有硬件的 `0x00`（熄灭）与 `0x41..=0x48`（八档亮度），改完仍要求非递减，
/// 否则 `bright up` 会越按越黑。
fn cmd_levels(
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

/// `led`：绕过所有状态机，直接下发一个背光命令字。
///
/// 报「完全不亮」时必须先分清两件事：
/// * 驱动**没发**（服务没起来 / 通道不通 / 被跟随逻辑覆盖）——下发会报错，或根本没人发；
/// * 驱动发了但**硬件不认这个值**——`led on` 亮、`led 0x41` 不亮，就是档位字节的问题。
///
/// 所以留一条能直发 `0x01 / 0x00 / 0x41..0x48` 的通道，让现场一条命令就能定位：
/// `asus-numpad led on` → 亮，说明通道好的；`asus-numpad led 0x41` → 不亮，
/// 说明这台机器的 EC 只吃两态、不吃亮度档，那跟随亮度就得整体关掉。
fn cmd_led(cfg: &Config, value: Option<&str>) -> Result<i32> {
    let raw = value.map(|v| v.trim().to_ascii_lowercase());
    let level = match raw.as_deref() {
        None | Some("") | Some("status") | Some("help") => {
            println!("{}", i18n::t("led.cmd.usage"));
            return Ok(2);
        }
        Some("on") => 0x01,
        Some("off") => 0x00,
        Some(v) => {
            let parsed = match v.strip_prefix("0x") {
                Some(hex) => u8::from_str_radix(hex, 16).ok(),
                None => v.parse::<u8>().ok(),
            };
            match parsed {
                Some(b) => b,
                None => {
                    println!("{}", i18n::tf("led.cmd.bad_value", &[("v", v)]));
                    return Ok(2);
                }
            }
        }
    };
    let ok = apply_led_level(cfg, level);
    Ok(if ok { 0 } else { 1 })
}

/// `bright`：手动三档调光 + 是否跟随键盘背光。
fn cmd_bright(
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

/// `lang`：查看 / 切换界面语言。
fn cmd_lang(cfg_path: &Path, code: Option<&str>) -> Result<i32> {
    println!("{}", lang_current_line());
    let list = Lang::ALL
        .iter()
        .map(|l| format!("{}（{}）", l.code(), l.native_name()))
        .collect::<Vec<_>>()
        .join(&i18n::t("common.list_sep"));
    println!("{}", i18n::tf("lang.available", &[("list", &list)]));
    let dirs = i18n::search_dirs()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(&i18n::t("common.list_sep"));
    println!("{}", i18n::tf("lang.dirs", &[("dirs", &dirs)]));
    match i18n::current_source() {
        Some(p) => println!(
            "{}",
            i18n::tf("lang.file", &[("path", &p.display().to_string())])
        ),
        None => println!("  {}", i18n::t("lang.file_builtin")),
    }

    let Some(raw) = code else {
        println!();
        println!("{}", i18n::t("lang.usage"));
        return Ok(0);
    };
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("auto") {
        let backup = persist_config_keys(cfg_path, &[("language", serde_json::json!("auto"))])?;
        println!();
        println!(
            "{}",
            i18n::tf(
                "lang.wrote",
                &[
                    ("path", &cfg_path.display().to_string()),
                    ("value", "auto"),
                    ("backup", &backup.display().to_string()),
                ]
            )
        );
        println!(
            "{}",
            i18n::tf(
                "lang.effect_auto",
                &[("name", i18n::detect_system().native_name())]
            )
        );
        println!("{}", i18n::t("lang.restart_hint"));
        try_restart_service();
        return Ok(0);
    }
    match Lang::parse(raw) {
        Some(lang) => {
            let backup =
                persist_config_keys(cfg_path, &[("language", serde_json::json!(lang.code()))])?;
            println!();
            println!(
                "{}",
                i18n::tf(
                    "lang.wrote",
                    &[
                        ("path", &cfg_path.display().to_string()),
                        ("value", lang.code()),
                        ("backup", &backup.display().to_string()),
                    ]
                )
            );
            println!("{}", i18n::t("lang.restart_hint"));
            try_restart_service();
            Ok(0)
        }
        None => {
            println!();
            println!("{}", i18n::tf("lang.bad_code", &[("v", raw)]));
            println!("{}", i18n::t("lang.usage"));
            Ok(2)
        }
    }
}

/// `layout`：查看 / 固定本机布局。
///
/// 不带参数时列出全部官方布局与支持级别；`layout auto` 恢复按 HID 硬件 ID 自动识别；
/// `layout GH3XLNDZ` 写死某套布局；`layout --ids` 额外列出 HID 硬件 ID 对照表。
fn cmd_layout(cfg: &Config, cfg_path: &Path, name: Option<&str>, ids: bool) -> Result<i32> {
    use asus_numpad::layout::{self, LayoutSupport};

    println!(
        "{}",
        i18n::tf(
            "layout.current",
            &[
                ("layout", &cfg.layout_name),
                ("resolved", cfg.layout_entry().name),
                ("support", cfg.layout_entry().support.as_str()),
            ]
        )
    );
    println!();
    println!("{}", i18n::t("layout.list_title"));
    for e in layout::LAYOUTS {
        let mark = match e.support {
            LayoutSupport::Verified => "*",
            LayoutSupport::Family => "~",
            _ => "-",
        };
        println!(
            "  {mark} {:<12} {:<12} cols={} i0={} i4={} l0={} l3={} nl={}/{} edges={}/{}/{}/{}",
            e.name,
            e.support.as_str(),
            e.layout.columns,
            e.layout.index0,
            e.layout.index4,
            e.layout.level0,
            e.layout.level3,
            e.layout.numlock_index,
            e.layout.numlock_level,
            e.layout.left,
            e.layout.right,
            e.layout.up,
            e.layout.down
        );
    }
    println!();
    println!("{}", i18n::t("layout.legend"));

    if ids {
        println!();
        println!("{}", i18n::t("layout.ids_title"));
        for (hid, lay) in layout::hid_list() {
            println!("  {hid} -> {lay}");
        }
    }

    let Some(raw) = name else {
        println!();
        println!("{}", i18n::t("layout.usage"));
        return Ok(0);
    };
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("auto") {
        let backup = persist_config_keys(cfg_path, &[("layout_name", serde_json::json!("auto"))])?;
        println!();
        println!(
            "{}",
            i18n::tf(
                "layout.wrote",
                &[
                    ("path", &cfg_path.display().to_string()),
                    ("value", "auto"),
                    ("backup", &backup.display().to_string()),
                ]
            )
        );
        println!("{}", i18n::t("layout.effect_auto"));
        try_restart_service();
        return Ok(0);
    }
    match layout::by_name(raw) {
        Some(entry) => {
            let backup =
                persist_config_keys(cfg_path, &[("layout_name", serde_json::json!(entry.name))])?;
            println!();
            println!(
                "{}",
                i18n::tf(
                    "layout.wrote",
                    &[
                        ("path", &cfg_path.display().to_string()),
                        ("value", entry.name),
                        ("backup", &backup.display().to_string()),
                    ]
                )
            );
            if entry.support.needs_confirm() {
                println!(
                    "{}",
                    i18n::tf(
                        "layout.unverified_warn",
                        &[("layout", entry.name), ("support", entry.support.as_str()),]
                    )
                );
                println!("{}", i18n::t("err.layout_unverified.hint"));
            }
            try_restart_service();
            Ok(0)
        }
        None => {
            println!();
            println!("{}", i18n::tf("layout.bad_name", &[("v", raw)]));
            println!("{}", i18n::t("layout.usage"));
            Ok(2)
        }
    }
}

fn cmd_dump(cfg: &Config) -> Result<i32> {
    println!("{}", banner());
    println!("{}", i18n::t("dump.mode"));
    println!("{}", cfg.summary());

    #[cfg(target_os = "linux")]
    {
        asus_numpad::linux::dump_events(cfg)?;
        Ok(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        println!("{}", i18n::t("dump.nonlinux"));
        Ok(1)
    }
}

/// 机型闸门：这台机器根本跑不起来的，就在启动前说清楚。
///
/// 本包只跑机型表里的机器：DMI 型号与触摸板模组都认不出时不再兜底，直接拦下。
/// 返回 `Some(退出码)` 表示已拦下，`None` 表示放行（`2` = `--sku` 给的 id 无效，
/// `3` = 本机不在支持列表）。
#[cfg(target_os = "linux")]
fn sku_gate(cfg: &Config) -> Option<i32> {
    let identity = sku::dmi_identity();
    let hw = sku::probe_hardware();
    let ids = sku::known_ids().join(", ");
    match sku::resolve(&identity, &hw, cfg.sku.as_deref()) {
        Ok(_) => None,
        Err(sku::ResolveError::UnknownId(e)) => {
            println!("{}", banner());
            println!("{}", i18n::tf("sku.bad_id", &[("id", &e.0), ("ids", &ids)]));
            Some(2)
        }
        Err(sku::ResolveError::Unsupported(u)) => {
            println!("{}", banner());
            println!("{}", unsupported_text(&u, &ids));
            // 「已拒绝启动」只在真去启动时才说：unsupported_text 也被 check 复用，
            // 那里没有可拒绝的启动过程。
            println!("{}", i18n::t("err.unsupported_stop"));
            println!("{}", i18n::t("err.unsupported_hint"));
            Some(3)
        }
    }
}

/// 挡掉没有 NumberPad 硬件的机器：DMI 型号对得上也不放行。
///
/// 判据是触摸板 HID 接口里有没有 FEATURE 报告 `0x0d`。4 区 RGB 键盘的配置（国行
/// 「魔霸新锐」、同型号的减配档）用同一块触摸板模组，DMI 与模组都分不出来，但描述符里
/// 没有这条 FEATURE——数字区背光和图标区都不存在，启动起来只会白抓触摸板。
#[cfg(target_os = "linux")]
fn numpad_gate() -> Option<i32> {
    use asus_numpad::linux::{probe_numpad_presence, NumpadPresence};
    match probe_numpad_presence() {
        NumpadPresence::Absent => {
            println!("{}", banner());
            println!("{}", i18n::t("err.no_numpad"));
            println!("{}", i18n::t("err.no_numpad.hint"));
            Some(3)
        }
        NumpadPresence::NoAccess => {
            // 读不到描述符是权限问题，不能当成「没有这块硬件」把人挡回去
            eprintln!("{}", i18n::t("err.numpad_no_access"));
            None
        }
        NumpadPresence::Present | NumpadPresence::Unknown => None,
    }
}

/// 「这台不在支持列表」的两条提示：厂商明确不是 ASUS / 只是认不出机型。
fn unsupported_text(u: &sku::Unsupported, ids: &str) -> String {
    match &u.non_asus {
        Some(vendor) => i18n::tf("err.not_asus", &[("vendor", vendor)]),
        None => i18n::tf("err.unsupported_model", &[("ids", ids)]),
    }
}

/// 输出「本机用哪套机型配置 + 凭什么判定」。
fn sku_block(cfg: &Config) -> String {
    // DMI 与 hidraw 只有 Linux 上有：在别的系统上跑自检时如实说「判定不了」，
    // 不能拿「认不出」当「不支持」，那会把开发机误导成硬件不兼容。
    if !cfg!(target_os = "linux") {
        return i18n::t("sku.nonlinux");
    }
    let identity = sku::dmi_identity();
    let hw = sku::probe_hardware();
    let ids = sku::known_ids().join(", ");
    let mut out = match sku::resolve(&identity, &hw, cfg.sku.as_deref()) {
        Ok(r) => {
            let mut s = i18n::tf(
                "sku.model",
                &[
                    ("sku", &r.sku.localized_display()),
                    ("source", &i18n::t(r.source.i18n_key())),
                ],
            );
            s.push('\n');
            if let Some(detail) = &r.detail {
                s.push_str(&i18n::tf("sku.detail", &[("detail", detail)]));
                s.push('\n');
            }
            s.push_str(&i18n::tf("sku.note", &[("note", &r.sku.localized_note())]));
            s
        }
        Err(sku::ResolveError::UnknownId(e)) => {
            i18n::tf("sku.bad_id", &[("id", &e.0), ("ids", &ids)])
        }
        Err(sku::ResolveError::Unsupported(u)) => unsupported_text(&u, &ids),
    };
    out.push('\n');
    if identity.haystacks().is_empty() {
        out.push_str(&i18n::t("sku.dmi_unknown"));
    } else {
        out.push_str(&i18n::tf("sku.dmi", &[("dmi", &identity.label())]));
    }
    out.push('\n');
    if !hw.is_empty() {
        out.push_str(&i18n::tf("sku.hw", &[("hw", &hw.label())]));
        out.push('\n');
    }
    // 机型兼容情况只维护在官网，程序内不再写死「哪台机器上过真机」
    out.push_str(&i18n::t("sku.compat"));
    out.push('\n');
    out
}

/// `sku` 子命令：查看自动识别结果 / 列出可选机型 / 手动指定或恢复自动识别。
fn cmd_sku(cfg: &Config, cfg_path: &Path, arg: Option<&str>) -> Result<i32> {
    let ids = sku::known_ids().join(", ");
    match arg.map(str::trim).filter(|a| !a.is_empty()) {
        None => {
            println!("{}", sku_block(cfg));
            Ok(0)
        }
        Some(a) if a.eq_ignore_ascii_case("list") => {
            println!("{}", i18n::tf("sku.list", &[("ids", &ids)]));
            for entry in sku::SKUS {
                println!("  {:<10} {}", entry.id, entry.display);
            }
            Ok(0)
        }
        Some(a) if a.eq_ignore_ascii_case("auto") => {
            let target = Config::writable_path(cfg_path);
            persist_config_keys(cfg_path, &[("sku", serde_json::Value::Null)])?;
            println!(
                "{}",
                i18n::tf("sku.clear", &[("path", &target.display().to_string())])
            );
            Ok(0)
        }
        Some(a) => match sku::by_id(a) {
            Some(entry) => {
                let target = Config::writable_path(cfg_path);
                persist_config_keys(cfg_path, &[("sku", serde_json::json!(entry.id))])?;
                println!(
                    "{}",
                    i18n::tf(
                        "sku.set",
                        &[
                            ("sku", entry.display),
                            ("path", &target.display().to_string()),
                        ]
                    )
                );
                Ok(0)
            }
            None => {
                eprintln!("{}", i18n::tf("sku.bad_id", &[("id", a), ("ids", &ids)]));
                Ok(2)
            }
        },
    }
}

fn cmd_run(cfg: Config, verbose: bool) -> Result<i32> {
    // 两道闸门只对 Linux 有意义：非 Linux 上读不到 DMI 与 hidraw，跑闸门只会
    // 得到「不在支持列表」，把「这个平台跑不了」误报成「这台机器不支持」。
    #[cfg(target_os = "linux")]
    {
        if let Some(code) = sku_gate(&cfg) {
            return Ok(code);
        }
        if let Some(code) = numpad_gate() {
            return Ok(code);
        }
    }
    #[cfg(target_os = "linux")]
    {
        println!("{}", banner());
        println!("{}", cfg.summary());
        println!(
            "{}",
            i18n::tf("run.brightness", &[("text", &brightness_report(&cfg))])
        );
        asus_numpad::linux::run_service(cfg, verbose)?;
        Ok(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (cfg, verbose);
        println!("{}", banner());
        println!("{}", i18n::t("run.nonlinux"));
        println!("{}", i18n::t("run.nonlinux_hint"));
        Ok(1)
    }
}

fn main() -> Result<()> {
    // 语言必须在任何输出之前定下来：先扫原始参数与环境变量，再读配置
    let argv: Vec<String> = std::env::args().collect();
    let cli_lang = raw_arg(&argv, "--lang");
    let pre_path = raw_arg(&argv, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(Config::default_path);
    let (pre_cfg, _) = Config::load(&pre_path);
    let (lang, source) = i18n::resolve(&pre_cfg.language, cli_lang.as_deref());
    i18n::init(lang);
    let _ = LANG_SOURCE.set(source);

    if let Some(raw) = &cli_lang {
        if !raw.eq_ignore_ascii_case("auto") && Lang::parse(raw).is_none() {
            eprintln!("{}", i18n::tf("lang.bad_code", &[("v", raw)]));
        }
    }

    let matches = build_cli().get_matches();
    let opts = opts_from(&matches);
    let (cfg, notes, cfg_path) = build_config(&opts);

    if opts.dump_config {
        println!("{}", cfg.to_json());
        return Ok(());
    }

    let code = match opts.cmd {
        Cmd::Check => cmd_check(&cfg, &notes)?,
        Cmd::Dump => cmd_dump(&cfg)?,
        Cmd::Bright { value, mode } => {
            cmd_bright(&cfg, &cfg_path, value.as_deref(), mode.as_deref())?
        }
        Cmd::Levels { step, value } => {
            cmd_levels(&cfg, &cfg_path, step.as_deref(), value.as_deref())?
        }
        Cmd::Led { value } => cmd_led(&cfg, value.as_deref())?,
        Cmd::Lang { code } => cmd_lang(&cfg_path, code.as_deref())?,
        Cmd::Layout { name, ids } => cmd_layout(&cfg, &cfg_path, name.as_deref(), ids)?,
        Cmd::Sku { id } => cmd_sku(&cfg, &cfg_path, id.as_deref())?,
        Cmd::Run => cmd_run(cfg, opts.verbose)?,
    };
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
