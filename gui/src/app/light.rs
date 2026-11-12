//! 灯光：档位下发、策略档表、NumLock 跟随。
//!
//! 档位映射不在这里算——统一走 `numdeck::brightness` / `led_proto`，GUI 只负责
//! 「用户点到哪一档」与「把结果写成一行日志」。

use std::time::{Duration, Instant};

use numdeck::led_proto;

use crate::backend;
use crate::kbdmap;

use super::{hex_table, now_hms, App, LogEntry, MAX_LOG};

impl App {
    /// 当前档位组对应的原始命令字表。
    ///
    /// 4 档（对齐键盘背光）用界面上的映射表，逐档可自定义；8 档是纯亮度控制，
    /// 仍按 `0x00`/`0x41..=0x48` 线性等分，与 Linux 侧同一函数。
    pub(super) fn level_table(&self) -> Vec<u8> {
        if self.settings.steps as usize == kbdmap::KBD_STEPS {
            return self.settings.kbd_map.clone();
        }
        backend::level_table(self.settings.steps)
    }

    pub(super) fn level_for_step(&self, step: u8) -> Option<u8> {
        if self.settings.steps as usize == kbdmap::KBD_STEPS {
            return kbdmap::level_for(step, &self.settings.kbd_map);
        }
        backend::level_for_step(self.settings.steps, step)
    }

    pub(super) fn push_log(&mut self, frame: String, note: String, ok: bool) {
        self.log.push_front(LogEntry {
            at: now_hms(),
            frame,
            note,
            ok,
        });
        self.log.truncate(MAX_LOG);
    }

    pub(super) fn send(&mut self, value: u8, note: &str) {
        let Some(idx) = self.selected else {
            self.set_status("没有可用的灯光接口".to_string(), true);
            return;
        };
        let device = self.devices[idx].clone();
        if !device.usable() {
            self.set_status(
                format!("{} 不带 FEATURE 0x0d，无法点灯", device.vid_pid()),
                true,
            );
            return;
        }
        let Some(frame) = led_proto::encode(value) else {
            return;
        };
        let result = self.backend.write(&device, frame);
        let hex = led_proto::hex(&frame);
        match result {
            Ok(()) => {
                self.push_log(hex.clone(), note.to_string(), true);
                self.set_status(format!("已写入 {hex}（{note}）"), false);
            }
            Err(err) => {
                self.push_log(hex.clone(), format!("{note}·失败"), false);
                self.set_status(format!("写入 {hex} 失败：{err:#}"), true);
            }
        }
    }

    pub(super) fn apply_current(&mut self) {
        if !self.settings.on {
            self.send(led_proto::LED_OFF, "熄灭");
            return;
        }
        match self.level_for_step(self.settings.step) {
            Some(0) => self.send(led_proto::LED_OFF, "档位 1＝熄灭"),
            Some(level) => {
                let note = format!("亮度 {} 档", self.settings.step);
                self.send(level, &note)
            }
            None => self.set_status("档位越界，未下发".to_string(), true),
        }
    }

    /// 把灯光页的四档映射整表写进驱动配置。
    ///
    /// 界面设置与驱动配置本来是两份东西，但档位表是两边共用的语义：界面上调好的
    /// 四档若不落到 `manual_brightness_levels`，拔插或重启后服务仍按旧表点灯。
    /// 这里直接写驱动那份文件，写完再把结果读回界面，保证看到的就是磁盘上的值。
    pub(super) fn sync_levels_to_driver(&mut self) {
        let map = self.settings.kbd_map.clone();
        if !kbdmap::monotonic(&map) {
            self.set_status(
                format!(
                    "未写入：{} 不是从暗到亮递增，驱动不接受这样的档位表",
                    hex_table(&map)
                ),
                true,
            );
            return;
        }
        self.driver.cfg.manual_brightness_levels = Some(map);
        self.driver.dirty = true;
        match self.driver.save() {
            Ok(msg) => {
                if let Some(table) = self.driver.cfg.manual_brightness_levels.clone() {
                    if table.len() == kbdmap::KBD_STEPS {
                        self.settings.kbd_map = table;
                    }
                }
                let now = hex_table(&self.settings.kbd_map);
                self.set_status(format!("{msg}；四档映射已同步为 {now}"), false);
            }
            Err(err) => self.set_status(format!("写入驱动配置失败：{err:#}"), true),
        }
    }

    /// 反过来：把驱动配置里的档位表读进灯光页，按服务当前实际在用的表点灯。
    pub(super) fn load_levels_from_driver(&mut self) {
        match self.driver.cfg.manual_brightness_levels.clone() {
            Some(table) if table.len() == kbdmap::KBD_STEPS => {
                let mut table = table;
                kbdmap::sanitize(&mut table);
                self.settings.kbd_map = table;
                let now = hex_table(&self.settings.kbd_map);
                self.set_status(format!("已读入驱动配置的四档映射：{now}"), false);
            }
            Some(table) => self.set_status(
                format!(
                    "驱动配置的档位表是 {} 档，套不进键盘四档（先在驱动配置页改成 4 档）",
                    table.len()
                ),
                true,
            ),
            None => self.set_status(
                "驱动配置没有显式档位表（驱动自己线性等分），界面保持当前映射".to_string(),
                true,
            ),
        }
    }

    pub(super) fn poll_numlock(&mut self) {
        if !self.settings.follow_numlock {
            return;
        }
        if self.last_poll.elapsed() < Duration::from_millis(self.settings.poll_ms) {
            return;
        }
        self.last_poll = Instant::now();
        let Some(on) = self.backend.numlock() else {
            return;
        };
        self.last_numlock = Some(on);
        // 第一次读到状态只用来对齐：硬件此刻就是这个状态，没必要再写一遍让灯闪一下
        if self.sent_numlock.is_none() {
            self.sent_numlock = Some(on);
            return;
        }
        if self.sent_numlock == Some(on) {
            return;
        }
        self.sent_numlock = Some(on);
        if on {
            if self.settings.on {
                self.apply_current();
            }
        } else {
            self.send(led_proto::LED_OFF, "NumLock 关闭，熄灯");
        }
    }

    pub(super) fn numlock_text(&self) -> String {
        match self.last_numlock {
            Some(true) => "NumLock：开".to_string(),
            Some(false) => "NumLock：关".to_string(),
            None => "NumLock：未知".to_string(),
        }
    }
}
