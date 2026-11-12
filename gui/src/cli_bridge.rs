//! 命令行工具桥接层。
//!
//! 界面上那张命令面板并不重新实现 `check` / `dump` / `bright` / `levels` / `led` /
//! `layout` / `sku` / `lang` 的行为，而是按表单拼出参数、调用仓库里那个 `numdeck`
//! 可执行文件，再把输出原样贴回来。理由是这些子命令会读 DMI、写配置文件、按语言文件
//! 输出文案，重写一遍必然与 CLI 漂移——界面上看到的和终端里敲出来的必须是同一件事。
//!
//! 可执行文件的查找顺序：`NUMDECK_BIN` → 与本程序同目录 → `PATH` → 几个常见安装
//! 位置。找不到时界面会说明，不会静默失败。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 一次调用的结果。`code` 为 `None` 表示进程没能启动或被信号终止。
pub struct Completion {
    pub argv: Vec<String>,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
    /// 超时被主动终止。`dump` 这类不会自己退出的子命令用得上。
    pub timed_out: bool,
    /// 输出超过上限被截断。`dump` 在转速高的时候一秒能刷几千行，界面渲染几兆文本
    /// 会直接卡住，所以采集端就设了闸；这里只说一声，别让用户以为输出就这么点。
    pub truncated: bool,
}

impl Completion {
    pub fn ok(&self) -> bool {
        self.error.is_none() && self.code == Some(0)
    }

    /// 汇总成一段可直接读的文本，供日志与输出框使用。
    pub fn summary(&self) -> String {
        let mut text = format!("$ {}", self.argv.join(" "));
        if let Some(err) = &self.error {
            text.push_str(&format!("\n[启动失败] {err}"));
            return text;
        }
        if self.timed_out {
            text.push_str("\n[超时终止] 已按上限中止，下面是收到的那部分输出");
        }
        if self.truncated {
            text.push_str(
                "\n[输出已截断] 超过 512 KiB 的部分没有保留——「复制」拿到的也是截断后的内容",
            );
        }
        text.push_str(&format!(
            "\n[退出码] {}",
            self.code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "被信号终止".to_string())
        ));
        if !self.stdout.trim().is_empty() {
            text.push_str("\n[stdout]\n");
            text.push_str(self.stdout.trim_end());
        }
        if !self.stderr.trim().is_empty() {
            text.push_str("\n[stderr]\n");
            text.push_str(self.stderr.trim_end());
        }
        text
    }
}

/// 定位 `numdeck` 可执行文件。
pub fn locate() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("NUMDECK_BIN") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Some(path);
        }
    }

    let name = format!("numdeck{}", std::env::consts::EXE_SUFFIX);

    // 同目录：源码树里 cargo build 之后两个二进制挨着，或安装包装在同一前缀下
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sibling = dir.join(&name);
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(&name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let fallbacks = [
        "/usr/bin/numdeck",
        "/usr/local/bin/numdeck",
        "/bin/numdeck",
        "/run/current-system/sw/bin/numdeck", // NixOS
    ];
    for path in fallbacks {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(home) = std::env::var_os("HOME") {
        let path = Path::new(&home).join(".cargo/bin").join(&name);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

/// 同步执行，调用方负责放进后台线程。
///
/// `dump` 会一直打印事件直到 Ctrl+C，`run` 是常驻引擎——这类子命令若不给上限，命令
/// 面板会永远停在「执行中」。到时杀掉进程，把已经捕获的输出原样交回界面。
///
/// 读输出这件事没有用 `wait_with_output`：它要等到管道 EOF 才返回，而 EOF 取决于
/// 「所有持有写端的进程」都退出。CLI 若哪天 fork 出子进程（或用户换了包装脚本），
/// 杀了主进程也仍会被继承下去的那个写端拖住，超时就成了摆设。这里的做法是杀完之后
/// 只再给一段很短的宽限期收残余输出，收不到就带着已有内容返回。
pub fn run(bin: &Path, args: Vec<String>, timeout: Option<Duration>) -> Completion {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push(bin.display().to_string());
    argv.extend(args.iter().cloned());

    let mut child = match Command::new(bin)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return Completion {
                argv,
                code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("{}：{err}", bin.display())),
                timed_out: false,
                truncated: false,
            };
        }
    };

    let mut timed_out = false;
    if let Some(limit) = timeout {
        let deadline = Instant::now() + limit;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(err) => {
                    let _ = child.kill();
                    return Completion {
                        argv,
                        code: None,
                        stdout: String::new(),
                        stderr: String::new(),
                        error: Some(format!("等待进程时出错：{err}")),
                        timed_out: false,
                        truncated: false,
                    };
                }
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                timed_out = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    // 正常退出的进程管道会立即到 EOF，不需要宽限期；被我们杀掉的只等一下残余输出
    let grace = if timed_out {
        Some(Duration::from_millis(300))
    } else {
        None
    };
    let drained = drain(&mut child, grace);

    Completion {
        argv,
        code: drained.code,
        stdout: drained.stdout,
        stderr: drained.stderr,
        error: None,
        timed_out,
        truncated: drained.truncated,
    }
}

struct Drained {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    truncated: bool,
}

/// 采集上限（每条管道）。512 KiB 足够看懂 `doctor` / `dump` 的前因后果，再多只是
/// 把界面拖垮：egui 每帧都要对整段文本重新排版，几兆 monospace 文本就是几百毫秒一帧。
const MAX_CAPTURE: usize = 512 * 1024;

/// 边读输出边等进程退出。
///
/// 两个读线程是必须的：子进程写满管道（64 KiB）就会阻塞，若先 `wait()` 再读，输出量
/// 大的子命令会互相等死。`grace` 为 `Some` 时只收这么久，剩下的交给已经脱离的读线
/// 程自己消化——它们不会阻塞界面。
fn drain(child: &mut std::process::Child, grace: Option<Duration>) -> Drained {
    let (tx, rx) = std::sync::mpsc::channel::<(&'static str, Vec<u8>)>();
    spawn_reader("out", child.stdout.take(), tx.clone());
    spawn_reader("err", child.stderr.take(), tx.clone());
    drop(tx);

    let code = child.wait().ok().and_then(|status| status.code());

    let mut out_buf = Vec::new();
    let mut err_buf = Vec::new();
    let mut truncated = false;
    let mut accept = |(tag, chunk): (&'static str, Vec<u8>)| {
        let buf = if tag == "out" {
            &mut out_buf
        } else {
            &mut err_buf
        };
        // 仍然把管道读空（不然后面子进程会写阻塞），只是不再往缓冲区里堆
        let room = MAX_CAPTURE.saturating_sub(buf.len());
        if chunk.len() > room {
            buf.extend_from_slice(&chunk[..room]);
            truncated = true;
        } else {
            buf.extend_from_slice(&chunk);
        }
    };
    match grace {
        None => {
            for item in rx.iter() {
                accept(item);
            }
        }
        Some(limit) => {
            let deadline = Instant::now() + limit;
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                match rx.recv_timeout(left) {
                    Ok(item) => accept(item),
                    Err(_) => break,
                }
            }
        }
    }
    drop(accept);

    Drained {
        code,
        stdout: String::from_utf8_lossy(&out_buf).to_string(),
        stderr: String::from_utf8_lossy(&err_buf).to_string(),
        truncated,
    }
}

/// 单条管道的读线程：**读到一块就送一块**，而不是攒到 EOF 再送。
///
/// 攒到 EOF 的话，一旦有孙进程继承了写端，超时后 300 ms 内主线程什么也收不到，前面
/// 已经打印的内容就白丢了。分块送出去，界面总能拿到被杀之前那部分。
/// `grace` 用尽后主线程不再等它，线程自行收尾即可——既不会拖住界面，也不会写坏别的。
fn spawn_reader<R: Read + Send + 'static>(
    tag: &'static str,
    handle: Option<R>,
    tx: std::sync::mpsc::Sender<(&'static str, Vec<u8>)>,
) {
    let Some(mut handle) = handle else {
        return;
    };
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match handle.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send((tag, buf[..n].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_includes_exit_code_and_streams() {
        let done = Completion {
            argv: vec!["numdeck".to_string(), "check".to_string()],
            code: Some(0),
            stdout: "ok\n".to_string(),
            stderr: String::new(),
            error: None,
            timed_out: false,
            truncated: false,
        };
        let text = done.summary();
        assert!(text.starts_with("$ numdeck check"));
        assert!(text.contains("[退出码] 0"));
        assert!(text.contains("ok"));
        assert!(done.ok());
    }

    #[test]
    fn summary_reports_spawn_failure() {
        let done = Completion {
            argv: vec!["numdeck".to_string()],
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: Some("没有那个文件".to_string()),
            timed_out: false,
            truncated: false,
        };
        assert!(done.summary().contains("[启动失败] 没有那个文件"));
        assert!(!done.ok());
    }

    #[test]
    fn summary_marks_signal_termination() {
        let done = Completion {
            argv: vec!["numdeck".to_string(), "run".to_string()],
            code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: None,
            timed_out: false,
            truncated: false,
        };
        assert!(done.summary().contains("被信号终止"));
    }

    #[test]
    fn summary_mentions_timeout() {
        let done = Completion {
            argv: vec!["numdeck".to_string(), "dump".to_string()],
            code: None,
            stdout: "event 0x41\n".to_string(),
            stderr: String::new(),
            error: None,
            timed_out: true,
            truncated: false,
        };
        let text = done.summary();
        assert!(text.contains("[超时终止]"));
        assert!(text.contains("event 0x41"));
        assert!(!done.ok());
    }

    /// `dump` 会一直跑，超时后必须真的收工，并把已捕获的输出交回来。
    #[cfg(unix)]
    #[test]
    fn timeout_kills_a_long_running_child() {
        let started = Instant::now();
        let done = run(
            Path::new("/bin/sh"),
            vec!["-c".to_string(), "echo started; sleep 10".to_string()],
            Some(Duration::from_millis(300)),
        );
        assert!(done.timed_out, "应当标记为超时终止");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "不该真的等满 10 秒"
        );
        assert!(done.stdout.contains("started"), "超时前的输出要留着");
        assert!(!done.ok());
    }

    #[cfg(unix)]
    #[test]
    fn normal_exit_is_not_flagged_as_timeout() {
        let done = run(
            Path::new("/bin/sh"),
            vec!["-c".to_string(), "echo hi".to_string()],
            Some(Duration::from_secs(10)),
        );
        assert!(!done.timed_out);
        assert!(done.ok());
        assert!(done.stdout.contains("hi"));
    }

    #[test]
    fn missing_binary_reports_error() {
        let done = run(
            Path::new("/definitely/not/here/numdeck"),
            vec!["check".to_string()],
            None,
        );
        assert!(done.error.is_some());
        assert!(!done.ok());
        assert!(!done.timed_out);
    }

    /// 端到端：命令面板拼出来的 argv 真的交给 `numdeck` 跑一遍。
    ///
    /// 默认不跑，因为它依赖一个构建好的可执行文件。手跑：
    /// `cargo build -p numdeck && cargo test -p numdeck-gui -- --ignored --nocapture e2e`
    ///
    /// 判据只看 clap 那一层：参数形态不对时它会打 "unexpected argument" / "invalid
    /// value" 并以用法错误退出。至于「这台机器不支持」「档位不单调」这类业务拒绝，
    /// 属于真机才有的结论，不影响这里要验的东西——界面拼的参数 CLI 能认。
    #[test]
    #[ignore = "需要先构建 numdeck（或用 NUMDECK_BIN 指过去）"]
    fn e2e_argv_is_accepted_by_the_real_cli() {
        use crate::cmdform::{
            Advanced, BrightAction, CommandForm, LedAction, LevelsAction, SubCmd,
        };

        let Some(bin) = locate() else {
            panic!("找不到 numdeck，先 cargo build -p numdeck");
        };
        let cfg = std::env::temp_dir().join("numdeck-gui-e2e.json");
        let cfg_text = cfg.display().to_string();

        let base = |sub: SubCmd| CommandForm {
            sub,
            advanced: Advanced {
                config: cfg_text.clone(),
                ..Advanced::default()
            },
            ..CommandForm::default()
        };

        let mut cases: Vec<(&str, CommandForm)> = Vec::new();
        cases.push(("check", base(SubCmd::Check)));

        let mut bright = base(SubCmd::Bright);
        bright.bright_action = BrightAction::Step;
        bright.bright_step = "2".to_string();
        cases.push(("bright 2", bright));

        let mut follow_off = base(SubCmd::Bright);
        follow_off.bright_action = BrightAction::Follow;
        follow_off.bright_follow_on = false;
        cases.push(("bright follow off", follow_off));

        let mut levels = base(SubCmd::Levels);
        levels.levels_action = LevelsAction::Reset;
        cases.push(("levels reset", levels));

        let mut led_on = base(SubCmd::Led);
        led_on.led_action = LedAction::On;
        cases.push(("led on", led_on));

        let mut led_raw = base(SubCmd::Led);
        led_raw.led_action = LedAction::Raw;
        led_raw.led_raw = "0x41".to_string();
        cases.push(("led 0x41", led_raw));

        let mut layout = base(SubCmd::Layout);
        layout.layout_ids = true;
        cases.push(("layout --ids", layout));

        let mut sku = base(SubCmd::Sku);
        sku.sku_id = "list".to_string();
        cases.push(("sku list", sku));

        let mut lang = base(SubCmd::Lang);
        lang.lang_code = "auto".to_string();
        cases.push(("lang auto", lang));

        // 全局开关也走一遍，确认它们被放在子命令之前
        let mut verbose = base(SubCmd::Check);
        verbose.advanced.verbose = true;
        verbose.advanced.dump_config = true;
        cases.push(("--verbose --dump-config check", verbose));

        let mut advanced = base(SubCmd::Check);
        advanced.advanced.i2c_bus = "1".to_string();
        advanced.advanced.numlock = "off".to_string();
        cases.push(("数字型全局参数", advanced));

        for (label, form) in cases {
            let argv = form
                .argv()
                .unwrap_or_else(|e| panic!("{label} 拼参数失败：{e}"));
            let done = run(&bin, argv, Some(Duration::from_secs(10)));
            let text = format!("{}{}", done.stdout, done.stderr);
            for bad in [
                "unexpected argument",
                "invalid value",
                "unrecognized subcommand",
                "Found argument",
            ] {
                assert!(
                    !text.contains(bad),
                    "{label} 被 CLI 拒绝（{bad}）：\n{}",
                    done.summary()
                );
            }
            assert!(
                done.code.is_some(),
                "{label} 没有正常退出：\n{}",
                done.summary()
            );
        }

        let _ = std::fs::remove_file(&cfg);
        let _ = std::fs::remove_file(cfg.with_extension("json.bak"));
    }
}
