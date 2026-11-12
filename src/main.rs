//! 命令行入口：`run` / `check` / `dump` / `bright` / `lang` / `--dump-config`。
//!
//! 界面文案全部走 `i18n`（简中 / 繁中 / 英文），语言按
//! `NUMDECK_LANG` > `--lang` > 配置 `language` > 系统 locale 的优先级确定。

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Result;
use numdeck::config::Config;
use numdeck::i18n::{self, Lang, LangSource};

mod cli;

use cli::*;

/// 本次生效语言的来源，供 `lang` 子命令交代清楚原因。
pub(crate) static LANG_SOURCE: OnceLock<LangSource> = OnceLock::new();

#[derive(Debug, Default)]
pub(crate) struct Opts {
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
    no_follow_brightness: bool,
    grab: bool,
    no_grab: bool,
    dump_config: bool,
    verbose: bool,
    cmd: Cmd,
}

#[derive(Debug, Default)]
pub(crate) enum Cmd {
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
    /// 手动校准：采四角与两格中心，反解面板范围与网格矩形。
    Calibrate {
        samples: Option<String>,
        write: bool,
        cols: Option<usize>,
        rows: Option<usize>,
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

/// 恢复 SIGPIPE 的系统默认处理（进程收到就退出，不 panic）。
///
/// 自己声明而不引 `libc`：SIGPIPE=13、SIG_DFL=0 在 Linux / macOS / BSD 上一致，
/// 而 `libc` 只挂在 lib target 的依赖里，bin 这边够不到。
#[cfg(unix)]
fn reset_sigpipe() {
    extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }
    const SIGPIPE: i32 = 13;
    const SIG_DFL: usize = 0;
    // SAFETY: `signal(2)` 只登记处置方式，不触碰任何 Rust 侧内存。
    unsafe {
        signal(SIGPIPE, SIG_DFL);
    }
}

// --------------------------------------------------------------------------- //
// calibrate：非原生机型的手动校准
// --------------------------------------------------------------------------- //

fn main() -> Result<()> {
    // 管道被 `head` / `grep -q` 提前关掉时，Rust 默认忽略 SIGPIPE，后续 println!
    // 会拿到 EPIPE 并 panic；release 是 panic=abort，用户看到的是
    // `Abort trap: 6` 而不是安静退出。CLI 恢复系统默认行为即可。
    #[cfg(unix)]
    reset_sigpipe();

    // 更名前用户脚本里写的是 ASUS_NUMPAD_*，先继承过来再看后面的读取点
    numdeck::compat::adopt_legacy_env();

    // 语言必须在任何输出之前定下来：先扫原始参数与环境变量，再读配置
    let argv: Vec<String> = std::env::args().collect();
    let cli_lang = raw_arg(&argv, "--lang");
    let pre_path = raw_arg(&argv, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(Config::default_path);
    // 这次 load 只为拿到配置里写的语言，结果丢掉。它会把自定义布局注册进全局表、
    // 并产出一批 notes；build_config 又完整 load 一遍，两批 notes 一拼就成了
    // 「同一句提示印两遍」。所以这里连 notes 一起丢弃，改由 build_config 统一交代。
    let (pre_cfg, _pre_notes) = Config::load(&pre_path);
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
        Cmd::Calibrate {
            samples,
            write,
            cols,
            rows,
        } => cmd_calibrate(&cfg, &cfg_path, samples.as_deref(), write, cols, rows)?,
        Cmd::Run => cmd_run(cfg, opts.verbose)?,
    };
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}
