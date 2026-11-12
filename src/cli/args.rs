//! 命令行骨架：clap 定义、参数到 `Opts` 的映射、以及把两者合成生效配置。

/// `--sku` 与 `numdeck sku` 的帮助文案键。
///
/// ARM64 构建里没有机型表，帮助文本不能还举 `sku g614` 这种例子——那是 x86_64 才有的东西。
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const SKU_HELP_KEY: &str = "cli.sku.help.arm";
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
const SKU_HELP_KEY: &str = "cli.sku.help";
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const SKU_LONG_KEY: &str = "cli.cmd.sku.long.arm";
#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
const SKU_LONG_KEY: &str = "cli.cmd.sku.long";

use clap::{Arg, ArgAction, ArgMatches, Command};
use numdeck::config::{Config, MixedMode, NumLockMode};
use numdeck::i18n::{self};
use numdeck::layout::KeyMode;

use super::util::parse_addr;
use crate::{Cmd, Opts};

pub(crate) fn detect_led_kind() -> String {
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

pub(crate) fn build_config(cli: &Opts) -> (Config, Vec<String>, std::path::PathBuf) {
    // 未显式指定 --config 时，先把旧版平铺路径的配置迁到 /etc/numdeck/ 下
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
    // `--sku` 只覆盖本次运行；要长期生效用 `numdeck sku <id>` 写进配置。
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
    // 这里是对「CLI 覆盖后的配置」再复核一遍，因此配置本身的问题会被第二次报到
    // notes 里；不去重的话 `check` 会把同一句提示印两遍（第一遍来自 load）。
    notes.extend(cfg.sanitize());
    Config::dedup_notes(&mut notes);
    (cfg, notes, path)
}

/// 命令行骨架：所有说明文字在运行时按当前语言取。
pub(crate) fn build_cli() -> Command {
    let t = i18n::t;
    Command::new("numdeck")
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
                .help(t(SKU_HELP_KEY)),
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
                .long_about(t(SKU_LONG_KEY))
                .arg(Arg::new("id").value_name("ID").help(t("cli.cmd.sku.id"))),
        )
        .subcommand(
            Command::new("calibrate")
                .about(t("cli.cmd.calibrate"))
                .long_about(t("cli.cmd.calibrate.long"))
                .arg(
                    Arg::new("samples")
                        .long("samples")
                        .value_name("FILE")
                        .help(t("cli.cmd.calibrate.samples")),
                )
                .arg(
                    Arg::new("write")
                        .long("write")
                        .action(ArgAction::SetTrue)
                        .help(t("cli.cmd.calibrate.write")),
                )
                .arg(
                    Arg::new("cols")
                        .long("cols")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize))
                        .help(t("cli.cmd.calibrate.cols")),
                )
                .arg(
                    Arg::new("rows")
                        .long("rows")
                        .value_name("N")
                        .value_parser(clap::value_parser!(usize))
                        .help(t("cli.cmd.calibrate.rows")),
                ),
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

pub(crate) fn opts_from(matches: &ArgMatches) -> Opts {
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
        Some(("calibrate", sub)) => Cmd::Calibrate {
            samples: sub.get_one::<String>("samples").cloned(),
            write: sub.get_flag("write"),
            cols: sub.get_one::<usize>("cols").copied(),
            rows: sub.get_one::<usize>("rows").copied(),
        },
        _ => Cmd::Run,
    };
    opts
}
