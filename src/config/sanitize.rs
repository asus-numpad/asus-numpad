//! 配置收敛：坏值一律回落官方默认，并逐项点名说明原因。

use super::*;

impl Config {
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut notes = Vec::new();
        // 自定义布局必须先注册：下面的 `layout_name` 解析要认得它们，
        // 否则写了自定义布局名的配置会被当成「未知布局」打回 auto。
        notes.extend(crate::custom::install_layouts(&self.custom_layouts));
        if let Err(err) = self.led_proto.validate() {
            notes.push(crate::i18n::tf(
                "note.led_proto_invalid",
                &[("err", &err.to_string())],
            ));
            self.led_proto = crate::custom::LedProtoSpec::default();
        }
        let pressure_before = self.pressure_switch;
        self.pressure_switch = self.pressure_switch.sanitized();
        if self.pressure_switch.enabled
            && (self.pressure_switch.percent != pressure_before.percent
                || self.pressure_switch.frames != pressure_before.frames)
        {
            notes.push(crate::i18n::tf(
                "note.pressure_clamped",
                &[
                    ("percent", &self.pressure_switch.percent.to_string()),
                    ("frames", &self.pressure_switch.frames.to_string()),
                ],
            ));
        }
        if self.pressure_switch.enabled {
            notes.push(crate::i18n::tf(
                "note.pressure_switch_on",
                &[
                    ("percent", &self.pressure_switch.percent.to_string()),
                    ("frames", &self.pressure_switch.frames.to_string()),
                ],
            ));
        }
        let (_state, hotkey_notes) = crate::custom::HotkeyState::new(&self.hotkeys);
        notes.extend(hotkey_notes);
        // 分区 / 校准的自洽性（与面板无关的那部分）。依赖面板尺寸的校验在几何
        // 构建处做——那里才拿得到 `ABS_MT_POSITION_*` 的实际范围。
        if !self.partition.is_unset() {
            // 与面板无关的那部分先在这里收干净：`check` 只打印 sanitize 的 notes，
            // 依赖面板的校验在 `build_geometry` 里做，那一步在 check 里看不到，
            // 所以「方向反了」「列数越界」这类能当场判掉的一律前移到这里点名。
            if self.partition.layout.trim().is_empty() {
                self.partition.layout = "auto".to_string();
            }
            if let Some(n) = self.partition.cols {
                if !(1..=8).contains(&n) {
                    notes.push(crate::i18n::tf(
                        "note.partition_cols_range",
                        &[("v", &n.to_string())],
                    ));
                    self.partition.cols = None;
                }
            }
            if let Some(n) = self.partition.rows {
                if !(1..=8).contains(&n) {
                    notes.push(crate::i18n::tf(
                        "note.partition_rows_range",
                        &[("v", &n.to_string())],
                    ));
                    self.partition.rows = None;
                }
            }
            for (what, rect) in [
                ("rect", self.partition.rect),
                ("rect_mm", self.partition.rect_mm),
            ] {
                if let Some(r) = rect {
                    if !(r[2] > r[0] && r[3] > r[1]) {
                        notes.push(crate::i18n::tf(
                            "note.partition_rect_bad",
                            &[
                                ("what", what),
                                (
                                    "vals",
                                    &format!("{:.0}, {:.0}, {:.0}, {:.0}", r[0], r[1], r[2], r[3]),
                                ),
                            ],
                        ));
                        if what == "rect" {
                            self.partition.rect = None;
                        } else {
                            self.partition.rect_mm = None;
                        }
                    }
                }
            }
            if let Some(n) = self.partition.cols {
                if !self.partition.col_ratios.is_empty() && self.partition.col_ratios.len() != n {
                    notes.push(crate::i18n::tf(
                        "note.partition_ratio_mismatch",
                        &[
                            ("what", "col_ratios"),
                            ("have", &self.partition.col_ratios.len().to_string()),
                            ("want", &n.to_string()),
                        ],
                    ));
                    self.partition.col_ratios.clear();
                }
            }
            if let Some(n) = self.partition.rows {
                if !self.partition.row_ratios.is_empty() && self.partition.row_ratios.len() != n {
                    notes.push(crate::i18n::tf(
                        "note.partition_ratio_mismatch",
                        &[
                            ("what", "row_ratios"),
                            ("have", &self.partition.row_ratios.len().to_string()),
                            ("want", &n.to_string()),
                        ],
                    ));
                    self.partition.row_ratios.clear();
                }
            }
            for what in ["col_ratios", "row_ratios"] {
                let bad = {
                    let v = if what == "col_ratios" {
                        &self.partition.col_ratios
                    } else {
                        &self.partition.row_ratios
                    };
                    v.iter().any(|r| !(r.is_finite() && *r > 0.0))
                };
                if bad {
                    notes.push(crate::i18n::tf(
                        "note.partition_ratio_bad",
                        &[("what", what)],
                    ));
                    if what == "col_ratios" {
                        self.partition.col_ratios.clear();
                    } else {
                        self.partition.row_ratios.clear();
                    }
                }
            }
            // NumLock 格越界同样与面板无关（只跟行列数有关），能判就判。
            if let (Some(cell), Some(c)) = (self.partition.numlock_cell, self.partition.cols) {
                let r = self.partition.rows.unwrap_or(1);
                if cell[0] >= r || cell[1] >= c {
                    notes.push(crate::i18n::tf(
                        "note.partition_numlock_outside",
                        &[
                            ("r", &cell[0].to_string()),
                            ("c", &cell[1].to_string()),
                            ("rows", &r.to_string()),
                            ("cols", &c.to_string()),
                        ],
                    ));
                    self.partition.numlock_cell = None;
                }
            }
            // 前面几项可能已经把它清空（比如只写了一个反序的 rect），这时别再报「已启用」。
            if !self.partition.is_unset() {
                notes.push(crate::i18n::tf(
                    "note.partition_on",
                    &[("layout", self.partition.layout.trim())],
                ));
            }
        }
        if !self.calibration.is_unset() {
            for n in self.calibration.notes() {
                notes.push(crate::i18n::tf(n.key(), &[("axis", n.axis())]));
            }
        }
        // 界面语言只接受 auto / zh-CN / zh-TW / en（大小写不敏感），其余回落 auto
        let lang_raw = self.language.trim().to_string();
        if lang_raw.is_empty() {
            self.language = "auto".to_string();
        } else if !lang_raw.eq_ignore_ascii_case("auto") {
            match crate::i18n::Lang::parse(&lang_raw) {
                Some(l) => self.language = l.code().to_string(),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.invalid_language",
                        &[("v", &lang_raw)],
                    ));
                    self.language = "auto".to_string();
                }
            }
        } else {
            self.language = "auto".to_string();
        }
        // 手动指定的机型 id：空串等于不指定；表里没有的 id 直接丢弃并提示，
        // 免得手改配置写错一个字母就让驱动再也起不来。
        if let Some(raw) = self.sku.take() {
            let id = raw.trim().to_string();
            match crate::sku::by_id(&id) {
                Some(entry) => self.sku = Some(entry.id.to_string()),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.sku_unknown",
                        &[("id", &id), ("ids", &crate::sku::known_ids().join(", "))],
                    ));
                }
            }
        }
        self.frame_ms = self.frame_ms.max(1);
        self.numlock_trigger_frames = self.numlock_trigger_frames.max(1);
        self.brightness_poll_ms = self.brightness_poll_ms.max(50);
        if self.keyboard_brightness_dir.is_empty() {
            self.keyboard_brightness_dir = crate::brightness::DEFAULT_DIR.to_string();
        }
        // 亮度档只允许落在 0x41..=0x48，越界值直接剔除（防手写配置把灯发到哨位上）
        let lo = VAL_BRIGHTNESS_BASE + 1;
        let hi = VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX;
        let before = self.brightness_levels.len();
        self.brightness_levels.retain(|v| (lo..=hi).contains(v));
        if self.brightness_levels.len() != before {
            let dropped = (before - self.brightness_levels.len()).to_string();
            notes.push(crate::i18n::tf(
                "note.levels_dropped",
                &[
                    ("n", &dropped),
                    ("lo", &format!("{lo:02x}")),
                    ("hi", &format!("{hi:02x}")),
                ],
            ));
        }
        // 自定义手动档表：非法值剔除、非递增表/空表/超长表按规则收敛，
        // 免得手改配置写成 `[0, 255]` 把灯发到哨位上，或写 20 档却没那么多物理档。
        //
        // 合法取值是 `0`（熄灭）或 `0x41..=0x48`：允许 0 是为了支持「从 0 到最大
        // 线性自定义四档」这种写法（如 `[0, 0x43, 0x46, 0x48]`），硬件侧 0 就是
        // 熄灭，与 `bright 0` 同义。
        if let Some(custom) = self.manual_brightness_levels.take() {
            let mut kept: Vec<u8> = custom
                .iter()
                .copied()
                .filter(|v| manual_level_allowed(*v))
                .collect();
            if kept.len() != custom.len() {
                let dropped = (custom.len() - kept.len()).to_string();
                notes.push(crate::i18n::tf(
                    "note.manual_levels_dropped",
                    &[
                        ("n", &dropped),
                        ("lo", &format!("{lo:02x}")),
                        ("hi", &format!("{hi:02x}")),
                    ],
                ));
            }
            if kept.len() > MANUAL_BRIGHTNESS_MAX_STEPS {
                let n = kept.len().to_string();
                let max = MANUAL_BRIGHTNESS_MAX_STEPS.to_string();
                notes.push(crate::i18n::tf(
                    "note.manual_levels_truncated",
                    &[("n", &n), ("max", &max)],
                ));
                kept.truncate(MANUAL_BRIGHTNESS_MAX_STEPS);
            }
            // 档位必须「从暗到亮」：否则 `bright 2` 可能比 `bright 1` 更暗，与「档位越大
            // 越亮」的直觉相反。允许相邻两档相等——硬件只有 8 个刻度，用户把两档设成
            // 同一个值只是看不出变化，属于合法写法（每档都可任意指定）。写错就整表丢弃，
            // 回落到默认四档（与线性等分数值相同）。
            if kept.is_empty() {
                notes.push(crate::i18n::t("note.manual_levels_reset"));
                self.manual_brightness_levels = default_manual_brightness_levels();
            } else if !kept.windows(2).all(|w| w[0] <= w[1]) {
                notes.push(crate::i18n::t("note.manual_levels_not_increasing"));
                self.manual_brightness_levels = default_manual_brightness_levels();
            } else {
                self.manual_brightness_levels = Some(kept);
            }
        }
        // 手动档上限取决于实际档数（自定义表长度可能不是 4）
        let steps = self.manual_steps();
        if self.manual_brightness_step > steps {
            let step = self.manual_brightness_step.to_string();
            let max = steps.to_string();
            notes.push(crate::i18n::tf(
                "note.manual_step_clamped",
                &[("step", &step), ("max", &max)],
            ));
            self.manual_brightness_step = steps;
        }
        let layout_raw = self.layout_name.trim().to_string();
        if layout_raw.is_empty() {
            // 空值按 auto 处理：跨机型通用靠的是按 HID 自动识别，而不是写死默认布局
            self.layout_name = "auto".to_string();
        } else if !layout_raw.eq_ignore_ascii_case("auto") {
            match crate::layout::by_name(&layout_raw) {
                Some(e) => self.layout_name = e.name.to_string(),
                None => {
                    notes.push(crate::i18n::tf(
                        "note.layout_unknown",
                        &[("v", &layout_raw)],
                    ));
                    self.layout_name = "auto".to_string();
                }
            }
        } else {
            self.layout_name = "auto".to_string();
        }
        Self::dedup_notes(&mut notes);
        notes
    }
}
