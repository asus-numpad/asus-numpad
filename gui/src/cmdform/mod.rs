//! 命令面板的表单状态与参数拼装。
//!
//! 这里只负责「界面上的控件值」到「`numdeck` 的 argv」的映射，不执行任何东西，
//! 因此可以脱离界面做单测。拼错参数名、把空串当成有效值、把互斥开关同时勾上这类
//! 问题，都由 [`CommandForm::argv`] 的测试兜住。
//!
//! 每个子命令的可选形态都照 CLI 的实际分支来：`bright` 的 VALUE 位既可以是档位数字，
//! 也可以是 `follow` / `cycle` / `up` / `down` 这类动作，`levels` 的 STEP 位在只写
//! `reset` 时是「恢复默认」，两者写法不同，所以界面用枚举区分而不是一个自由输入框。

mod action;
mod advanced;
mod form;
mod sub;

pub use action::*;
pub use advanced::*;
pub use form::*;
pub use sub::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn form(sub: SubCmd) -> CommandForm {
        CommandForm {
            sub,
            ..Default::default()
        }
    }

    #[test]
    fn check_and_dump_take_no_arguments() {
        assert_eq!(form(SubCmd::Check).argv().unwrap(), vec!["check"]);
        assert_eq!(form(SubCmd::Dump).argv().unwrap(), vec!["dump"]);
    }

    #[test]
    fn bright_status_sends_no_value() {
        assert_eq!(form(SubCmd::Bright).argv().unwrap(), vec!["bright"]);
    }

    #[test]
    fn bright_step_is_a_bare_number() {
        let mut f = form(SubCmd::Bright);
        f.bright_action = BrightAction::Step;
        f.bright_step = " 3 ".to_string();
        assert_eq!(f.argv().unwrap(), vec!["bright", "3"]);

        f.bright_step = "亮".to_string();
        assert!(f.argv().unwrap_err().contains("档位需要数字"));
    }

    #[test]
    fn bright_follow_carries_on_or_off() {
        let mut f = form(SubCmd::Bright);
        f.bright_action = BrightAction::Follow;
        assert_eq!(f.argv().unwrap(), vec!["bright", "follow", "on"]);
        f.bright_follow_on = false;
        assert_eq!(f.argv().unwrap(), vec!["bright", "follow", "off"]);
    }

    #[test]
    fn bright_cycle_and_up_down() {
        let mut f = form(SubCmd::Bright);
        f.bright_action = BrightAction::Cycle;
        assert_eq!(f.argv().unwrap(), vec!["bright", "cycle"]);
        f.bright_action = BrightAction::Up;
        assert_eq!(f.argv().unwrap(), vec!["bright", "up"]);
        f.bright_action = BrightAction::Down;
        assert_eq!(f.argv().unwrap(), vec!["bright", "down"]);
    }

    #[test]
    fn levels_table_and_reset() {
        assert_eq!(form(SubCmd::Levels).argv().unwrap(), vec!["levels"]);

        let mut f = form(SubCmd::Levels);
        f.levels_action = LevelsAction::Reset;
        assert_eq!(f.argv().unwrap(), vec!["levels", "reset"]);
    }

    #[test]
    fn levels_set_needs_both_fields() {
        let mut f = form(SubCmd::Levels);
        f.levels_action = LevelsAction::Set;
        f.levels_value.clear();
        assert!(f.argv().unwrap_err().contains("同时给出"));

        f.levels_value = "0x48".to_string();
        f.levels_step = "2".to_string();
        assert_eq!(f.argv().unwrap(), vec!["levels", "2", "0x48"]);
    }

    #[test]
    fn led_variants() {
        assert_eq!(form(SubCmd::Led).argv().unwrap(), vec!["led"]);

        let mut f = form(SubCmd::Led);
        f.led_action = LedAction::On;
        assert_eq!(f.argv().unwrap(), vec!["led", "on"]);
        f.led_action = LedAction::Off;
        assert_eq!(f.argv().unwrap(), vec!["led", "off"]);

        f.led_action = LedAction::Raw;
        assert_eq!(f.argv().unwrap(), vec!["led", "0x41"]);
        f.led_raw.clear();
        assert!(f.argv().unwrap_err().contains("0x41"));
    }

    #[test]
    fn layout_name_and_ids_flag() {
        let mut f = form(SubCmd::Layout);
        f.layout_name = "g614".to_string();
        f.layout_ids = true;
        assert_eq!(f.argv().unwrap(), vec!["layout", "g614", "--ids"]);
        assert!(f.writes_config());
    }

    #[test]
    fn layout_without_name_is_read_only() {
        let mut f = form(SubCmd::Layout);
        f.layout_ids = true;
        assert_eq!(f.argv().unwrap(), vec!["layout", "--ids"]);
        assert!(!f.writes_config());
    }

    #[test]
    fn sku_and_lang_take_optional_positional() {
        let mut f = form(SubCmd::Sku);
        f.sku_id = "g614jv".to_string();
        assert_eq!(f.argv().unwrap(), vec!["sku", "g614jv"]);

        let mut f = form(SubCmd::Lang);
        assert_eq!(f.argv().unwrap(), vec!["lang"]);
        f.lang_code = "zh_TW".to_string();
        assert_eq!(f.argv().unwrap(), vec!["lang", "zh_TW"]);
    }

    #[test]
    fn advanced_flags_go_before_subcommand() {
        let mut f = form(SubCmd::Check);
        f.advanced.verbose = true;
        f.advanced.config = "/tmp/a.json".to_string();
        assert_eq!(
            f.argv().unwrap(),
            vec!["--config", "/tmp/a.json", "--verbose", "check"]
        );
    }

    #[test]
    fn advanced_numbers_are_validated() {
        let mut f = form(SubCmd::Check);
        f.advanced.i2c_bus = "abc".to_string();
        assert!(f.argv().unwrap_err().contains("--i2c-bus"));

        f.advanced.i2c_bus = "12".to_string();
        assert_eq!(f.argv().unwrap(), vec!["--i2c-bus", "12", "check"]);
    }

    #[test]
    fn grab_and_nograb_are_exclusive() {
        let mut f = form(SubCmd::Check);
        f.advanced.grab = true;
        f.advanced.no_grab = true;
        assert!(f.argv().unwrap_err().contains("互斥"));
    }

    #[test]
    fn preview_quotes_paths_with_spaces() {
        let mut f = form(SubCmd::Dump);
        f.advanced.config = "/tmp/my dir/a.json".to_string();
        let text = f.preview("numdeck");
        assert!(text.contains("\"/tmp/my dir/a.json\""));
        assert!(text.ends_with("dump"));
    }

    #[test]
    fn preview_escapes_inner_quotes() {
        let mut f = form(SubCmd::Dump);
        f.advanced.config = "/tmp/a \"b\".json".to_string();
        let text = f.preview("numdeck");
        assert!(
            text.contains(r#""/tmp/a \"b\".json""#),
            "预览里要能直接粘回终端：{text}"
        );
    }

    #[test]
    fn write_commands_are_flagged() {
        assert!(!form(SubCmd::Check).writes_config());
        assert!(!form(SubCmd::Bright).writes_config());
        assert!(!form(SubCmd::Led).writes_config());
        assert!(form(SubCmd::Sku).with_sku("auto").writes_config());
        assert!(form(SubCmd::Lang).with_lang("en").writes_config());
    }

    impl CommandForm {
        fn with_sku(mut self, id: &str) -> Self {
            self.sku_id = id.to_string();
            self
        }

        fn with_lang(mut self, code: &str) -> Self {
            self.lang_code = code.to_string();
            self
        }
    }
}
