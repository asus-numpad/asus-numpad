//! 设置页。

use eframe::egui;

use crate::sysinfo::{self};
use crate::theme::{self};
use crate::{autostart, settings};

use super::super::widgets::open_dir;
use super::super::{Action, App};

impl App {
    pub(in crate::app) fn draw_settings(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::section(ui, &p, "启动行为", |ui| {
            theme::check_row(
                ui,
                &p,
                &mut self.settings.apply_on_start,
                "启动时下发当前档位",
                Some("勾上后每次打开界面都重发一遍当前档位，省得手动点「点亮」。"),
            );
            theme::note(
                ui,
                &p,
                "休眠、热插拔后 EC 会丢状态；勾选后每次启动都会重发一遍。",
            );

            if autostart::supported() {
                let mut on = self.autostart_on;
                if theme::check_row(
                    ui,
                    &p,
                    &mut on,
                    "开机自动启动",
                    Some("写入 HKCU\\...\\Run，不需要管理员权限；取消勾选即删除该项。"),
                )
                .changed()
                {
                    *action = Some(Action::SetAutostart(on));
                }
            } else if sysinfo::has_service() {
                // Linux 的「自启」就是 systemd 用户单元的 enable，和后台服务共用一份状态
                let mut on = self.service_enabled;
                if theme::check_row(
                    ui,
                    &p,
                    &mut on,
                    "开机自启",
                    Some(
                        "等价于 systemctl --user enable numdeck，和系统状态页共用同一份状态。",
                    ),
                )
                .changed()
                {
                    *action = Some(Action::ServiceAutostart(on));
                }
            } else {
                theme::note(ui, &p, "当前平台不支持开机自启")
            }

            ui.add_space(8.0);
            let picked = theme::row(
                ui,
                &p,
                "设置",
                Some("写的是界面自己的设置文件；驱动配置不受影响。"),
                |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[
                            (
                                "保存设置",
                                true,
                                "把上面的开关写进界面的设置文件，下次打开沿用。",
                            ),
                            ("恢复默认", false, "界面设置回到初始值；驱动配置不受影响。"),
                        ],
                    )
                },
            );
            match picked {
                Some(0) => *action = Some(Action::SaveSettings),
                Some(1) => *action = Some(Action::ResetSettings),
                _ => {}
            }
            match &self.font_note {
                Some(path) => {
                    theme::read_row(ui, &p, "界面字体", path, Some("界面实际加载的中文字体文件"));
                }
                None => theme::note(ui, &p, "未找到可用中文字体，界面可能显示为方块"),
            };
        });

        ui.add_space(10.0);

        // 路径与版本：出问题时第一个要看的就是这几行
        theme::section(ui, &p, "路径与版本", |ui| {
            theme::read_row(
                ui,
                &p,
                "界面设置",
                &settings::config_path().display().to_string(),
                Some("界面自己的设置文件：档位、联动间隔、开自启等。"),
            );
            theme::read_row(
                ui,
                &p,
                "驱动配置",
                &self.driver.write_path.display().to_string(),
                Some("后台服务读的那份配置；界面上「保存并重启服务」改的就是它。"),
            );
            theme::read_row(
                ui,
                &p,
                "当前接口",
                &self
                    .devices
                    .get(self.selected.unwrap_or(usize::MAX))
                    .map(|device| device.path.clone())
                    .unwrap_or_else(|| "未选择".to_string()),
                Some("当前选中的 HID 接口节点，报文都从这里出去。"),
            );
            theme::read_row(
                ui,
                &p,
                "版本",
                &format!(
                    "界面 {} / 库 {}",
                    env!("CARGO_PKG_VERSION"),
                    numdeck::VERSION
                ),
                Some("界面与驱动库各自的版本；对不上说明装的是混搭包。"),
            );

            ui.add_space(6.0);
            let picked = theme::row(
                ui,
                &p,
                "打开目录",
                Some("在系统文件管理器里定位配置所在目录，方便直接改配置文件。"),
                |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[
                            ("设置目录", false, "在文件管理器里打开界面设置所在目录。"),
                            ("驱动目录", false, "在文件管理器里打开驱动配置所在目录。"),
                        ],
                    )
                },
            );
            match picked {
                Some(0) => {
                    let dir = settings::config_path();
                    let dir = dir.parent().unwrap_or(&dir).to_path_buf();
                    self.report_open(open_dir(&dir), &dir);
                }
                Some(1) => {
                    let file = self.driver.write_path.clone();
                    let dir = file.parent().unwrap_or(&file).to_path_buf();
                    self.report_open(open_dir(&dir), &dir);
                }
                _ => {}
            }
        });
    }
}
