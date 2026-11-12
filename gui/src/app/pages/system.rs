//! 系统页与诊断页。

use eframe::egui;

use numdeck::led_proto;

use crate::settings;
use crate::sysinfo::ServiceVerb;
use crate::theme::{self};

use super::super::widgets::{stamp, state_help};
use super::super::{Action, App};

impl App {
    /// 只读的平台状态 + 后台服务开关。
    ///
    /// 灯不亮时这一区先给答案：机型认没认出来、布局取的是哪套、背光通道在不在、
    /// 后台服务是否在跑。省掉「回去敲命令行再看一遍」这一步。
    pub(in crate::app) fn draw_system(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row(ui, &p, "系统状态");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::tip(
                        theme::secondary(ui, &p, "刷新"),
                        "重读机型、布局、背光通道与后台服务状态。插拔设备后点一下即可。",
                    )
                    .clicked()
                    {
                        *action = Some(Action::RefreshState);
                    }
                });
            });

            for (name, value) in &self.state.rows {
                theme::read_row(ui, &p, name, value, state_help(name));
            }

            for hint in &self.state.hints {
                theme::row(
                    ui,
                    &p,
                    "提示",
                    Some("驱动运行时给出的即时提示，出现这行黄字说明某项配置需要留意。"),
                    |ui| {
                        ui.label(egui::RichText::new(hint.clone()).small().color(p.warn));
                    },
                );
            }

            if let Some(service) = &self.state.service {
                ui.add_space(4.0);
                let color = if service.is_active() { p.ok } else { p.warn };
                // 服务状态是一行值，走 read_row 就落在控件列；三颗按钮另起一行平分控件列，
                // 不再让「一段状态文字 + 三颗按钮」挤在一行、右边缘和别处对不上。
                theme::row(
                    ui,
                    &p,
                    "后台服务",
                    Some("用户单元不需要 root；停止后数字键与手势失效，触摸板恢复成普通触摸板。"),
                    |ui| {
                        ui.label(egui::RichText::new("状态").size(12.5).color(p.text_weak));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {}",
                                service.active, service.enabled
                            ))
                            .size(12.5)
                            .color(color),
                        );
                    },
                );
                let picked = theme::row(
                    ui,
                    &p,
                    "服务控制",
                    Some("systemd 用户单元 numdeck.service 的启停与重启；重启用于让刚写入的驱动配置生效。"),
                    |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[
                            ("启动", false, "systemctl --user start numdeck"),
                            ("停止", false, "systemctl --user stop numdeck"),
                            ("重启", false, "改完驱动配置后用它让新配置生效"),
                        ],
                    )
                });
                match picked {
                    Some(0) => *action = Some(Action::ServiceRun(ServiceVerb::Start)),
                    Some(1) => *action = Some(Action::ServiceRun(ServiceVerb::Stop)),
                    Some(2) => *action = Some(Action::ServiceRun(ServiceVerb::Restart)),
                    _ => {}
                }
                theme::note(
                    ui,
                    &p,
                    "用户单元不需要 root；停止后数字键与手势失效，触摸板恢复成普通触摸板。",
                )
            }

            // 诊断出口放最后：它是「要发出去问人」时才点的，不该挤掉上面的状态与服务操作
            ui.add_space(6.0);
            self.draw_diagnostics(ui);
        });
    }

    /// 诊断信息出口：内容与 `numdeck-gui --selftest` 同一份，省得为了看一眼接口和
    /// 报文去开终端。写文件只落在界面自己的配置目录旁边，不碰系统路径。
    pub(in crate::app) fn draw_diagnostics(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let picked =
            theme::row(
                ui,
                &p,
                "诊断导出",
                Some("内容：平台状态、接口列表、8 档与熄灭报文——和 --selftest 打印的一致。"),
                |ui| {
                    theme::actions(
                    ui,
                    &p,
                    &[
                        ("复制诊断", false, "把整份诊断文本放进剪贴板，方便贴到 issue 里。"),
                        (
                            "保存报告",
                            false,
                            "在界面设置文件旁边写一份 diag-时间戳.txt，内容与 --selftest 一致。",
                        ),
                    ],
                )
                },
            );
        {
            if picked == Some(0) {
                let text = self.diagnostics();
                ui.ctx().copy_text(text);
                self.set_status("诊断信息已复制到剪贴板".to_string(), false);
            }
            if picked == Some(1) {
                let text = self.diagnostics();
                let path = settings::config_path()
                    .parent()
                    .map(|dir| dir.join(format!("diag-{}.txt", stamp())));
                match path.map(|path| std::fs::write(&path, &text).map(|_| path)) {
                    Some(Ok(path)) => {
                        self.set_status(format!("诊断报告已写入 {}", path.display()), false)
                    }
                    Some(Err(err)) => {
                        self.set_status(format!("写诊断报告失败：{err}"), true);
                    }
                    None => self.set_status("取不到设置文件目录".to_string(), true),
                }
            }
        }
    }

    /// 与 `--selftest` 同构的诊断文本。
    pub(in crate::app) fn diagnostics(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "后端：{}（真实硬件：{}）\n",
            self.backend.name(),
            self.backend.is_real()
        ));
        out.push_str(&format!("{}\n", self.state.subtitle));
        for (name, value) in &self.state.rows {
            out.push_str(&format!("  {name}：{value}\n"));
        }
        for hint in &self.state.hints {
            out.push_str(&format!("  提示：{hint}\n"));
        }
        if let Some(service) = &self.state.service {
            out.push_str(&format!(
                "  后台服务：{} / {}\n",
                service.active, service.enabled
            ));
        }

        if self.devices.is_empty() {
            out.push_str("未枚举到任何 HID 接口。\n");
        } else {
            out.push_str(&format!("共 {} 个接口：\n", self.devices.len()));
            for device in &self.devices {
                out.push_str(&format!(
                    "  {:<10} {:<8} feature={:<5} len={:<3} rank={:?} selected={}\n",
                    device.vid_pid(),
                    device.vendor(),
                    device.has_feature,
                    device.feature_len,
                    device.rank,
                    device.selected
                ));
                out.push_str(&format!("             path={}\n", device.path));
            }
        }

        out.push_str("\n档位报文：\n");
        for (index, level) in led_proto::brightness_levels().iter().enumerate() {
            if let Some(frame) = led_proto::encode(*level) {
                out.push_str(&format!(
                    "  {} 档 0x{:02x} -> {}\n",
                    index + 1,
                    level,
                    led_proto::hex(&frame)
                ));
            }
        }
        for (label, level) in [
            ("熄灭", led_proto::LED_OFF),
            ("普通点亮", led_proto::LED_ON),
        ] {
            if let Some(frame) = led_proto::encode(level) {
                out.push_str(&format!("{label}报文：{}\n", led_proto::hex(&frame)));
            }
        }
        out
    }
}
