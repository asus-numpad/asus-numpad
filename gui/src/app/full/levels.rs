//! 亮度档：跟随键盘背光还是手动三档，含逐档实测。

use super::*;

impl App {
    /// `manual_brightness_levels` 的编辑器：每档一个命令字，增删档都直接改表长。
    pub(super) fn draw_manual_levels(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let mut levels = self.driver.cfg.manual_brightness_levels.clone();
        let explicit = levels.is_some();
        let mut use_explicit = explicit;
        if theme::check_row(
            ui,
            &p,
            &mut use_explicit,
            "显式给出档位表（不勾则由驱动线性等分）",
            Some("勾上就自己填每档的命令字；不勾则由驱动按硬件档位线性等分。"),
        )
        .changed()
        {
            levels = if use_explicit {
                Some(vec![0x00, 0x43, 0x45, 0x48])
            } else {
                None
            };
        }

        if let Some(table) = levels.as_mut() {
            // 四格一行铺满控件列：档数变化时换行位置固定，第二行的格子与第一行对着
            theme::block(
                ui,
                &p,
                "档位表",
                Some("按 1、2、3… 的顺序每档填一个命令字，相邻档必须单调。"),
                |ui| theme::slot_grid(ui, &p, table, 0x00, 0x48),
            );
            let picked = theme::action_row(
                ui,
                &p,
                "增删档",
                Some("表长即档数（1..=8）；只有 0x00（灭）与 0x41..=0x48 是硬件真实档位，写别的数不会更亮。"),
                &[
                    ("加一档", false, "在表尾接一档，命令字接上一档往后排。"),
                    ("减一档", false, "删掉表尾一档。"),
                    ("线性四档", false, "重置成 0x00 / 0x43 / 0x45 / 0x48 四档。"),
                ],
            );
            match picked {
                Some(0) if table.len() < 8 => {
                    let last = table.last().copied().unwrap_or(0x41);
                    table.push((last + 1).min(0x48));
                }
                Some(1) if table.len() > 1 => {
                    table.pop();
                }
                Some(2) => *table = vec![0x00, 0x43, 0x45, 0x48],
                _ => {}
            }
            // 日常按键盘四档调亮度在灯光页做，这里只负责和驱动配置对齐，避免两处各改一套。
            let picked = theme::action_row(
                ui,
                &p,
                "与灯页对齐",
                Some("按键盘四档调亮度在灯光页做；这里只负责把两边档位表对齐。"),
                &[
                    (
                        "用四档填充",
                        false,
                        "把灯光页的四档映射写进这份驱动档位表。",
                    ),
                    (
                        "从驱动读回",
                        false,
                        "反过来：用驱动配置里的档位表覆盖界面。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SyncLevelsToDriver),
                Some(1) => *action = Some(Action::LoadLevelsFromDriver),
                _ => {}
            }
        }

        if levels != self.driver.cfg.manual_brightness_levels {
            self.driver.cfg.manual_brightness_levels = levels;
        }

        theme::row_hint(
            ui,
            &p,
            "固定档位序号",
            Some("固定停在这一档，不再跟随键盘背光。"),
            "不跟灯时固定亮到第几档（1..8）",
            |ui| {
                ui.add_sized(
                    [theme::ctrl_width(), theme::CTRL_H],
                    egui::DragValue::new(&mut self.driver.cfg.manual_brightness_step).range(1..=8),
                );
            },
        );
    }
}
