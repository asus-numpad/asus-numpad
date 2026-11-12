//! 界面里点出来的动作：先收集、再执行。
//!
//! 面板闭包只往 `Option<Action>` 里塞一枚动作，真正干活放在画完之后的
//! [`App::run_action`] 里。这样闭包里不会和 `self` 的其余字段互相借，也方便一眼看完
//! 「这个按钮到底会触发什么」。

use numdeck::led_proto;
use eframe::egui;

use crate::autostart;
use crate::settings::{self, Settings};
use crate::sysinfo::{self, ServiceVerb};

use super::App;

/// 界面里点出来的动作，先收集、等界面画完再执行，避免面板闭包和状态互相借。
pub(in crate::app) enum Action {
    Rescan,
    SendOff,
    SendCurrent,
    /// 按指定 level 下发（键盘四档映射表里逐档试亮用）
    SendLevel(u8, String),
    /// 把键盘四档映射整表写进驱动配置（`manual_brightness_levels`）
    SyncLevelsToDriver,
    /// 从驱动配置读回档位表，覆盖界面上的四档映射
    LoadLevelsFromDriver,
    SaveSettings,
    ResetSettings,
    SetAutostart(bool),
    ServiceRun(ServiceVerb),
    ServiceAutostart(bool),
    RefreshState,
    RunCommand,
    SaveDriver,
    SaveDriverAndRestart,
    ReloadDriver,
    ResetDriver,
}

impl App {
    pub(in crate::app) fn run_action(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Rescan => self.rescan(),
            Action::SendOff => {
                self.settings.on = false;
                self.send(led_proto::LED_OFF, "熄灭");
            }
            Action::SendCurrent => {
                self.settings.on = true;
                self.apply_current();
            }
            Action::SendLevel(level, note) => {
                self.settings.on = level != led_proto::LED_OFF;
                self.send(level, &note);
            }
            Action::SyncLevelsToDriver => self.sync_levels_to_driver(),
            Action::LoadLevelsFromDriver => self.load_levels_from_driver(),
            Action::SaveSettings => {
                self.settings.sanitize();
                match settings::save(&self.settings) {
                    Ok(()) => {
                        // 显式保存已落盘，节流待写标志一并清掉，避免 400 ms 后又重复写一次
                        self.settings_dirty = false;
                        self.settings_dirty_at = None;
                        self.set_status("设置已保存".to_string(), false);
                    }
                    Err(err) => self.set_status(format!("保存设置失败：{err:#}"), true),
                }
            }
            Action::ResetSettings => {
                self.settings = Settings::default();
                self.set_status("已恢复默认设置".to_string(), false);
                self.apply_current();
            }
            Action::SetAutostart(on) => {
                let cmd = autostart::command_line();
                match autostart::set(on, &cmd) {
                    Ok(()) => {
                        self.autostart_on = on;
                        let msg = if on {
                            "已登记开机自启"
                        } else {
                            "已取消开机自启"
                        };
                        self.set_status(msg.to_string(), false);
                    }
                    Err(err) => self.set_status(format!("更新自启项失败：{err:#}"), true),
                }
            }
            Action::ServiceRun(verb) => {
                let busy = match verb {
                    ServiceVerb::Start => "正在启动后台服务…",
                    ServiceVerb::Stop => "正在停止后台服务…",
                    ServiceVerb::Restart => "正在重启后台服务…",
                };
                self.start_op(ctx, busy, move || {
                    sysinfo::service_run(verb).map_err(|err| format!("{err:#}"))
                });
            }
            Action::ServiceAutostart(on) => {
                self.service_enabled = on;
                self.start_op(ctx, "正在更新开机自启…", move || {
                    sysinfo::service_autostart(on).map_err(|err| format!("{err:#}"))
                });
            }
            Action::RefreshState => self.state_want = true,
            Action::RunCommand => self.run_command(),
            Action::SaveDriver => match self.driver.save() {
                Ok(msg) => self.set_status(msg, false),
                Err(err) => self.set_status(format!("保存驱动配置失败：{err:#}"), true),
            },
            Action::SaveDriverAndRestart => {
                match self.driver.save() {
                    Ok(msg) => match sysinfo::service_run(ServiceVerb::Restart) {
                        Ok(done) => self.set_status(format!("{msg}；{done}"), false),
                        Err(err) => {
                            self.set_status(format!("{msg}，但后台服务重启失败：{err:#}"), true)
                        }
                    },
                    Err(err) => self.set_status(format!("保存驱动配置失败：{err:#}"), true),
                }
                self.state_want = true;
            }
            Action::ReloadDriver => {
                // 磁盘上可能被别处改过，重读是覆盖式：界面里没保存的改动会一起没掉，
                // 得说清楚，不能让人以为「重新读取」是合并。
                let dropped = self.driver.dirty;
                self.driver.reload();
                let path = self.driver.write_path.display().to_string();
                if dropped {
                    self.set_status(format!("已重新读取 {path}；界面上未保存的改动已丢弃"), true);
                } else {
                    self.set_status(format!("已重新读取 {path}"), false);
                }
            }
            Action::ResetDriver => {
                self.driver.reset();
                self.set_status("驱动配置已恢复官方默认值（还需点保存）".to_string(), false);
            }
        }
    }
}
