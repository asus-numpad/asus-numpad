//! 表单命令的后台执行。
//!
//! 界面把 `CommandForm` 拼成一行 `numdeck <args>`，交给线程跑；读 DMI、等 systemd
//! 重启都可能几百毫秒，同步跑会把界面卡住。这里的职责只有三件：起进程、收结果、
//! 把结果翻成日志与状态文案。

use crate::cli_bridge::{self, Completion};

use super::App;

impl App {
    /// 把表单拼成 `numdeck <args>` 交给后台线程跑。
    ///
    /// 不在这里直接 `Command::output()`：读 DMI、等 systemd 重启服务都可能要几百毫秒，
    /// 同步执行会把界面卡住。
    pub(super) fn run_command(&mut self) {
        if self.cmd_running {
            return;
        }
        let Some(bin) = self.cmd_bin.clone() else {
            self.set_status(
                "找不到 numdeck 可执行文件：装好驱动包后点「重新查找」，或设置 NUMDECK_BIN"
                    .to_string(),
                true,
            );
            return;
        };
        let args = match self.cmd.argv() {
            Ok(args) => args,
            Err(err) => {
                self.set_status(format!("参数有误：{err}"), true);
                return;
            }
        };
        let label = args.join(" ");
        let timeout = self.cmd.timeout();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let done = cli_bridge::run(&bin, args, timeout);
            let _ = tx.send(done);
        });
        self.cmd_rx = Some(rx);
        self.cmd_running = true;
        let note = match timeout {
            Some(limit) => format!(
                "正在执行 numdeck {label} ……（{} 秒后自动收工）",
                limit.as_secs()
            ),
            None => format!("正在执行 numdeck {label} ……"),
        };
        self.set_status(note, false);
    }

    pub(super) fn poll_command(&mut self) {
        use std::sync::mpsc::TryRecvError;

        let Some(rx) = self.cmd_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(done) => {
                self.cmd_rx = None;
                self.cmd_running = false;
                self.finish_command(done);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.cmd_rx = None;
                self.cmd_running = false;
                self.set_status("命令行进程异常中断".to_string(), true);
            }
        }
    }

    pub(super) fn finish_command(&mut self, done: Completion) {
        let ok = done.ok();
        let wrote = self.cmd.writes_config();
        let summary = done.summary();

        // 日志里放子命令本身与首行结果，完整输出留在输出框
        let label = if done.argv.len() > 1 {
            done.argv[1..].join(" ")
        } else {
            "numdeck".to_string()
        };
        let mut note = done
            .error
            .clone()
            .into_iter()
            .chain(done.stderr.lines().map(str::to_string))
            .chain(done.stdout.lines().map(str::to_string))
            .map(|line| line.trim().to_string())
            .find(|line| !line.is_empty())
            .unwrap_or_else(|| match done.code {
                Some(code) => format!("退出码 {code}"),
                None => "被信号终止".to_string(),
            });
        if note.chars().count() > 60 {
            note = note.chars().take(60).collect::<String>() + "…";
        }
        self.push_log(format!("cli {label}"), note.clone(), ok);

        self.cmd_ok = ok;
        self.cmd_out = Some(summary);
        let verdict = if ok {
            "完成".to_string()
        } else if done.timed_out {
            "已按时间上限终止，输出见下方".to_string()
        } else {
            note.clone()
        };
        self.set_status(format!("numdeck {label}：{verdict}"), !ok);

        // 写配置的子命令会改机型 / 布局 / 档位 / 语言，界面上的状态跟着重读一次
        if ok && wrote {
            self.state_want = true;
        }
    }

    pub(super) fn rescan(&mut self) {
        match self.backend.scan() {
            Ok(devices) => {
                self.devices = devices;
                let wanted = self.settings.device_path.clone();
                let by_path = wanted
                    .as_deref()
                    .and_then(|p| self.devices.iter().position(|d| d.path == p));
                // 记住的接口必须还能点灯，否则退回自动挑选
                let by_path = by_path.filter(|&i| self.devices[i].usable());
                self.selected = by_path.or_else(|| self.devices.iter().position(|d| d.selected));
                let msg = match self.selected {
                    Some(i) => {
                        let d = &self.devices[i];
                        self.settings.device_path = Some(d.path.clone());
                        format!("已选中 {}（{}）", d.vid_pid(), d.vendor())
                    }
                    None => "未找到可点灯的接口".to_string(),
                };
                let bad = self.selected.is_none();
                self.set_status(msg, bad);
            }
            Err(err) => {
                self.devices.clear();
                self.selected = None;
                self.set_status(format!("枚举 HID 接口失败：{err:#}"), true);
            }
        }
    }
}
