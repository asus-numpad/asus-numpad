//! 扩展页：自定义布局、自定义点亮报文、压感切换、全局热键。
//!
//! 这一页专门管「官方表还没覆盖到的机型」：贴纸 / 镭雕方案自己写键表，背光报文不同的
//! 机型自己填模板，带压感的触摸板用力度切开关，键盘上再绑一个组合键当兜底。
//!
//! 四项互不依赖，改哪一项都不影响其它项：布局只影响键位，报文只影响点亮，压感与热键
//! 只是两种「不用伸手去摸触摸板」的开关方式——原生机型也可以只用其中一项。
//!
//! 界面里改的全是驱动配置（`numdeck.json`）里的 `custom_layouts` / `led_proto` /
//! `pressure_switch` / `hotkeys`，保存走驱动配置页同一套落盘逻辑，不会多出第二份文件。

use eframe::egui;

use numdeck::custom::{self, HotkeyAction, HotkeySpec, LedProtoSpec};

use super::{small, Action, App, WARN};
use crate::theme;

/// 键表固定 4 行，与 `Geometry` 的网格行数一致。
const KEY_ROWS: usize = 4;

impl App {
    /// 扩展页整体。
    ///
    /// 画完统一比对一次配置，有差异就标脏——与驱动配置页同一套「改了没保存」提示。
    pub(super) fn draw_extension(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let before = self.driver.cfg.clone();

        self.draw_ext_header(ui, action);
        ui.add_space(10.0);
        self.draw_ext_layouts(ui);
        ui.add_space(10.0);
        self.draw_ext_proto(ui);
        ui.add_space(10.0);
        self.draw_ext_pressure(ui);
        ui.add_space(10.0);
        self.draw_ext_hotkeys(ui);
        ui.add_space(10.0);
        self.draw_ext_fragment(ui);

        if self.driver.cfg != before {
            self.driver.dirty = true;
        }
    }

    /// 扩展页抬头：写完存哪、有没有改动、落盘与自检动作。
    ///
    /// 与驱动配置页同一套排法——保存类动作常驻页首。原来这张卡压在最底下，四张配置卡
    /// 一铺满就得滚到页尾才找得到「保存」，改完参数的人以为没地方存。
    fn draw_ext_header(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let p = self.palette;
        let dirty = self.driver.dirty;
        let write_path = self.driver.write_path.display().to_string();

        theme::card(ui, &p, |ui| {
            ui.horizontal(|ui| {
                theme::title_row_help(
                    ui,
                    &p,
                    "扩展",
                    Some(
                        "四项扩展与驱动共用同一个配置文件；改完要保存，键表与新报文还要重启后台服务才生效。",
                    ),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if dirty {
                        theme::pill(ui, "有改动未保存", p.warn)
                            .on_hover_text("点「保存」或「保存并重启服务」落盘");
                    } else {
                        theme::pill(ui, "已与磁盘一致", p.ok);
                    }
                });
            });
            ui.add_space(2.0);
            theme::note(ui, &p, format!("写入：{write_path}"));

            ui.add_space(8.0);
            let picked = theme::action_row(
                ui,
                &p,
                "写入",
                Some("保存＝只落盘；保存并重启＝写回配置后 systemctl --user restart。"),
                &[
                    (
                        "保存并重启服务",
                        true,
                        "写回配置文件并 systemctl --user restart；换键表、换点亮报文后必须走这个。",
                    ),
                    (
                        "保存",
                        false,
                        "只落盘，不重启后台服务；正在用的触摸板不中断。",
                    ),
                    (
                        "校验扩展项",
                        false,
                        "在副本上跑一遍解析：布局能不能装、HID 有没有和内置表打架、报文模板合不合法、压感阈值会不会被夹、热键解析得出来吗。不动磁盘也不动设备。",
                    ),
                    (
                        "放弃改动",
                        false,
                        "重新从磁盘读一遍配置，没保存的改动全部丢掉；已保存的不受影响。",
                    ),
                ],
            );
            match picked {
                Some(0) => *action = Some(Action::SaveDriverAndRestart),
                Some(1) => *action = Some(Action::SaveDriver),
                Some(2) => {
                    // 在副本上跑一遍真正的 sanitize，而不是只装布局：CLI `check` 能看到
                    // 的东西，在这里一次给全，不用先存盘再去命令行对账。
                    let mut probe = self.driver.cfg.clone();
                    let notes = probe.sanitize();
                    self.ext_notes = if notes.is_empty() {
                        vec!["全部通过：布局、HID 绑定、报文模板、压感与热键都没有问题。"
                            .to_string()]
                    } else {
                        notes
                    };
                }
                _ => {}
            }
            if picked == Some(3) {
                *action = Some(Action::ReloadDriver);
                self.ext_notes.clear();
            }
            if !self.ext_notes.is_empty() {
                ui.add_space(6.0);
                for note in self.ext_notes.clone() {
                    theme::row(ui, &p, "", None, |ui| {
                        ui.add(egui::Label::new(small(note.clone())).wrap());
                    });
                }
            }
        });
    }

    /// 卡一：自定义布局（键表 + HID 绑定）。
    fn draw_ext_layouts(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "自定义布局",
                Some(
                    "官方表里没有的机型、或者给普通触摸板贴一张小键盘贴纸、镭雕一套键位——\
                     键表按实际排布怎么写就怎么填，列数自动跟着键表走。",
                ),
            );
            ui.add_space(4.0);
            theme::note(
                ui,
                &p,
                "键名照键盘上印的写：7 8 9 +、0 period enter、esc tab back、up down left right；\
                 空位写 none（`.` 与 `-` 同样表示空位），减号写 minus，小数点写 period。\
                 列与列之间空格分隔，最多 8 列，缺的行留空即可。",
            );

            let mut remove: Option<usize> = None;
            let count = self.driver.cfg.custom_layouts.len();
            for index in 0..count {
                ui.add_space(8.0);
                // 每条布局之间用一条淡线切开，避免几条挤成一片分不清
                let rect = ui.available_rect_before_wrap();
                ui.painter()
                    .hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, p.stroke));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("#{}", index + 1))
                            .size(12.5)
                            .color(p.text_weak),
                    );
                    if theme::secondary(ui, &p, "删除这条").clicked() {
                        remove = Some(index);
                    }
                });
                draw_one_layout(ui, &p, &mut self.driver.cfg.custom_layouts[index], index);
            }
            if let Some(index) = remove {
                self.driver.cfg.custom_layouts.remove(index);
            }

            ui.add_space(8.0);
            action_row(ui, |ui| {
                if theme::secondary(ui, &p, "添加一条自定义布局").clicked() {
                    self.driver
                        .cfg
                        .custom_layouts
                        .push(new_layout(self.driver.cfg.custom_layouts.len()));
                }
            });

            // 已装上的自定义布局一览：让用户确认「写进去了」而不只是「存在配置里」
            let installed = custom::list_custom();
            if !installed.is_empty() {
                ui.add_space(6.0);
                theme::note(ui, &p, "已注册（校验后生效）：");
                for (name, hids, rows) in installed {
                    let bind = if hids.is_empty() {
                        "未绑 HID".to_string()
                    } else {
                        hids.join(" ")
                    };
                    theme::read_row(ui, &p, &bind, &format!("{name} · {} 行", rows.len()), None);
                }
            }

            // 装不上的原因一次列全：命名 / HID 冲突 + 键表解析错误
            let mut issues = layout_issues(&self.driver.cfg.custom_layouts);
            for (index, spec) in self.driver.cfg.custom_layouts.iter().enumerate() {
                if custom::keys_inherit_base(&spec.keys) {
                    continue;
                }
                if let Err(err) = custom::KeyTable::parse(&spec.keys) {
                    let tag = if spec.name.trim().is_empty() {
                        format!("#{} 未命名", index + 1)
                    } else {
                        format!("#{} {}", index + 1, spec.name.trim())
                    };
                    issues.push(format!("{tag}：{err}"));
                }
            }
            for line in issues {
                ui.add_space(2.0);
                theme::row(ui, &p, "", None, |ui| {
                    ui.label(egui::RichText::new(line).size(12.0).color(WARN));
                });
            }
        });
    }

    /// 卡二：自定义点亮报文。
    fn draw_ext_proto(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "点亮报文",
                Some(
                    "NumberPad 的背光是 FEATURE 报告中第 4 字节写亮度档；少数机型的报告 ID、\
                     前缀或尾字节不一样，这里可以按机型整条换掉。",
                ),
            );
            ui.add_space(4.0);
            let builtin = self.driver.cfg.led_proto.is_builtin();
            theme::read_row(
                ui,
                &p,
                "当前",
                if builtin {
                    "内置 0d 14 03 <level> ad"
                } else {
                    "自定义模板"
                },
                Some("内置报文走原来的快路径；自定义模板每点亮一次按档位替换占位符。"),
            );
            let resp = theme::row(
                ui,
                &p,
                "模板",
                Some("十六进制字节，空格或逗号分隔。用 ff 标记被亮度档替换的那一字节，其余原样下发。"),
                |ui| theme::text_field(ui, &mut self.ext_proto, "0d 14 03 ff ad"),
            );
            if resp.changed() {
                self.ext_proto_err = None;
                match parse_hex(&self.ext_proto) {
                    Ok(template) => {
                        let spec = LedProtoSpec { template };
                        match spec.validate() {
                            Ok(()) => {
                                self.driver.cfg.led_proto = spec;
                                self.ext_proto_dirty = true;
                            }
                            Err(err) => self.ext_proto_err = Some(err),
                        }
                    }
                    Err(err) => self.ext_proto_err = Some(err),
                }
            }
            if let Some(err) = self.ext_proto_err.clone() {
                theme::row(ui, &p, "", None, |ui| {
                    ui.label(egui::RichText::new(err).size(12.0).color(WARN));
                });
            }
            // 档位预览：把 0..4 档真会下发的字节摊开，换模板时不用靠脑补对表
            let proto = self.driver.cfg.led_proto.clone();
            if proto.validate().is_ok() {
                let levels: Vec<(u8, String)> = (0..=4u8)
                    .map(|level| {
                        let text = proto
                            .encode(level)
                            .map(|bytes| hex_of(&bytes))
                            .unwrap_or_else(|| "—".to_string());
                        (level, text)
                    })
                    .collect();
                egui::CollapsingHeader::new("各档实际下发的字节")
                    .id_salt("ext_proto_preview")
                    .default_open(!builtin)
                    .show(ui, |ui| {
                        for (level, text) in levels {
                            theme::read_row(
                                ui,
                                &p,
                                &format!("{level} 档"),
                                &text,
                                Some("这是把占位符替换成该档位后的整条报文，驱动就是照这几串字节下发的。"),
                            );
                        }
                    });
            }
            ui.add_space(2.0);
            action_row(ui, |ui| {
                ui.horizontal(|ui| {
                    if theme::secondary(ui, &p, "恢复内置报文").clicked() {
                        self.driver.cfg.led_proto = LedProtoSpec::default();
                        self.ext_proto = hex_of(&self.driver.cfg.led_proto.template);
                        self.ext_proto_err = None;
                    }
                });
            });
            // 手改配置后模板会变，输入框跟着对齐一次（用户正在打字时不动它）
            if !self.ext_proto_dirty
                && self.ext_proto != hex_of(&self.driver.cfg.led_proto.template)
            {
                self.ext_proto = hex_of(&self.driver.cfg.led_proto.template);
            }
            self.ext_proto_dirty = false;
        });
    }

    /// 卡三：压感切换（用力按一下 = 开关灯）。
    fn draw_ext_pressure(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "压感切换",
                Some(
                    "带压感（ABS_PRESSURE / ABS_MT_PRESSURE）的触摸板，可以按到某个力度就切换\
                     小键盘开关，不用去够键盘上的开关。",
                ),
            );
            ui.add_space(4.0);
            theme::check_row(
                ui,
                &p,
                &mut self.driver.cfg.pressure_switch.enabled,
                "启用",
                Some("没有压力轴的机型打开也不会误触：驱动读不到满量程就自动停用。"),
            );
            theme::row_hint(
                ui,
                &p,
                "触发阈值",
                Some("按到满量程的百分之多少就算「用力」；默认 80%。"),
                "%",
                |ui| theme::pair_num(ui, &mut self.driver.cfg.pressure_switch.percent, 1..=100),
            );
            theme::row_hint(
                ui,
                &p,
                "持续帧数",
                Some("要连续保持多少帧才算数：一帧等于芯片帧间隔，通常是 8 ms。帧数少容易误触，多则需要按得久。"),
                "帧",
                |ui| theme::pair_num(ui, &mut self.driver.cfg.pressure_switch.frames, 1..=200),
            );
            theme::row_hint(
                ui,
                &p,
                "冷却",
                Some("两次切换之间的最小间隔，防止按住不放来回翻转。"),
                "ms",
                |ui| {
                    ui.add_sized(
                        egui::Vec2::new(theme::pair_b(), theme::CTRL_H),
                        egui::DragValue::new(&mut self.driver.cfg.pressure_switch.cooldown_ms)
                            .range(0.0..=5000.0)
                            .speed(10.0),
                    )
                },
            );
            theme::note(
                ui,
                &p,
                "压感与「混合模式」互不影响：这里只改开关状态，不合成按键，也不会把压感事件转发出去。",
            );
        });
    }

    /// 卡四：全局热键。
    fn draw_ext_hotkeys(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "全局热键",
                Some(
                    "在物理键盘上绑一个组合键直接切换小键盘：触摸板贴纸方案、镭雕方案都能用，\
                     也不需要打开任何界面。",
                ),
            );
            ui.add_space(4.0);
            theme::note(
                ui,
                &p,
                "热键只旁听键盘事件，不拦截按键，正常打字不受影响；触发时也不会额外闪烁灯。",
            );

            let mut remove: Option<usize> = None;
            // 先按规整写法收一遍组合键，下面每行据此判断「有没有和别的条目撞车」
            let labels: Vec<String> = self
                .driver
                .cfg
                .hotkeys
                .iter()
                .map(|spec| hotkey_key(&spec.gesture))
                .collect();
            let count = self.driver.cfg.hotkeys.len();
            for index in 0..count {
                ui.add_space(6.0);
                theme::block(ui, &p, &format!("热键 {}", index + 1), None, |ui| {
                    let spec = &mut self.driver.cfg.hotkeys[index];
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            egui::Vec2::new(theme::ctrl_width() - 96.0, theme::CTRL_H),
                            egui::TextEdit::singleline(&mut spec.gesture).hint_text("ctrl+alt+n"),
                        );
                        if theme::secondary(ui, &p, "删除").clicked() {
                            remove = Some(index);
                        }
                    });
                    ui.horizontal(|ui| {
                        let mut action =
                            HotkeyAction::parse(&spec.action).unwrap_or(HotkeyAction::Toggle);
                        let before = action;
                        egui::ComboBox::from_id_salt(("hotkey_action", index))
                            .width(theme::pair_b())
                            .selected_text(action_label(action))
                            .show_ui(ui, |ui| {
                                for item in
                                    [HotkeyAction::Toggle, HotkeyAction::On, HotkeyAction::Off]
                                {
                                    ui.selectable_value(&mut action, item, action_label(item));
                                }
                            });
                        if action != before {
                            spec.action = action.as_str().to_string();
                        }
                    });
                    draw_hotkey_check(ui, &p, spec);
                    let mine = hotkey_key(&spec.gesture);
                    if !mine.is_empty() && labels.iter().filter(|other| **other == mine).count() > 1
                    {
                        ui.label(
                            egui::RichText::new(
                                "与其它条目重复：同一个组合键只有排在前面的那条生效",
                            )
                            .size(12.0)
                            .color(WARN),
                        );
                    }
                });
            }
            if let Some(index) = remove {
                self.driver.cfg.hotkeys.remove(index);
            }

            ui.add_space(8.0);
            action_row(ui, |ui| {
                if theme::secondary(ui, &p, "添加一个热键").clicked() {
                    self.driver.cfg.hotkeys.push(HotkeySpec {
                        gesture: String::new(),
                        action: HotkeyAction::Toggle.as_str().to_string(),
                    });
                }
            });
        });
    }

    /// 卡五：配置片段——四项打包成一段 JSON，同机型之间直接对拷。
    ///
    /// 只装四项扩展，不碰帧节拍、几何兜底、界面语言这些「换了机器就该重设」的项。
    fn draw_ext_fragment(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        theme::card(ui, &p, |ui| {
            theme::title_row_help(
                ui,
                &p,
                "配置片段",
                Some(
                    "把四项扩展打包成一段 JSON：同机型的人贴过去点「应用」就能用。\
                     只含 custom_layouts / led_proto / pressure_switch / hotkeys，\
                     不含帧节拍、几何兜底、界面语言这些按机器走的项。",
                ),
            );
            ui.add_space(4.0);
            theme::row(
                ui,
                &p,
                "片段",
                Some("点「导出」从当前配置生成；也可以直接粘贴别人给的片段。"),
                |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.ext_snippet)
                            .desired_rows(5)
                            .desired_width(theme::ctrl_width())
                            .hint_text("{ ... }"),
                    )
                },
            );
            ui.add_space(2.0);
            action_row(ui, |ui| {
                ui.horizontal(|ui| {
                    let resp = theme::secondary(ui, &p, "导出");
                    let resp = theme::tip(resp, "把当前四项扩展序列化成 JSON 文本，可直接复制走。");
                    if resp.clicked() {
                        self.ext_snippet = fragment_of(&self.driver.cfg);
                        let len = self.ext_snippet.len();
                        self.ext_snippet_msg = Some((format!("已生成 {len} 字节的片段"), false));
                    }
                    // 复制走系统剪贴板：片段是给人转发的，还得让人自己去框里选中再 Ctrl+C 太笨。
                    let resp = theme::secondary(ui, &p, "复制");
                    let resp = theme::tip(resp, "把框里的片段放进系统剪贴板，直接发给同机型的人。");
                    if resp.clicked() {
                        if self.ext_snippet.trim().is_empty() {
                            self.ext_snippet_msg =
                                Some(("框里还是空的，先点「导出」".to_string(), true));
                        } else {
                            ui.ctx().copy_text(self.ext_snippet.clone());
                            let len = self.ext_snippet.len();
                            self.ext_snippet_msg =
                                Some((format!("已复制 {len} 字节到剪贴板"), false));
                        }
                    }
                    let resp = theme::secondary(ui, &p, "存成文件");
                    let resp = theme::tip(
                        resp,
                        "把片段写成一份 .json 放进下载目录，重复导出不会覆盖上一份。",
                    );
                    if resp.clicked() {
                        self.ext_snippet_msg = Some(match save_snippet(&self.ext_snippet) {
                            Ok(path) => (format!("已写入 {}", path.display()), false),
                            Err(err) => (err, true),
                        });
                    }
                    let resp = theme::secondary(ui, &p, "应用");
                    let resp = theme::tip(
                        resp,
                        "把框里的片段写进配置；缺的项保持原样，写坏的项会明确指出是哪一段。",
                    );
                    if resp.clicked() {
                        self.ext_snippet_msg =
                            Some(apply_fragment(&mut self.driver.cfg, &self.ext_snippet));
                    }
                });
            });
            if let Some((msg, bad)) = self.ext_snippet_msg.clone() {
                ui.add_space(2.0);
                theme::row(ui, &p, "", None, |ui| {
                    ui.label(egui::RichText::new(msg).size(12.0).color(if bad {
                        WARN
                    } else {
                        p.ok
                    }));
                });
            }
        });
    }
}

/// 片段落盘：优先写进下载目录，没有这个目录就写到家目录。
///
/// 只新建、不覆盖——连点两次导出不该把上一份盖掉，那样等于没有留底。
pub(super) fn save_snippet(text: &str) -> Result<std::path::PathBuf, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("框里还是空的，先点「导出」".to_string());
    }
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "找不到家目录，没法定位保存位置".to_string())?;
    let dir = if home.join("Downloads").is_dir() {
        home.join("Downloads")
    } else {
        home
    };
    save_snippet_in(&dir, text)
}

/// 没有标签的一排控件：缩进到控件列，和带标签的行左右对齐。
///
/// 这里不用 [`theme::ctrl_column`]：它内部的格子按「剩余可用高度」开，放进滚动区的
/// 长页面里时，那次 `available_height()` 会把整行撑成整屏高——页面上看来就是一行按钮
/// 之后凭空多出一大片空白。缩进 + `horizontal` 的效果一样，行高只由控件自己决定。
fn action_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(theme::LABEL_W + theme::ROW_GAP);
        add(ui);
    });
}

/// 字节串转回可编辑的十六进制文本。
pub(super) fn hex_of(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

mod fragment;
pub(crate) use fragment::*;
mod layouts;
pub(crate) use layouts::*;
mod patch;
pub(crate) use patch::*;
mod hotkeys;
pub(crate) use hotkeys::*;
mod proto;
pub(crate) use proto::*;

#[cfg(test)]
mod tests {
    use super::*;
    use numdeck::config::Config;
    use numdeck::custom::{CustomLayoutSpec, LayoutPatch};
    use numdeck::layout::LAYOUTS;

    /// 导出再导入必须得到同一份四项配置——片段是对外分享的文件格式，不能单向。
    #[test]
    fn fragment_round_trip() {
        let source = Config {
            custom_layouts: vec![CustomLayoutSpec {
                name: "GH3X-贴纸".to_string(),
                base: "GH4XLNDZ".to_string(),
                hid_ids: vec!["ASUE1416".to_string()],
                keys: vec!["7 8 9 +".to_string(), "4 5 6 -".to_string()],
                patch: LayoutPatch {
                    columns: Some(4),
                    ..Default::default()
                },
            }],
            hotkeys: vec![HotkeySpec {
                gesture: "ctrl+alt+n".to_string(),
                action: "toggle".to_string(),
            }],
            ..Default::default()
        };

        let text = fragment_of(&source);
        let mut target = Config::default();
        let (msg, bad) = apply_fragment(&mut target, &text);
        assert!(!bad, "导入片段失败：{msg}");
        assert_eq!(target.custom_layouts, source.custom_layouts);
        assert_eq!(target.hotkeys, source.hotkeys);
        assert_eq!(target.led_proto, source.led_proto);
        assert_eq!(target.pressure_switch, source.pressure_switch);
    }

    /// 空片段、非 JSON、四项都不沾的 JSON 都要明确报错，而不是「悄悄什么都没改」。
    #[test]
    fn fragment_rejects_bad_input() {
        let mut cfg = Config::default();
        let (msg, bad) = apply_fragment(&mut cfg, "   ");
        assert!(bad && msg.contains("空"), "{msg}");

        let (msg, bad) = apply_fragment(&mut cfg, "numpad = 1");
        assert!(bad && msg.contains("JSON"), "{msg}");

        let (msg, bad) = apply_fragment(&mut cfg, r#"{"frame_ms": 8}"#);
        assert!(bad && msg.contains("四项"), "{msg}");
        assert_eq!(cfg, Config::default(), "报错时不该改动配置");
    }

    /// 报文模板不合法时整段拒绝，不能把坏模板写进配置。
    #[test]
    fn fragment_rejects_bad_led_proto() {
        let mut cfg = Config::default();
        let (msg, bad) = apply_fragment(&mut cfg, r#"{"led_proto": {"template": [13, 20]}}"#);
        assert!(bad && msg.contains("led_proto"), "{msg}");
        assert_eq!(cfg, Config::default());
    }

    /// 两组字面不同、实际同义的写法要认成同一条热键，否则重复检测形同虚设。
    #[test]
    fn hotkey_key_normalises_case_and_order() {
        assert_eq!(hotkey_key("ctrl+alt+n"), hotkey_key("Alt+Ctrl+N"));
        assert_ne!(hotkey_key("ctrl+alt+n"), hotkey_key("ctrl+shift+n"));
        assert_eq!(hotkey_key("  "), "");
    }

    /// 命名与 HID 冲突要在界面上先报出来，覆盖 `install_layouts` 会退回的几种。
    #[test]
    fn layout_issues_flags_conflicts() {
        let builtin = LAYOUTS[0].name.to_string();
        let specs = vec![
            CustomLayoutSpec {
                name: String::new(),
                ..Default::default()
            },
            CustomLayoutSpec {
                name: builtin,
                ..Default::default()
            },
            CustomLayoutSpec {
                name: "贴纸".to_string(),
                hid_ids: vec!["ASUE1416".to_string()],
                ..Default::default()
            },
            CustomLayoutSpec {
                name: "贴纸".to_string(),
                hid_ids: vec!["asue1416".to_string()],
                ..Default::default()
            },
        ];
        let issues = layout_issues(&specs);
        assert!(
            issues.iter().any(|line| line.contains("未命名")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("内置布局重名")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("重名")),
            "{issues:?}"
        );
        assert!(
            issues.iter().any(|line| line.contains("ASUE1416")),
            "{issues:?}"
        );
    }

    /// 两次导出不能互相覆盖：留底的意义就在于第一份还在。
    #[test]
    fn snippet_file_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("numpad-snippet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = save_snippet_in(&dir, "{\"a\":1}").unwrap();
        let second = save_snippet_in(&dir, "{\"a\":2}").unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "{\"a\":1}");
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "{\"a\":2}");
        assert_eq!(
            first.file_name().unwrap().to_string_lossy(),
            "numdeck-extend.json"
        );
        std::fs::remove_file(&first).ok();
        std::fs::remove_file(&second).ok();
        std::fs::remove_dir(&dir).ok();
    }

    /// 空框点「存成文件」要报错，不能悄悄写一份空文件出去。
    #[test]
    fn snippet_file_rejects_blank() {
        let dir = std::env::temp_dir().join(format!("numpad-blank-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(save_snippet_in(&dir, "   ").is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        assert!(save_snippet("").is_err());
        std::fs::remove_dir(&dir).ok();
    }
}
