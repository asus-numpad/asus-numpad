//! 命令页：目标、参数、高级项、执行与输出。
//!
//! 表单本身是纯数据（`cmdform::CommandForm`），这一组函数只做两件事：把控件
//! 绑定到表单字段、在提交时把表单翻译成 `Action::Run`。

use eframe::egui;

use crate::cli_bridge;
use crate::cmdform::{BrightAction, LedAction, LevelsAction, SubCmd};
use crate::theme::{self};

use super::super::widgets::{combo, field, small, tail_lines};
use super::super::{Action, App, VISIBLE_OUTPUT_LINES};

impl App {
    /// 命令面板：把 CLI 的全部子命令接到界面上。
    ///
    /// 做法是拼参数、调 `numdeck` 本身，而不是在界面里重写一遍：`check` 要读 DMI，
    /// `levels` / `sku` / `lang` 要改配置文件，输出文案又跟着语言文件走，重写必然与
    /// CLI 漂移。界面上显示的命令行与终端里敲下的完全一致，可直接复制过去复现。
    pub(in crate::app) fn draw_commands(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        // 页签已经是独立一页，内部不再折叠：收起来等于把功能藏起来。
        theme::section(ui, &p, "命令行（numdeck 全部子命令）", |ui| {
            self.draw_command_target(ui);
            ui.add_space(4.0);

            theme::row(
                ui,
                &p,
                "子命令",
                Some("选一条 numdeck 子命令，下面的参数会跟着变；执行时拼出的命令行与终端里完全一致。"),
                |ui| {
                    let combo = egui::ComboBox::from_id_salt("cmd_sub")
                        .width(crate::theme::ctrl_width())
                        .selected_text(self.cmd.sub.name())
                        .show_ui(ui, |ui| {
                            for sub in SubCmd::ALL {
                                ui.selectable_value(&mut self.cmd.sub, sub, sub.name());
                            }
                        });
                    theme::tip(combo.response, "切换子命令；右侧一句是该子命令的作用。");
                },
            );

            ui.add_space(2.0);
            // 子命令自己带一句说明（CmdForm::about），直接摆在下拉框下面：
            // 名字叫 markers / levels 的子命令，光看名字猜不出它是读还是写。
            theme::note(ui, &p, self.cmd.sub.about());
            ui.add_space(6.0);
            self.draw_command_args(ui);
            self.draw_command_advanced(ui);
            ui.add_space(4.0);
            self.draw_command_launcher(ui, action);
            self.draw_command_output(ui);
        });
    }

    pub(in crate::app) fn draw_command_target(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        // 可执行文件是一行只读值，走 read_row 就自动落在控件列里、超长时截断，
        // 不再像以前那样「一段文字 + 一颗按钮」挤在一行，按钮宽度还跟别处不一样。
        let shown = match &self.cmd_bin {
            Some(path) => path.display().to_string(),
            None => "未找到，点下面的「重新查找」".to_string(),
        };
        theme::read_row(
            ui,
            &p,
            "可执行文件",
            &shown,
            Some("界面调用的就是同一个 numdeck 二进制，输出与终端完全一致。"),
        );
        let missing = self.cmd_bin.is_none();
        let picked = theme::row(ui, &p, "", None, |ui| {
            theme::actions(
                ui,
                &p,
                &[("重新查找", false, "按 PATH 与环境变量重找一次可执行文件。")],
            )
        });
        if picked == Some(0) {
            self.cmd_bin = cli_bridge::locate();
        }
        if missing {
            theme::note(
                ui,
                &p,
                "装好驱动包后再点「重新查找」；或用环境变量 NUMDECK_BIN 指向可执行文件。",
            )
        }
        theme::note(
            ui,
            &p,
            "这里调的就是同一个 numdeck 二进制，输出与终端一致。`run` 是前台常驻引擎，界面上对应的入口在「系统状态」区的服务启停。",
        );
    }

    pub(in crate::app) fn draw_command_args(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        match self.cmd.sub {
            SubCmd::Check => theme::note(ui, &p, "这条子命令不需要参数。"),
            SubCmd::Dump => {
                theme::note(
                    ui,
                    &p,
                    "打印原始触摸板事件，本该一直跑到 Ctrl+C。这里只跑 5 秒就收工，把这段时间里的事件贴到下面。",
                );
            }
            SubCmd::Bright => {
                let items: Vec<_> = BrightAction::ALL.iter().map(|a| (*a, a.label())).collect();
                theme::row(
                    ui,
                    &p,
                    "动作",
                    Some("bright 的动作：step 设档、cycle/up/down 挪一档、follow 开关跟随、status 只看状态。"),
                    |ui| combo(ui, "cmd_bright_action", &mut self.cmd.bright_action, &items),
                );
                match self.cmd.bright_action {
                    BrightAction::Step => {
                        field(
                            ui,
                            &p,
                            "档位",
                            &mut self.cmd.bright_step,
                            "1 起，取决于档位数",
                            Some("要切到的档位序号，从 1 开始，上限是当前档位组的大小。"),
                        );
                    }
                    BrightAction::Follow => {
                        theme::check_row(
                            ui,
                            &p,
                            &mut self.cmd.bright_follow_on,
                            "开启跟随",
                            Some(
                                "不勾＝关闭跟随；关闭后不管键盘背光怎么变，触摸板都保持固定亮度。",
                            ),
                        );
                        theme::note(ui, &p, "会写入配置并重启后台服务。")
                    }
                    BrightAction::Status => {
                        theme::note(ui, &p, "只打印当前档位与跟随状态，不改配置。")
                    }
                    BrightAction::Cycle | BrightAction::Up | BrightAction::Down => {
                        theme::note(ui, &p, "按当前档位表挪一档，并重启后台服务。")
                    }
                }
            }
            SubCmd::Levels => {
                let items: Vec<_> = LevelsAction::ALL.iter().map(|a| (*a, a.label())).collect();
                theme::row(
                    ui,
                    &p,
                    "动作",
                    Some("levels 的动作：table 打印档位表、set 单档改命令字、reset 恢复默认。"),
                    |ui| combo(ui, "cmd_levels_action", &mut self.cmd.levels_action, &items),
                );
                match self.cmd.levels_action {
                    LevelsAction::Table => theme::note(ui, &p, "打印每档对应的命令字。"),
                    LevelsAction::Reset => {
                        theme::note(ui, &p, "把档位表恢复成默认，并重启后台服务。")
                    }
                    LevelsAction::Set => {
                        field(
                            ui,
                            &p,
                            "档位",
                            &mut self.cmd.levels_step,
                            "从 1 开始",
                            Some("要改的是第几档。"),
                        );
                        field(
                            ui,
                            &p,
                            "命令字",
                            &mut self.cmd.levels_value,
                            "0x41 到 0x48，或十进制",
                            Some("这一档要写的 level 字节，合法范围 0x41..=0x48。"),
                        );
                        theme::note(ui, &p, "相邻档必须单调，写反了 CLI 会拒绝并保持原配置。")
                    }
                }
            }
            SubCmd::Led => {
                let items: Vec<_> = LedAction::ALL.iter().map(|a| (*a, a.label())).collect();
                theme::row(
                    ui,
                    &p,
                    "动作",
                    Some("led 的动作：on/off 直接下发，raw 绕开状态机写指定命令字。"),
                    |ui| combo(ui, "cmd_led_action", &mut self.cmd.led_action, &items),
                );
                match self.cmd.led_action {
                    LedAction::Status => {
                        theme::note(
                            ui,
                            &p,
                            "CLI 的 led 没有状态查询，不带值只会打印用法。当前命令字看「灯光」区或 check。",
                        );
                    }
                    LedAction::Raw => {
                        field(
                            ui,
                            &p,
                            "命令字",
                            &mut self.cmd.led_raw,
                            "如 0x41",
                            Some("要直接下发的 level 字节，十六进制或十进制都可以。"),
                        );
                        theme::note(
                            ui,
                            &p,
                            "绕开状态机直接下发：led on 亮而 led 0x41 不亮，就说明这台机器只吃两态。",
                        );
                    }
                    LedAction::On | LedAction::Off => {
                        theme::note(ui, &p, "直接从当前选中的接口下发一帧，不改配置。")
                    }
                }
            }
            SubCmd::Layout => {
                field(
                    ui,
                    &p,
                    "布局名",
                    &mut self.cmd.layout_name,
                    "留空＝只查看，如 GH3XLNDZ",
                    Some("填布局名会写配置；留空只打印当前布局与可选列表。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut self.cmd.layout_ids,
                    "打印 HID 对照表",
                    Some("勾上相当于加 --ids：把 HID 号与布局的对照表一并打印出来。"),
                );
                if !self.cmd.layout_name.trim().is_empty() {
                    theme::note(ui, &p, "填了布局名就会写配置并重启后台服务。")
                }
            }
            SubCmd::Sku => {
                field(
                    ui,
                    &p,
                    "机型",
                    &mut self.cmd.sku_id,
                    "留空＝查看；list＝列全部；auto＝恢复自动识别",
                    Some("填具体机型 ID 会覆盖自动识别结果；list 列全部机型，auto 恢复自动。"),
                );
                theme::note(ui, &p, "指定具体机型 ID 会写配置，覆盖自动识别结果。")
            }
            SubCmd::Lang => {
                field(
                    ui,
                    &p,
                    "语言",
                    &mut self.cmd.lang_code,
                    "留空＝查看；auto / en / zh_CN / zh_TW",
                    Some("界面与日志的语言：auto 跟系统，也可指定 en / zh_CN / zh_TW。"),
                );
            }
        }
    }

    pub(in crate::app) fn draw_command_advanced(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("高级选项（全局参数）")
            .default_open(false)
            .show(ui, |ui| {
                let p = self.palette;
                let adv = &mut self.cmd.advanced;
                let rows: [(&str, &mut String, &str, &str); 12] = [
                    (
                        "--config",
                        &mut adv.config,
                        "配置文件路径",
                        "改用另一份配置文件，默认走系统的 /etc/numdeck/config.json。",
                    ),
                    (
                        "--device",
                        &mut adv.device,
                        "指定触摸板设备",
                        "覆盖自动挑选的触摸板节点。",
                    ),
                    (
                        "--keyboard",
                        &mut adv.keyboard,
                        "键盘设备（读背光状态用）",
                        "跟着键盘背光走时，从这里读当前背光档。",
                    ),
                    (
                        "--led",
                        &mut adv.led,
                        "LED 设备",
                        "指定 LEDs 节点，一般是 input::numlock。",
                    ),
                    (
                        "--hidraw",
                        &mut adv.hidraw,
                        "hidraw 节点名，如 hidraw3",
                        "直连 hidraw 下发 FEATURE 报文时用。",
                    ),
                    (
                        "--i2c-bus",
                        &mut adv.i2c_bus,
                        "整数",
                        "走 i2c 通道时指定总线号。",
                    ),
                    (
                        "--i2c-addr",
                        &mut adv.i2c_addr,
                        "如 0x15",
                        "走 i2c 通道时指定从机地址。",
                    ),
                    (
                        "--i2c-transfer",
                        &mut adv.i2c_transfer,
                        "i2c 传输方式",
                        "i2c 的传输模式，默认 i2ctransfer。",
                    ),
                    (
                        "--key-mode",
                        &mut adv.key_mode,
                        "键盘扫描码模式",
                        "键码风格：raw / mapped 一类，和布局配套。",
                    ),
                    (
                        "--layout",
                        &mut adv.layout,
                        "覆盖布局名",
                        "临时覆盖配置里的布局，不落盘。",
                    ),
                    (
                        "--sku",
                        &mut adv.sku,
                        "覆盖机型 ID",
                        "临时覆盖配置里的机型，不落盘。",
                    ),
                    (
                        "--numlock",
                        &mut adv.numlock,
                        "覆盖 NumLock 状态来源",
                        "临时指定 NumLock 状态来源，如 sysfs / evdev。",
                    ),
                ];
                for (flag, value, hint, help) in rows {
                    theme::row(ui, &p, flag, Some(help), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text(hint)
                                .desired_width(crate::theme::ctrl_width()),
                        );
                    });
                }
                theme::row(
                    ui,
                    &p,
                    "--mixed-mode",
                    Some("混排模式：有虚拟触摸板就开（auto）/ 强制开 / 纯数字键盘。"),
                    |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut adv.mixed_mode)
                                .hint_text("混排模式")
                                .desired_width(crate::theme::ctrl_width()),
                        );
                    },
                );

                ui.add_space(2.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.verbose,
                    "--verbose",
                    Some("打印识别过程与通道细节，排查收获接口时用。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.dump_config,
                    "--dump-config",
                    Some("打印解析前的原始配置，用来确认配置有没有被正确读到。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.no_follow_brightness,
                    "--no-follow-brightness",
                    Some("不跟随键盘背光，触摸板保持自己的档位。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.allow_unverified,
                    "--allow-unverified",
                    Some("允许在未验证机型 / 布局上运行，键位可能对不上。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.grab,
                    "--grab",
                    Some("独占抓取输入设备；与 --no-grab 互斥，只能勾一个。"),
                );
                theme::check_row(
                    ui,
                    &p,
                    &mut adv.no_grab,
                    "--no-grab",
                    Some("不独占抓取设备；与 --grab 互斥。"),
                );
                theme::note(
                    ui,
                    &p,
                    "留空的项不会出现在命令行里；--grab 与 --no-grab 只能勾一个。",
                );
            });
    }

    pub(in crate::app) fn draw_command_launcher(
        &mut self,
        ui: &mut egui::Ui,
        action: &mut Option<Action>,
    ) {
        let bin_name = self
            .cmd_bin
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "numdeck".to_string());
        let preview = self.cmd.preview(&bin_name);
        let writes = self.cmd.writes_config();
        let p = self.palette;
        let can_run = self.cmd_bin.is_some() && !self.cmd_running;

        // 命令行预览：等宽字体、整行可点选复制
        egui::Frame::new()
            .fill(p.sunk)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(preview.clone()).monospace().size(12.5))
                        .wrap(),
                );
            });
        ui.add_space(6.0);

        let text = if self.cmd_running {
            "执行中……"
        } else {
            "执行"
        };
        let picked = theme::ctrl_column(ui, |ui| {
            theme::actions(
                ui,
                &p,
                &[
                    (text, true, "按上面这条命令跑一次，输出贴在下面。"),
                    ("复制命令", false, "把这条命令行复制到剪贴板。"),
                ],
            )
        });
        let _ = can_run;
        if picked == Some(0) && can_run {
            *action = Some(Action::RunCommand);
        }
        if picked == Some(1) {
            ui.ctx().copy_text(preview.clone());
        }
        if writes {
            theme::note(ui, &p, "这条子命令会写入配置文件。")
        }
    }

    pub(in crate::app) fn draw_command_output(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        if self.cmd_running {
            theme::row(
                ui,
                &p,
                "状态",
                Some("命令已提交，正在等 numdeck 进程退出并回传输出。"),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(small("等待进程返回……"));
                    });
                },
            );
        }

        let Some(output) = self.cmd_out.clone() else {
            return;
        };
        let picked = theme::row(
            ui,
            &p,
            "输出",
            Some("上一条命令的输出，标准输出与标准错误合并显示；只是界面上的文本，不影响配置。"),
            |ui| {
                theme::actions(
                    ui,
                    &p,
                    &[
                        ("复制", false, "把这段输出复制到剪贴板。"),
                        ("清空", false, "只清界面上的这段输出，不影响配置。"),
                    ],
                )
            },
        );
        if picked == Some(0) {
            ui.ctx().copy_text(output.clone());
        }
        if picked == Some(1) {
            self.cmd_out = None;
            return;
        }

        let color = if self.cmd_ok { p.text } else { p.bad };
        // 只排末尾这些行：egui 每帧都要把这段文本重新排版一遍，`dump` 转储几千行时
        // 整段渲染会让界面明显卡顿（`drain` 端已经有 512 KiB 硬上限，这里再按行收一次）。
        // 「复制」拿到的仍是完整输出。
        let shown = tail_lines(&output, VISIBLE_OUTPUT_LINES);
        egui::ScrollArea::vertical()
            .id_salt("cmd_output")
            .max_height(240.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if shown.0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "（只显示最后 {VISIBLE_OUTPUT_LINES} 行，共 {} 行；「复制」给的是完整输出）",
                            shown.2
                        ))
                        .small()
                        .color(p.text_weak),
                    );
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&shown.1)
                            .monospace()
                            .small()
                            .color(color),
                    )
                    .wrap(),
                );
            });
    }
}
