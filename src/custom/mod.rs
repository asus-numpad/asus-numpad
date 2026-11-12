//! 自定义扩展层：自定义布局（几何 + 键表）、自定义 LED 报文、压感切换、全局热键。
//!
//! 官方布局表（[`crate::layout::LAYOUTS`]）只覆盖 INF 里出现过的机型：一旦遇到
//! 没收录的触摸板模组，或者用户想在没有镭雕的普通触摸板上「贴一张小键盘贴纸」，
//! 就只剩改源码一条路。本模块把这三件事从代码里搬进配置：
//!
//! * [`CustomLayoutSpec`]：一份布局 = 15 个几何参数（可只写与基准布局不同的项）
//!   + 4 行键表 + 可选绑定的 HID 模组 ID；
//! * [`LedProtoSpec`]：点亮报文模板，`0xff` 是「亮度字节」的占位；
//! * [`PressureSwitch`]：压感触摸板用力按压 → 切换数字键盘开关；
//! * [`HotkeySpec`]：物理键盘上的全局组合键 → 切换 / 开 / 关。
//!
//! 键表与报文都是纯数据，因此可以在非 Linux 平台跑单测。

mod keys;
pub use keys::*;
mod table;
pub use table::*;
mod spec;
pub use spec::*;
mod led;
pub use led::*;
mod pressure;
pub use pressure::*;
mod hotkey;
pub use hotkey::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keycodes as kc;
    use crate::layout::{self, GH3XLNDZ, ROWS};
    use crate::layout::{KeyMode, Layout};

    /// 自定义布局注册表是进程级全局状态，测试并行跑会互相清空，必须串行。
    fn registry_lock() -> std::sync::MutexGuard<'static, ()> {
        crate::test_lock::registry()
    }

    #[test]
    fn key_code_aliases() {
        assert_eq!(key_code("7"), Some(kc::KEY_7));
        assert_eq!(key_code("kp7"), Some(kc::KEY_KP7));
        assert_eq!(key_code("KP_7"), Some(kc::KEY_KP7));
        assert_eq!(key_code("kp-dot"), Some(kc::KEY_KPDOT));
        assert_eq!(key_code("F13"), Some(kc::KEY_F13));
        assert_eq!(key_code("backspace"), Some(kc::KEY_BACKSPACE));
        assert_eq!(key_code("volup"), Some(kc::KEY_VOLUMEUP));
        assert_eq!(key_code("0x64"), Some(0x64));
        assert_eq!(key_code("296"), Some(296));
        assert_eq!(key_code("nope"), None);
    }

    #[test]
    fn key_label_roundtrip() {
        for code in [
            kc::KEY_KP7,
            kc::KEY_KPENTER,
            kc::KEY_BACKSPACE,
            kc::KEY_F13,
            kc::KEY_7,
            0x123,
        ] {
            let label = key_label(code);
            let back = key_code(&label).unwrap_or_else(|| {
                // F13 / 数字 / kp7 都能解回；未知键码走 `0x..` 形式
                assert!(label.starts_with("0x"), "{label}");
                u16::from_str_radix(&label[2..], 16).unwrap()
            });
            assert_eq!(back, code, "label {label}");
        }
    }

    #[test]
    fn keytable_parse_and_lookup() {
        const BASE: Layout = GH3XLNDZ;
        let rows = vec![
            "7 8 9 slash .".to_string(),
            "4 5 6 asterisk backspace".to_string(),
            "1 2 3 minus enter".to_string(),
            "0 0 period plus enter".to_string(),
        ];
        let t = KeyTable::parse(&rows).unwrap();
        assert_eq!(t.cols(), 5);
        assert_eq!(t.key_at(0, 0, KeyMode::Keypad), Some(kc::KEY_KP7));
        assert_eq!(t.key_at(0, 0, KeyMode::Digits), Some(kc::KEY_7));
        assert_eq!(t.key_at(2, 4, KeyMode::Keypad), Some(kc::KEY_KPENTER));
        // 内置表与手写同构表在关键格上一致
        let builtin = builtin_table();
        for row in 0..ROWS {
            for col in 0..builtin.cols() {
                assert_eq!(
                    builtin.key_at(row, col, KeyMode::Keypad),
                    t.key_at(row, col, KeyMode::Keypad),
                    "cell ({row},{col})"
                );
            }
        }
        let _ = BASE;
    }

    #[test]
    fn keytable_accepts_fixed_codes_and_gaps() {
        let t = KeyTable::parse(&[
            "kp1 kp2 kp3 . F13".to_string(),
            "kp4 kp5 kp6 . 0xAA".to_string(),
        ])
        .unwrap();
        assert_eq!(t.cols(), 5);
        assert_eq!(t.key_at(0, 0, KeyMode::Digits), Some(kc::KEY_KP1));
        assert_eq!(t.key_at(0, 3, KeyMode::Keypad), None);
        assert_eq!(t.key_at(0, 4, KeyMode::Keypad), Some(kc::KEY_F13));
        assert_eq!(t.key_at(1, 4, KeyMode::Keypad), Some(0xAA));
        // 不足 4 行的补齐为空行
        assert_eq!(t.key_at(3, 0, KeyMode::Keypad), None);
    }

    /// 符号键允许照键盘上的样子直接写：`+` `*` `/`，省得用户先去查一遍英文名。
    /// `-` 仍是空位（与 `describe` 打印空位一致），减号只能写 `minus`。
    #[test]
    fn keytable_accepts_printed_symbols() {
        let t = KeyTable::parse(&[
            "7 8 9 +".to_string(),
            "4 5 6 *".to_string(),
            "1 2 3 /".to_string(),
            "0 . minus -".to_string(),
        ])
        .unwrap();
        assert_eq!(t.key_at(0, 3, KeyMode::Keypad), Some(kc::KEY_KPPLUS));
        assert_eq!(t.key_at(1, 3, KeyMode::Keypad), Some(kc::KEY_KPASTERISK));
        assert_eq!(t.key_at(2, 3, KeyMode::Keypad), Some(kc::KEY_KPSLASH));
        assert_eq!(t.key_at(3, 2, KeyMode::Keypad), Some(kc::KEY_KPMINUS));
        // `-` 与 `.` 都当空位
        assert_eq!(t.key_at(3, 3, KeyMode::Keypad), None);
        assert_eq!(t.key_at(3, 1, KeyMode::Keypad), None);
        // 打印回来仍是英文名，能再解析回去
        let again = KeyTable::parse(&t.describe()).unwrap();
        assert_eq!(again, t);
    }

    #[test]
    fn keytable_rejects_bad_input() {
        assert!(KeyTable::parse(&[]).is_err());
        assert!(KeyTable::parse(&["1 2".into(), "1 2 3".into()]).is_err());
        assert!(KeyTable::parse(&["1 bogus 3".into()]).is_err());
        assert!(KeyTable::parse(&["1 2 3 4 5 6 7 8 9".into()]).is_err());
    }

    #[test]
    fn layout_patch_overrides_only_written_fields() {
        let patch = LayoutPatch {
            columns: Some(4),
            left: Some(20),
            ..Default::default()
        };
        let l = patch.apply(GH3XLNDZ);
        assert_eq!(l.columns, 4);
        assert_eq!(l.left, 20);
        assert_eq!(l.index0, GH3XLNDZ.index0);
    }

    #[test]
    fn custom_layout_registers_and_resolves() {
        let _guard = registry_lock();
        let spec = CustomLayoutSpec {
            name: "PASTE40".to_string(),
            hid_ids: vec!["asue9999".to_string()],
            keys: vec![
                "7 8 9 slash".to_string(),
                "4 5 6 asterisk".to_string(),
                "1 2 3 minus".to_string(),
                "0 period plus enter".to_string(),
            ],
            patch: LayoutPatch {
                columns: Some(4),
                ..Default::default()
            },
            ..Default::default()
        };
        let notes = install_layouts(&[spec]);
        assert!(notes.iter().all(|n| !n.contains("ignored")), "{notes:?}");
        let entry = entry_by_name("PASTE40").expect("registered");
        assert_eq!(entry.layout.columns, 4);
        assert_eq!(entry_by_hid("ASUE9999").map(|e| e.name), Some("PASTE40"));
        let table = table_for("PASTE40");
        assert_eq!(table.cols(), 4);
        assert_eq!(table.key_at(0, 0, KeyMode::Keypad), Some(kc::KEY_KP7));
        // 内置布局仍然解析到静态表
        assert_eq!(table_for("GH3XLNDZ").cols(), 5);
        assert!(layout::by_name("PASTE40").is_some());
    }

    #[test]
    fn custom_layout_rejects_builtin_name() {
        let _guard = registry_lock();
        let notes = install_layouts(&[CustomLayoutSpec {
            name: "GH3XLNDZ".to_string(),
            keys: vec!["1 2 3 4 5".to_string()],
            ..Default::default()
        }]);
        // 文案走 i18n（默认英文），断言用文案里的关键词而不是整句
        assert!(
            notes
                .iter()
                .any(|n| n.contains("clashes") && n.contains("GH3XLNDZ")),
            "{notes:?}"
        );
    }

    #[test]
    fn proto_default_matches_builtin_report() {
        let spec = LedProtoSpec::default();
        assert!(spec.is_builtin());
        assert!(spec.validate().is_ok());
        assert_eq!(
            spec.encode(0x45),
            crate::led_proto::encode(0x45).map(|b| b.to_vec())
        );
        assert_eq!(spec.encode(0x60), None);
    }

    #[test]
    fn proto_custom_template() {
        let spec = LedProtoSpec {
            template: vec![0x0a, 0x01, LEVEL_MARK, 0x00],
        };
        assert!(!spec.is_builtin());
        assert!(spec.validate().is_ok());
        assert_eq!(spec.encode(0x43), Some(vec![0x0a, 0x01, 0x43, 0x00]));
        assert!(LedProtoSpec {
            template: vec![0x0a, 0x01, 0x42]
        }
        .validate()
        .is_err());
        assert!(LedProtoSpec {
            template: vec![LEVEL_MARK, LEVEL_MARK]
        }
        .validate()
        .is_err());
    }

    #[test]
    fn pressure_gate_needs_frames_and_rearms() {
        let mut gate = PressureGate::new(PressureSwitch {
            enabled: true,
            percent: 80,
            frames: 3,
            cooldown_ms: 500.0,
        });
        let thr = 800; // max 1000 * 80%
        assert!(!gate.feed(thr + 10, 1000, true, 0.0));
        assert!(!gate.feed(thr + 10, 1000, true, 8.0));
        assert!(gate.feed(thr + 10, 1000, true, 16.0));
        // 按住不放不会再触发
        assert!(!gate.feed(thr + 10, 1000, true, 24.0));
        assert!(!gate.feed(thr + 10, 1000, true, 1000.0));
        // 松开压力后重新武装，冷却后再触发
        assert!(!gate.feed(0, 1000, true, 1010.0));
        assert!(!gate.feed(thr + 10, 1000, true, 1018.0));
        assert!(!gate.feed(thr + 10, 1000, true, 1026.0));
        assert!(gate.feed(thr + 10, 1000, true, 1034.0));
        assert_eq!(gate.fired(), 2);
    }

    #[test]
    fn pressure_gate_ignores_pressureless_panels() {
        let mut gate = PressureGate::new(PressureSwitch {
            enabled: true,
            ..Default::default()
        });
        assert!(!gate.feed(9999, 0, true, 0.0));
        let mut off = PressureGate::new(PressureSwitch::default());
        assert!(!off.feed(9999, 1000, true, 0.0));
    }

    #[test]
    fn hotkey_parse_and_match() {
        let hk = Hotkey::parse("ctrl+alt+n").unwrap();
        assert_eq!(hk.mods, MOD_CTRL | MOD_ALT);
        assert_eq!(hk.code, kc::KEY_N);
        assert_eq!(hk.label(), "ctrl+alt+n");
        let (mut state, notes) = HotkeyState::new(&[HotkeySpec {
            gesture: "ctrl+alt+n".to_string(),
            action: "toggle".to_string(),
        }]);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(state.feed(kc::KEY_LEFTCTRL, 1), None);
        assert_eq!(state.feed(kc::KEY_LEFTALT, 1), None);
        assert_eq!(state.feed(kc::KEY_N, 1), Some(HotkeyAction::Toggle));
        // 按住不放不重复触发
        assert_eq!(state.feed(kc::KEY_N, 2), None);
        assert_eq!(state.feed(kc::KEY_N, 1), Some(HotkeyAction::Toggle));
        state.feed(kc::KEY_N, 0);
        state.feed(kc::KEY_LEFTALT, 0);
        assert_eq!(state.feed(kc::KEY_N, 1), None); // 缺 ctrl
    }

    #[test]
    fn hotkey_rejects_bad_specs() {
        assert!(Hotkey::parse("ctrl+").is_err());
        assert!(Hotkey::parse("ctrl+alt").is_err());
        assert!(Hotkey::parse("n+m").is_err());
        let (state, notes) = HotkeyState::new(&[HotkeySpec {
            gesture: "bogus+x".to_string(),
            action: "toggle".to_string(),
        }]);
        assert!(state.is_empty());
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn custom_layout_inherits_base_keys() {
        let _guard = registry_lock();
        // 只改几何、键位照抄：keys 留空必须合法，且沿用基准那张表
        let spec = CustomLayoutSpec {
            name: "STICKER-3X4".to_string(),
            base: crate::layout::DEFAULT_LAYOUT_NAME.to_string(),
            hid_ids: vec![" asue9999 ".to_string(), String::new()],
            keys: Vec::new(),
            ..Default::default()
        };
        let notes = install_layouts(&[spec]);
        assert!(notes.is_empty(), "{notes:?}");

        let entry = entry_by_name("STICKER-3X4").expect("应当注册成功");
        assert_eq!(entry.layout.columns as usize, builtin_table().cols());
        let table = table_for("STICKER-3X4");
        assert_eq!(table.describe(), builtin_table().describe());
        // HID 绑定的空白项被丢掉，其余转大写
        assert_eq!(
            entry_by_hid("ASUE9999").map(|e| e.name),
            Some("STICKER-3X4")
        );
        assert!(entry_by_hid("").is_none());

        // 全是空白行也算留空
        assert!(keys_inherit_base(&["".to_string(), "   ".to_string()]));
        assert!(!keys_inherit_base(&["7 8 9".to_string()]));
        install_layouts(&[]);
    }

    #[test]
    fn custom_layout_chain_inherits_previous_custom_table() {
        let _guard = registry_lock();
        let first = CustomLayoutSpec {
            name: "CHAIN-A".to_string(),
            base: crate::layout::DEFAULT_LAYOUT_NAME.to_string(),
            keys: vec!["kp7 kp8 kp9".to_string(), "kp4 kp5 kp6".to_string()],
            ..Default::default()
        };
        let second = CustomLayoutSpec {
            name: "CHAIN-B".to_string(),
            base: "CHAIN-A".to_string(),
            keys: Vec::new(),
            ..Default::default()
        };
        let notes = install_layouts(&[first, second]);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(
            table_for("CHAIN-B").describe(),
            table_for("CHAIN-A").describe()
        );
        assert_eq!(table_for("CHAIN-B").cols(), 3);
        install_layouts(&[]);
    }

    #[test]
    fn custom_layout_notes_bad_shapes() {
        let _guard = registry_lock();
        let ragged = CustomLayoutSpec {
            name: "RAGGED".to_string(),
            base: crate::layout::DEFAULT_LAYOUT_NAME.to_string(),
            keys: vec!["7 8 9".to_string(), "4 5".to_string()],
            ..Default::default()
        };
        let too_wide = CustomLayoutSpec {
            name: "WIDE".to_string(),
            base: crate::layout::DEFAULT_LAYOUT_NAME.to_string(),
            keys: vec!["7 8 9 4 5 6 1 2 3".to_string()],
            ..Default::default()
        };
        let no_base = CustomLayoutSpec {
            name: "ORPHAN".to_string(),
            base: "NOPE".to_string(),
            ..Default::default()
        };
        let notes = install_layouts(&[ragged, too_wide, no_base]);
        assert_eq!(notes.len(), 3, "{notes:?}");
        assert!(entry_by_name("RAGGED").is_none());
        assert!(entry_by_name("WIDE").is_none());
        assert!(entry_by_name("ORPHAN").is_none());
        install_layouts(&[]);
    }

    #[test]
    fn proto_validate_bounds() {
        // 空模板 / 超长模板 / 缺占位 / 占位落在首字节（report id）
        assert!(LedProtoSpec { template: vec![] }.validate().is_err());
        assert!(LedProtoSpec {
            template: vec![0x0d; 65]
        }
        .validate()
        .is_err());
        assert!(LedProtoSpec {
            template: vec![0x0d, 0x14, 0x03, 0xad]
        }
        .validate()
        .is_err());
        assert!(LedProtoSpec {
            template: vec![LEVEL_MARK, 0x14, 0x03, LEVEL_MARK]
        }
        .validate()
        .is_err());
        // 恰好 64 字节是上限，可以过
        let mut max = vec![0x0d; 64];
        max[63] = LEVEL_MARK;
        assert!(LedProtoSpec { template: max }.validate().is_ok());
        // 内置模板的 is_builtin 只在完全一致时为真
        let mut tweaked = LedProtoSpec::default().template;
        tweaked[4] = 0x00;
        assert!(!LedProtoSpec { template: tweaked }.is_builtin());
    }

    #[test]
    fn proto_encode_covers_every_step() {
        let spec = LedProtoSpec {
            template: vec![0x0a, 0x01, LEVEL_MARK, 0x00],
        };
        let mut seen = std::collections::HashSet::new();
        for value in 0x40..=0x4f {
            let Some(bytes) = spec.encode(value) else {
                continue;
            };
            assert_eq!(bytes[2], crate::led_proto::level_byte(value).unwrap());
            seen.insert(bytes[2]);
        }
        assert!(seen.len() >= 8, "亮度档位没有真正写进占位符：{seen:?}");
    }

    #[test]
    fn pressure_switch_sanitizes_bounds() {
        let low = PressureSwitch {
            enabled: true,
            percent: 0,
            frames: 0,
            cooldown_ms: f64::NAN,
        }
        .sanitized();
        assert_eq!(low.percent, 1);
        assert_eq!(low.frames, 1);
        assert_eq!(low.cooldown_ms, 600.0);

        let high = PressureSwitch {
            enabled: true,
            percent: 250,
            frames: 9999,
            cooldown_ms: -5.0,
        }
        .sanitized();
        assert_eq!(high.percent, 100);
        assert_eq!(high.frames, 200);
        assert_eq!(high.cooldown_ms, 600.0);
    }

    #[test]
    fn pressure_gate_cooldown_and_contact() {
        let mut gate = PressureGate::new(PressureSwitch {
            enabled: true,
            percent: 50,
            frames: 1,
            cooldown_ms: 300.0,
        });
        assert!(gate.feed(600, 1000, true, 0.0));
        // 抬起压力重新武装，但冷却期未过 → 不触发
        assert!(!gate.feed(0, 1000, true, 100.0));
        assert!(!gate.feed(600, 1000, true, 100.0));
        // 触点离开也会重新武装
        assert!(!gate.feed(0, 1000, false, 200.0));
        assert!(!gate.feed(600, 1000, true, 250.0));
        assert!(gate.feed(600, 1000, true, 300.0));
        assert_eq!(gate.fired(), 2);
    }

    #[test]
    fn hotkey_modifier_order_is_irrelevant() {
        let a = Hotkey::parse("alt+ctrl+n").unwrap();
        let b = Hotkey::parse("ctrl+alt+n").unwrap();
        let c = Hotkey::parse("N+CTRL+ALT").unwrap();
        assert_eq!(a, b);
        assert_eq!(b, c);
        assert_eq!(a.label(), "ctrl+alt+n");
        // 重复写同一个主键不算错，也不改变结果
        assert_eq!(Hotkey::parse("ctrl+n+n").unwrap().code, kc::KEY_N);
        assert_eq!(Hotkey::parse("n").unwrap().mods, 0);
    }

    #[test]
    fn hotkey_action_aliases() {
        assert_eq!(HotkeyAction::parse("").unwrap(), HotkeyAction::Toggle);
        assert_eq!(HotkeyAction::parse(" ON ").unwrap(), HotkeyAction::On);
        assert_eq!(HotkeyAction::parse("enable").unwrap(), HotkeyAction::On);
        assert_eq!(HotkeyAction::parse("off").unwrap(), HotkeyAction::Off);
        assert_eq!(HotkeyAction::parse("disable").unwrap(), HotkeyAction::Off);
        assert!(HotkeyAction::parse("reboot").is_err());
        assert_eq!(HotkeyAction::On.as_str(), "on");
    }

    #[test]
    fn hotkey_state_reset_and_unknown_action_note() {
        let (mut state, notes) = HotkeyState::new(&[
            HotkeySpec {
                gesture: "ctrl+alt+n".to_string(),
                action: "toggle".to_string(),
            },
            HotkeySpec {
                gesture: "ctrl+alt+m".to_string(),
                action: "reboot".to_string(),
            },
            HotkeySpec {
                gesture: "  ".to_string(),
                action: "toggle".to_string(),
            },
        ]);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(state.describe().len(), 1);
        // 切 tty 后残留的按下态必须能被清掉，否则第一次按键会误触发
        state.feed(kc::KEY_LEFTCTRL, 1);
        state.feed(kc::KEY_LEFTALT, 1);
        state.clear();
        assert_eq!(state.feed(kc::KEY_N, 1), None);
        state.feed(kc::KEY_N, 0);
        state.feed(kc::KEY_LEFTCTRL, 1);
        state.feed(kc::KEY_LEFTALT, 1);
        assert_eq!(state.feed(kc::KEY_N, 1), Some(HotkeyAction::Toggle));
    }

    #[test]
    fn hotkey_ignores_repeat_and_release() {
        let (mut state, _) = HotkeyState::new(&[HotkeySpec {
            gesture: "ctrl+shift+k".to_string(),
            action: "on".to_string(),
        }]);
        state.feed(kc::KEY_LEFTCTRL, 1);
        state.feed(kc::KEY_LEFTSHIFT, 1);
        assert_eq!(state.feed(kc::KEY_K, 1), Some(HotkeyAction::On));
        // 长按重复事件不重复触发
        for _ in 0..5 {
            assert_eq!(state.feed(kc::KEY_K, 2), None);
        }
        // 抬起主键后可以再次触发
        state.feed(kc::KEY_K, 0);
        assert_eq!(state.feed(kc::KEY_K, 1), Some(HotkeyAction::On));
        assert_eq!(state.feed(kc::KEY_K, 0), None);
    }
}
