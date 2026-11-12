//! 后台任务：状态重读、服务动作、设置落盘。
//!
//! 这三件事的共同点是「慢」——起 `systemctl` 子进程、等服务真起来、写磁盘。它们全部
//! 走同一套形状：界面线程发包，下一帧取回结果，取不到就过一会儿再看。集中在这里，
//! 免得每个调用点各写一遍超时与重试。

use std::time::{Duration, Instant};

use eframe::egui;

use crate::settings;
use crate::sysinfo;

use super::App;

/// 平台状态里 systemctl 要真起进程，别每帧都刷。
pub(super) const STATE_REFRESH: Duration = Duration::from_secs(10);

/// 一次状态读取最多等多久。`snapshot()` 里要问 systemctl / 读 sysfs，机器忙或者
/// dbus 打嗝时可能几十秒不回来；不设上限的话 `state_rx` 一直占着，每 10 秒的重读
/// 全被「上一次还没回来」挡掉，界面就停在旧状态上且毫无提示。
const STATE_TIMEOUT: Duration = Duration::from_secs(8);

/// 设置落盘的静默时间：拖滑块/数字框时每帧都在变，停手这么久才写一次文件。
///
/// 早先是「一有变化就写」，拖动一次亮度滑块能连写几十次磁盘；现在只在停手后
/// 落一次，窗口关闭时（`App::drop`）再兜一次，不会丢最后的改动。
pub(super) const SETTINGS_SAVE_QUIET: Duration = Duration::from_millis(400);

impl App {
    /// 重读平台状态。服务启停之后必须调用，否则界面还显示旧状态。
    ///
    /// 起子进程读 systemd 状态这一步放到线程里：界面线程只负责发包、收包。
    /// 还在读的时候再调一次不会排队（`state_rx` 占着就跳过）。
    pub(super) fn refresh_state(&mut self, ctx: &egui::Context) {
        if self.state_rx.is_some() {
            // 上一次还没回来：留着这个请求，等收包后再发起，别把「刚重启完服务，
            // 界面还显示旧状态」拖到下一个 10 秒周期
            self.state_want = true;
            return;
        }
        self.state_want = false;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(sysinfo::snapshot());
        });
        self.state_rx = Some(rx);
        self.state_at = Instant::now();
        // 结果到达时得有下一次重绘，否则最快也要等下一次鼠标移动才更新
        ctx.request_repaint_after(Duration::from_millis(120));
    }

    /// 把一次服务动作丢给线程，做完下一帧取结果。
    ///
    /// 已经在跑的时候直接忽略新请求：`systemctl restart` 连点两下等于让两条命令抢同一个
    /// 单元，最后的状态谁也说不准，还都卡在界面线程上。
    pub(super) fn start_op<F>(&mut self, ctx: &egui::Context, busy: &str, job: F)
    where
        F: FnOnce() -> Result<String, String> + Send + 'static,
    {
        if self.op_rx.is_some() {
            self.set_status("上一步服务操作还没结束，稍等一下".to_string(), false);
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(job());
        });
        self.op_rx = Some(rx);
        self.set_status(busy.to_string(), false);
        ctx.request_repaint_after(Duration::from_millis(120));
    }

    /// 收服务动作的结果。
    pub(super) fn poll_op(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.op_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(msg)) => {
                self.op_rx = None;
                self.set_status(msg, false);
                // 服务刚动过，状态区要立刻重读，否则还显示旧值
                self.state_want = true;
                ctx.request_repaint();
            }
            Ok(Err(err)) => {
                self.op_rx = None;
                self.set_status(err, true);
                self.state_want = true;
                ctx.request_repaint();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(120));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.op_rx = None;
                self.set_status("服务操作没有回话（执行线程异常退出）".to_string(), true);
            }
        }
    }

    /// 把后台读到的平台状态收进来。
    pub(super) fn poll_state(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.state_rx.as_ref() else {
            return;
        };
        // 读得太久就放弃这一次（线程自己会跑完，只是结果不要了），放行下一次重读，
        // 否则界面会一直显示上一次读到的状态而看不出是「卡住了」还是「真的没变」。
        if self.state_at.elapsed() > STATE_TIMEOUT {
            self.state_rx = None;
            self.state_want = true; // 下一帧就重新发起
            self.set_status(
                "读取系统状态超时（systemctl 或 sysfs 没有及时返回），稍后自动重试".to_string(),
                true,
            );
            ctx.request_repaint();
            return;
        }
        match rx.try_recv() {
            Ok(snap) => {
                self.state = snap;
                self.service_enabled = self
                    .state
                    .service
                    .as_ref()
                    .map(|s| s.enabled.trim() == "enabled")
                    .unwrap_or(false);
                self.state_rx = None;
                ctx.request_repaint();
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // 线程还没回话：约一帧之后再试，别把界面挂在等待上
                ctx.request_repaint_after(Duration::from_millis(120));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // 线程没了（理论上不会）：清掉占位，允许重新发起，别永久卡住刷新
                self.state_rx = None;
                self.state_at = Instant::now();
            }
        }
    }

    /// 设置脏了且已经停手，就写一次盘；还没停手则约好下一次重绘，别让计时停住。
    pub(super) fn flush_settings(&mut self, ctx: &egui::Context) {
        if !self.settings_dirty {
            return;
        }
        let waited = self
            .settings_dirty_at
            .map(|at| at.elapsed())
            .unwrap_or(SETTINGS_SAVE_QUIET);
        if waited < SETTINGS_SAVE_QUIET {
            ctx.request_repaint_after(SETTINGS_SAVE_QUIET - waited);
            return;
        }
        match settings::save(&self.settings) {
            Ok(()) => {
                self.settings_dirty = false;
                self.settings_dirty_at = None;
                self.settings_save_err = None;
            }
            Err(err) => {
                // 写不进去（磁盘满、目录被删、权限被改）绝不能静默：界面其他地方都在
                // 说「已保存」，实际重启之后就没了。保留脏标记，等 400 ms 再试一次；
                // 同一条错误只报一次，别把状态栏刷成滚动日志。
                let text = err.to_string();
                if self.settings_save_err.as_deref() != Some(text.as_str()) {
                    self.set_status(format!("设置没能写入磁盘：{text}（稍后自动重试）"), true);
                    self.settings_save_err = Some(text);
                }
                self.settings_dirty_at = Some(Instant::now());
                ctx.request_repaint_after(SETTINGS_SAVE_QUIET);
            }
        }
    }
}
