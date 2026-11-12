//! 灯光页、键位映射页、NumLock 跟随页、运行日志页。

use eframe::egui;

use crate::kbdmap;
use crate::theme::{self};

use super::super::widgets::{hex_table, small};
use super::super::{Action, App, WARN};

impl App {
    /// 灯光页：档位组 / 亮度 / 动作分三块，每块一张卡片。
    ///
    /// 亮度选择从 `Slider` 换成自绘档位条（[`theme::level_meter`]）：滑块看不到一共有
    /// 几档、也不知下一档在哪个方向，档位条一格一档，点哪格就是哪档。
    pub(in crate::app) fn draw_lighting(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let steps = self.settings.steps;
        let table = self.level_table();
        let current = self.settings.step;

        theme::section(ui, &p, "亮度档位", |ui| {
            theme::row(
                ui,
                &p,
                "档位组",
                Some("4 档＝和键盘背光的四个亮度档一一对齐；8 档＝只用触摸板自身的纯亮度分档，过渡更细。"),
                |ui| {
                let items = [
                    (
                        4u8,
                        "4 档 · 对齐键盘背光",
                        "把键盘背光的 4 个档映射到触摸板，观感与官方四档一致",
                    ),
                    (
                        8u8,
                        "8 档 · 纯亮度",
                        "只用触摸板自身的 8 档亮度，不跟键盘背光走",
                    ),
                ];
                if let Some(picked) = theme::segmented(ui, &p, "steps", steps, &items) {
                    self.settings.steps = picked;
                    if self.settings.step > picked {
                        self.settings.step = picked;
                    }
                }
            });
            ui.add_space(6.0);

            // 档位条要用到下面算好的 steps / current，所以先取一份当前值
            let value = self.settings.step;
            if let Some(picked) =
                theme::ctrl_column(ui, |ui| theme::level_meter(ui, &p, "level", value, steps))
            {
                self.settings.step = picked;
                *action = Some(Action::SendCurrent);
            }
            ui.add_space(4.0);

            let level = self.level_for_step(self.settings.step);
            theme::row(
                ui,
                &p,
                "当前档位",
                Some("界面上选中的档位，以及它真正写进 FEATURE 0x0d 的 level 字节。"),
                |ui| {
                    let text = match level {
                        Some(0) => "当前：熄灭".to_string(),
                        Some(v) => format!("当前：第 {} 档 · 0x{v:02x}", self.settings.step),
                        None => format!("当前：第 {} 档", self.settings.step),
                    };
                    ui.label(
                        egui::RichText::new(text)
                            .size(13.0)
                            .strong()
                            .color(p.accent),
                    );
                    ui.label(
                        egui::RichText::new(format!("共 {steps} 档 · 点格子直接切档"))
                            .size(12.0)
                            .color(p.text_weak),
                    );
                },
            );
            ui.add_space(2.0);
            let mapped = table
                .iter()
                .map(|v| format!("0x{v:02x}"))
                .collect::<Vec<_>>()
                .join("  ");
            theme::row(
                ui,
                &p,
                "档位 → 报文",
                Some("每档下发时用的 level 字节：0x41 最暗到 0x48 最亮，0x00 是熄灭。"),
                |ui| {
                    ui.label(
                        egui::RichText::new(mapped)
                            .size(11.5)
                            .monospace()
                            .color(p.text_weak),
                    );
                },
            );

            ui.add_space(8.0);
            theme::row(
                ui,
                &p,
                "下发",
                Some("两个动作都只碰背光，不改配置：点亮＝按当前档重发一帧，熄灭＝下发 0x00。"),
                |ui| {
                    if theme::tip(
                        theme::primary(ui, &p, "点亮"),
                        "把当前档位立刻写进背光。休眠、热插拔后 EC 丢状态时用它找回亮度。",
                    )
                    .clicked()
                    {
                        *action = Some(Action::SendCurrent);
                    }
                    if theme::tip(
                        theme::secondary(ui, &p, "熄灭"),
                        "下发 0x00 熄灯；数字键和手势不受影响。",
                    )
                    .clicked()
                    {
                        *action = Some(Action::SendOff);
                    }
                },
            );
        });

        ui.add_space(10.0);

        if steps as usize == kbdmap::KBD_STEPS {
            theme::section(ui, &p, "键盘 4 档 → 触摸板亮度", |ui| {
                self.draw_kbd_map(ui, action);
            });
        } else {
            theme::section(ui, &p, "8 档映射", |ui| {
                theme::note(
                    ui,
                    &p,
                    format!(
                        "线性等分：{mapped_note}",
                        mapped_note = table
                            .iter()
                            .map(|v| format!("0x{v:02x}"))
                            .collect::<Vec<_>>()
                            .join("  ")
                    ),
                );
                theme::note(
                    ui,
                    &p,
                    "8 档是纯亮度控制；切到 4 档即可逐档对齐键盘背光的四个亮度档。",
                );
            });
        }

        ui.add_space(10.0);
        let _ = current;
    }

    /// 键盘 4 档 → 触摸板亮度的逐档编辑器。
    ///
    /// 键盘背光只有 BIOS 给的 4 档，触摸板侧能写的是 FEATURE `0x0d` 的 level 字节
    /// （`0x00` 灭 / `0x41..=0x48` 八档，没有中间灰度）。这里把两条轴对齐：每一档
    /// 单独选触摸板档位、单独试亮，还可以整表写进驱动配置，让后台服务用同一份表。
    pub(in crate::app) fn draw_kbd_map(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::note(
            ui,
            &p,
            "键盘第 N 档对应哪个触摸板档位逐档可选；第 1 档选「熄灭」即与官方四档观感一致。",
        );

        let mut retouch_current = false;
        // 一行一档：标签在左列、下拉框与「试亮」在控件列、帧报文在说明列。四行严格对齐，
        // 不必再猜列宽——之前用 Grid 手写四列宽度，总宽比控件列宽出一截，整张表就歪在
        // 卡片左边，和上下表单对不上。
        for step in 1..=kbdmap::KBD_STEPS as u8 {
            let index = (step - 1) as usize;
            let mut level = self.settings.kbd_map[index];
            let frame = kbdmap::frame_preview(level);
            theme::row_hint(
                ui,
                &p,
                &format!("键盘第 {step} 档"),
                Some("键盘背光由 BIOS 给出的 4 个档位，这里逐档对齐触摸板档位。"),
                &frame,
                |ui| {
                    let combo_width = theme::ctrl_width() - theme::BTN_W - theme::ROW_GAP;
                    let combo = egui::ComboBox::from_id_salt(("kbd_map", step))
                        .width(combo_width)
                        .selected_text(kbdmap::level_label(level))
                        .show_ui(ui, |ui| {
                            for (value, text) in kbdmap::level_choices() {
                                ui.selectable_value(&mut level, value, text);
                            }
                        });
                    theme::tip(
                        combo.response,
                        "这一档对应的触摸板亮度；选「熄灭」即与官方四档观感一致。",
                    );
                    let send = theme::secondary(ui, &p, "试亮");
                    if theme::tip(send, format!("把第 {step} 档的亮度立刻下发一次")).clicked()
                    {
                        *action = Some(Action::SendLevel(
                            level,
                            format!("键盘第 {step} 档 → {}", kbdmap::level_label(level)),
                        ));
                    }
                },
            );

            if level != self.settings.kbd_map[index] {
                self.settings.kbd_map[index] = level;
                // 改的正好是当前档：顺手把新亮度刷上去，省一次点击
                if self.settings.on && self.settings.step == step {
                    retouch_current = true;
                }
            }
        }
        if retouch_current {
            *action = Some(Action::SendCurrent);
        }

        ui.add_space(4.0);
        theme::row(
            ui,
            &p,
            "整表操作",
            Some("三颗按钮作用在整张四档表上；写入前会先检查是否从暗到亮递增。"),
            |ui| {
                // 三颗按钮平分控件列，右边缘和上面的下拉框齐平
                let picked = theme::actions(
                    ui,
                    &p,
                    &[
                        ("恢复默认", false, "恢复成与官方四档观感一致的映射。"),
                        (
                            "写入驱动",
                            false,
                            "把这张表写进驱动的 manual_brightness_levels，后台服务随即按它映射。",
                        ),
                        ("读回驱动", false, "用驱动配置里的档位表覆盖界面上这张表。"),
                    ],
                );
                match picked {
                    Some(0) => {
                        self.settings.kbd_map = kbdmap::default_map();
                        *action = Some(Action::SendCurrent);
                    }
                    Some(1) => *action = Some(Action::SyncLevelsToDriver),
                    Some(2) => *action = Some(Action::LoadLevelsFromDriver),
                    _ => {}
                }
            },
        );

        if kbdmap::monotonic(&self.settings.kbd_map) {
            theme::note(ui, &p, "整表从暗到亮递增，可直接写入驱动配置。")
        } else {
            theme::row_hint(
                ui,
                &p,
                "校验",
                Some("驱动要求相邻档不得变暗，否则写入会被拒绝。"),
                "不是递增：写入驱动会被拒绝",
                |ui| ui.label(egui::RichText::new("未通过").size(12.5).color(WARN)),
            );
            theme::note(
                ui,
                &p,
                "当前档位表不是从暗到亮递增，驱动要求相邻档不得变暗，写入驱动配置会被拒绝。",
            );
        }
        match self.driver.cfg.manual_brightness_levels.as_ref() {
            Some(table) => {
                theme::read_row(
                    ui,
                    &p,
                    "驱动档位表",
                    &format!("{}（{} 档）", hex_table(table), table.len()),
                    Some("驱动配置里 manual_brightness_levels 的当前值，后台服务按这张表映射键盘四档。"),
                );
            }
            None => theme::note(ui, &p, "驱动配置当前未指定档位表（由驱动自己线性等分）。"),
        }
        theme::read_row(
            ui,
            &p,
            "四档报文",
            &kbdmap::frames_text(&self.settings.kbd_map),
            Some("四档依次下发的 FEATURE 0x0d 帧，写入驱动配置时就是这四行。"),
        );
    }

    pub(in crate::app) fn draw_follow(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::section(ui, &p, "联动", |ui| {
            theme::check_row(
                ui,
                &p,
                &mut self.settings.follow_numlock,
                "跟随 NumLock",
                Some("NumLock 关掉就自动熄灯，打开就恢复。状态按下面的间隔轮询一次。"),
            );
            if self.settings.follow_numlock {
                theme::row(
                    ui,
                    &p,
                    "轮询间隔",
                    Some("读 NumLock 状态的周期。调小反应更快，但每秒会多读几次 sysfs。"),
                    |ui| {
                        ui.add(
                            egui::Slider::new(&mut self.settings.poll_ms, 100..=2000)
                                .step_by(50.0)
                                .suffix(" ms"),
                        );
                    },
                );
                theme::read_row(
                    ui,
                    &p,
                    "当前状态",
                    &self.numlock_text(),
                    Some("NumLock 的实时状态，取自驱动上报；与键盘指示灯应当一致。"),
                );
                if self.sent_numlock.is_none() {
                    theme::note(ui, &p, "等待第一次状态读取……")
                }
            }
            if !self.backend.is_real() {
                let mut mock_on = self.backend.numlock().unwrap_or(true);
                if theme::check_row(
                    ui,
                    &p,
                    &mut mock_on,
                    "模拟 NumLock 状态",
                    Some("只有模拟后端才有：手动设定 NumLock 真假，用来验证联动逻辑。"),
                )
                .changed()
                {
                    self.backend.set_numlock(mock_on);
                    self.last_numlock = Some(mock_on);
                }
            }
        });
    }

    pub(in crate::app) fn draw_log(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::section(ui, &p, "报文日志", |ui| {
            let picked = theme::row(
                ui,
                &p,
                "记录",
                Some("每次下发都会记一行，用来对照硬件表现。"),
                |ui| {
                    theme::actions(
                        ui,
                        &p,
                        &[("清空", false, "清空这份日志；不影响硬件状态，也不动配置。")],
                    )
                },
            );
            if picked == Some(0) {
                self.log.clear();
            }
            if self.log.is_empty() {
                theme::note(ui, &p, "还没有下发过报文。");
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    egui::Grid::new("log")
                        .striped(true)
                        .num_columns(3)
                        .show(ui, |ui| {
                            for entry in &self.log {
                                ui.monospace(&entry.at);
                                let mark = if entry.ok { "ok" } else { "fail" };
                                ui.monospace(format!("{mark}  {}", entry.frame));
                                ui.label(small(entry.note.clone()));
                                ui.end_row();
                            }
                        });
                });
        });
    }
}
