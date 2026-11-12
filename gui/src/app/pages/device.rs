//! 设备页与顶栏、状态条。
//!
//! 只负责「把 `Snapshot` 摆出来」：设备清单、服务状态、探测结果。任何来自
//! `sysinfo::Snapshot` 的判断都留在 `snapshot` 侧，这里不重算。

use eframe::egui;

use numdeck::led_policy::{RANK_WARN_ASUS_FALLBACK, RANK_WARN_NO_FEATURE};

use crate::kbdmap;
use crate::theme::{self};

use super::super::{Action, App, BRAND};

impl App {
    /// 顶栏：品牌块 + 两级标题 + 右侧状态胶囊与「重新扫描」。
    ///
    /// 原来是一行 `heading + 小字 + 按钮`，层级挤在一起；这里把「这是什么工具」
    /// 「当前连的是哪块硬件」拆成两行，右侧状态改用胶囊，一眼能扫到。
    pub(in crate::app) fn draw_header(&mut self, root: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let real = self.backend.is_real();
        let backend_name = self.backend.name().to_string();
        let bar = egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(20, 12)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    theme::logo(ui, &p, 26.0);
                    ui.add_space(9.0);
                    ui.vertical(|ui| {
                        // 与官网页头同构：品牌名 + 版本角标一行，下面一行小字交代
                        // 「这是哪个界面、连着哪块硬件」。
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 7.0;
                            ui.label(egui::RichText::new(BRAND).size(15.5).strong().color(p.text));
                            theme::tag(ui, &p, concat!("v", env!("CARGO_PKG_VERSION")))
                                .on_hover_text("界面版本；和驱动库版本对不上说明装的是混搭包");
                        });
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {}",
                                self.window_title(),
                                self.state.subtitle
                            ))
                            .size(11.5)
                            .color(p.text_weak),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let rescan = theme::tip(
                            theme::secondary(ui, &p, "重新扫描"),
                            "重新枚举 HID 接口：热插拔设备、或刚装上官方驱动之后点它。",
                        );
                        if rescan.clicked() {
                            *action = Some(Action::Rescan);
                        }
                        let (dot, tint) = if real {
                            ("●  ", p.ok)
                        } else {
                            ("●  ", p.warn)
                        };
                        theme::pill(
                            ui,
                            &format!("{dot}{backend_name}"),
                            if real { p.ok } else { p.warn },
                        )
                        .on_hover_text(if real {
                            "已连到真实 HID 接口"
                        } else {
                            "模拟后端：报文只进日志，不会碰硬件"
                        });
                        let _ = tint;
                    });
                });
            });
        // 顶栏与内容区之间只放一条极淡的分隔线：比给面板加描边更轻，
        // 但足以让白色顶栏和灰色内容底分属两层。
        let r = bar.response.rect;
        root.painter()
            .hline(r.x_range(), r.bottom(), egui::Stroke::new(1.0, p.stroke));
    }

    /// 左侧接口列表：每个接口一张小卡，选中态用强调色淡底 + 描边。
    ///
    /// 原来是 `selectable_label` 加一堆紧跟的小字，条目之间只隔一条细线，多个接口时
    /// 很难看出「哪一张卡是这台设备」。改成卡片后边界清楚，路径这类长文本可以直接折行。
    pub(in crate::app) fn draw_devices(&mut self, root: &mut egui::Ui) {
        let p = self.palette;
        let mut clicked: Option<usize> = None;
        egui::Panel::left("devices")
            .resizable(true)
            .default_size(340.0)
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(16, 14)),
            )
            .show(root, |ui| {
                theme::title_row_help(
                    ui,
                    &p,
                    "HID 接口",
                    Some("驱动通过 FEATURE 0x0d 控制的那一路 HID 接口；同一台机器上出现多个时，选带 NumberPad 小键盘的那个。"),
                );
                theme::note(
                    ui,
                    &p,
                    format!("{} · 共 {} 个接口", self.backend.name(), self.devices.len()),
                );
                ui.add_space(4.0);

                if !self.backend.is_real() {
                    theme::pill(ui, "模拟后端", p.warn).on_hover_text("报文只进日志，不会碰硬件");
                    ui.add_space(4.0);
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.devices.is_empty() {
                            ui.label(egui::RichText::new("没有枚举到任何 HID 接口").color(p.text));
                            // 两个平台没枚举到接口的原因不同，提示跟着后端走
                            let hint = if cfg!(windows) {
                                "确认 ASUS 官方驱动已安装、设备未被其它程序独占。"
                            } else {
                                "确认触摸板带 FEATURE 0x0d、hidraw 已加载，且 udev 规则已生效。"
                            };
                            theme::note(ui, &p, hint);
                            return;
                        }
                        for (i, d) in self.devices.iter().enumerate() {
                            let selected = self.selected == Some(i);
                            let name = if d.product.is_empty() {
                                d.tail().to_string()
                            } else {
                                d.product.clone()
                            };
                            let card = egui::Frame::new()
                                // 选中态不再画整圈描边：侧栏条目改用「淡底 + 左侧色条」表达选中，
                                // 像系统设置的侧边栏，条目之间也就不需要边框来分隔
                                .fill(if selected {
                                    p.accent_soft
                                } else {
                                    egui::Color32::TRANSPARENT
                                })
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(12, 9))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    if selected {
                                        // 色条先画、文字后画，色条只占左侧 2.5px，不会压到内容
                                        let bar = egui::Rect::from_min_size(
                                            egui::pos2(
                                                ui.max_rect().left() - 9.0,
                                                ui.max_rect().top() + 2.0,
                                            ),
                                            egui::Vec2::new(
                                                2.5,
                                                (ui.available_height() - 4.0).max(18.0),
                                            ),
                                        );
                                        ui.painter().rect_filled(
                                            bar,
                                            egui::CornerRadius::same(2),
                                            p.accent,
                                        );
                                    }
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} · {}",
                                            d.vid_pid(),
                                            d.vendor()
                                        ))
                                        .size(13.0)
                                        .strong()
                                        .color(p.text),
                                    );
                                    ui.label(
                                        egui::RichText::new(name).size(12.0).color(p.text_weak),
                                    );
                                    let (tint, text) = if d.usable() {
                                        (
                                            p.ok,
                                            format!(
                                                "FEATURE 0x0d 可用 · 报告 {} 字节",
                                                d.feature_len
                                            ),
                                        )
                                    } else {
                                        (p.warn, "无可用 FEATURE 0x0d".to_string())
                                    };
                                    ui.label(egui::RichText::new(text).size(11.5).color(tint));
                                    match d.rank {
                                        Some(RANK_WARN_NO_FEATURE) => {
                                            ui.label(
                                                egui::RichText::new(
                                                    "优先级：兜底（缺 FEATURE 报告）",
                                                )
                                                .size(11.5)
                                                .color(p.warn),
                                            );
                                        }
                                        Some(RANK_WARN_ASUS_FALLBACK) => {
                                            ui.label(
                                                egui::RichText::new(
                                                    "优先级：ASUS 兜底（OMNI RECEIVER）",
                                                )
                                                .size(11.5)
                                                .color(p.text_weak),
                                            );
                                        }
                                        _ => {}
                                    }
                                    ui.label(
                                        egui::RichText::new(d.path.clone())
                                            .size(11.0)
                                            .color(p.text_weak),
                                    );
                                })
                                .response;
                            let resp =
                                ui.interact(card.rect, card.id.with("row"), egui::Sense::click());
                            // 卡片里的路径与产品名在窄侧栏里会被截断，悬浮一次给全。
                            let resp = theme::tip(
                                resp,
                                format!(
                                    "{} · {}\n{}\n{}",
                                    d.vid_pid(),
                                    d.vendor(),
                                    if d.product.is_empty() {
                                        d.tail().to_string()
                                    } else {
                                        d.product.clone()
                                    },
                                    d.path
                                ),
                            );
                            if resp.clicked() {
                                clicked = Some(i);
                            }
                            ui.add_space(8.0);
                        }
                    });
            });
        if let Some(i) = clicked {
            self.selected = Some(i);
            self.settings.device_path = Some(self.devices[i].path.clone());
            let label = format!(
                "已选中 {}（{}）",
                self.devices[i].vid_pid(),
                self.devices[i].vendor()
            );
            self.set_status(label, !self.devices[i].usable());
        }
    }

    pub(in crate::app) fn draw_status(&mut self, root: &mut egui::Ui) {
        let p = self.palette;
        let bar = egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(20, 8)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    let color = if self.status_bad { p.bad } else { p.text_weak };
                    ui.label(
                        egui::RichText::new(self.status.clone())
                            .size(12.5)
                            .color(color),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // 状态栏常显「键盘第几档 → 哪个亮度值」，看一眼就知道当前映射对不对。
                        // 左侧状态文字可能很长（报文 + 说明），这里只放最短形式，避免被挤出窗口。
                        let level = self.level_for_step(self.settings.step);
                        let text = match (self.settings.steps as usize == kbdmap::KBD_STEPS, level)
                        {
                            (true, Some(value)) => format!(
                                "键盘 {}/{} → 0x{value:02x}",
                                self.settings.step,
                                kbdmap::KBD_STEPS
                            ),
                            (false, Some(value)) => {
                                format!("{}/8 → 0x{value:02x}", self.settings.step)
                            }
                            (_, None) => {
                                format!("档位 {}/{}", self.settings.step, self.settings.steps)
                            }
                        };
                        theme::pill(ui, &text, p.text_weak).on_hover_text(
                            "当前生效的档位与下发值：键盘第几档对应 FEATURE 0x0d 的 level 字节",
                        );
                    });
                });
            });
        let r = bar.response.rect;
        root.painter()
            .hline(r.x_range(), r.top(), egui::Stroke::new(1.0, p.stroke));
    }

    pub(in crate::app) fn report_open(
        &mut self,
        result: std::io::Result<()>,
        dir: &std::path::Path,
    ) {
        match result {
            Ok(()) => self.set_status(format!("已打开 {}", dir.display()), false),
            Err(err) => self.set_status(format!("打不开 {}：{err}", dir.display()), true),
        }
    }
}
