//! 背光通道：自动探测结果、手动指定、实测反馈。

use super::*;

impl App {
    /// 背光通道：灯不亮时第一个要看的地方。
    pub(super) fn draw_channel(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            egui::CollapsingHeader::new("背光通道与跟灯")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::label_cell(
                        ui,
                        &p,
                        "通道",
                        Some("背光报文走哪条链路：自动挑选，或强制 hidraw / i2c。灯不亮时先在这里换一条试。"),
                    );
                    egui::ComboBox::from_id_salt("cfg_led_kind")
                        .width(theme::ctrl_width())
                        .selected_text(self.driver.cfg.led_kind.clone())
                        .show_ui(ui, |ui| {
                            for kind in ["auto", "hidraw", "i2c", "none"] {
                                ui.selectable_value(
                                    &mut self.driver.cfg.led_kind,
                                    kind.to_string(),
                                    match kind {
                                        "auto" => "auto：按探测结果挑",
                                        "hidraw" => "hidraw：FEATURE 0x0d",
                                        "i2c" => "i2c：直接写 EC 寄存器",
                                        _ => "none：不控制背光",
                                    },
                                );
                            }
                        });
                });

                let mut hidraw = self.driver.cfg.hidraw.clone().unwrap_or_default();
                if field(
                    ui,
                    &p,
                    "hidraw 设备",
                    &mut hidraw,
                    "留空 = 自动挑选",
                    Some("指定 hidraw 节点名（如 hidraw3）；留空就按机型自动挑带 FEATURE 0x0d 的那个。"),
                )
                .changed()
                {
                    let text = hidraw.trim().to_string();
                    self.driver.cfg.hidraw = if text.is_empty() { None } else { Some(text) };
                }

                theme::row(
                    ui,
                    &p,
                    "i2c 总线",
                    Some("走 i2c 通道时的总线号，一般配合下面的从机地址一起用。"),
                    |ui| {
                        // 勾选框与数字框并排，两段加起来正好是整列宽，右边缘与别的行齐平
                        let cfg_bus = self.driver.cfg.i2c_bus;
                        let mut auto_bus = cfg_bus.is_none();
                        let mut bus = cfg_bus.unwrap_or(0);
                        let manual_bus = !auto_bus;
                        let (auto_resp, bus_resp) = theme::pair(
                            ui,
                            |ui| {
                                let resp = theme::switch(ui, &p, &mut auto_bus);
                                ui.label(egui::RichText::new("自动").size(12.5).color(p.text_weak));
                                resp
                            },
                            |ui| {
                                ui.add_enabled_ui(manual_bus, |ui| {
                                    theme::pair_num(ui, &mut bus, 0..=99)
                                })
                                .inner
                            },
                        );
                        if auto_resp.changed() {
                            self.driver.cfg.i2c_bus = if auto_bus { None } else { Some(bus) };
                        } else if bus_resp.changed() {
                            self.driver.cfg.i2c_bus = Some(bus);
                        }
                    },
                );
                theme::row(
                    ui,
                    &p,
                    "i2c 地址",
                    Some("走 i2c 通道时的从机地址，触摸板通常在 0x15 附近。"),
                    |ui| {
                        let cfg_addr = self.driver.cfg.i2c_addr;
                        let mut auto_addr = cfg_addr.is_none();
                        let mut addr = cfg_addr.unwrap_or(0x38);
                        let manual_addr = !auto_addr;
                        let (auto_resp, addr_resp) = theme::pair(
                            ui,
                            |ui| {
                                let resp = theme::switch(ui, &p, &mut auto_addr);
                                ui.label(egui::RichText::new("自动").size(12.5).color(p.text_weak));
                                resp
                            },
                            |ui| {
                                ui.add_enabled_ui(manual_addr, |ui| {
                                    ui.add_sized(
                                        egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                                        egui::DragValue::new(&mut addr)
                                            .range(0..=0x7f)
                                            .custom_formatter(|v, _| format!("0x{:02x}", v as u8)),
                                    )
                                })
                                .inner
                            },
                        );
                        if auto_resp.changed() {
                            self.driver.cfg.i2c_addr = if auto_addr { None } else { Some(addr) };
                        } else if addr_resp.changed() {
                            self.driver.cfg.i2c_addr = Some(addr);
                        }
                    },
                );
                let mut payload = self.driver.cfg.i2c_payload.clone();
                if field(
                    ui,
                    &p,
                    "i2c 载荷首字节",
                    &mut payload,
                    "0x00",
                    Some("走 i2c 时每帧的第一个字节，官方默认 0x00。"),
                )
                .changed()
                {
                    self.driver.cfg.i2c_payload = payload.trim().to_string();
                }
                let mut transfer = self.driver.cfg.i2c_transfer.clone();
                if field(
                    ui,
                    &p,
                    "i2c 工具",
                    &mut transfer,
                    "i2ctransfer",
                    Some("用来发 i2c 帧的外部命令，默认 i2ctransfer。"),
                )
                .changed()
                {
                    self.driver.cfg.i2c_transfer = transfer.trim().to_string();
                }
                theme::note(
                    ui,
                    &p,
                    "条件是 hidraw 上没有 FEATURE 0x0d（少数模组被官方驱动占掉了报告）。i2c 只在 Linux 有效。",
                );

                ui.add_space(6.0);
                theme::check_row(
                    ui,
                    &p,
                    &mut self.driver.cfg.follow_keyboard_brightness,
                    "跟随键盘背光",
                    Some("默认开启：键盘背光变几档，触摸板跟着变几档。关掉后各走各的。"),
                );
                if self.driver.cfg.follow_keyboard_brightness {
                    let mut dir = self.driver.cfg.keyboard_brightness_dir.clone();
                    if field(
                        ui,
                        &p,
                        "键盘背光目录",
                        &mut dir,
                        "/sys/class/leds/…",
                        Some("从哪个 leds 节点读键盘背光档位；机型不同节点名也不同。"),
                    )
                    .changed()
                    {
                        self.driver.cfg.keyboard_brightness_dir = dir.trim().to_string();
                    }
                    theme::row(
                        ui,
                        &p,
                        "轮询间隔",
                        Some("多久读一次键盘背光状态。调小跟得更紧，但每秒读的次数也更多。"),
                        |ui| {
                            ui.add_sized(
                                [theme::ctrl_width(), theme::CTRL_H],
                                egui::DragValue::new(&mut self.driver.cfg.brightness_poll_ms)
                                    .range(50..=5000)
                                    .suffix(" ms"),
                            );
                        },
                    );
                } else {
                    ui.label(small("不跟灯时用固定档位，见下面的手动档位表。"));
                }

                ui.add_space(6.0);
                theme::label_cell(
                    ui,
                    &p,
                    "手动档位表",
                    Some("自己填每档对应的命令字；相邻档必须单调，否则 CLI 会拒绝。"),
                );
                self.draw_manual_levels(ui, action);
            });
        });
    }
}
