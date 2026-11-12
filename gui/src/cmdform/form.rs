//! 表单整体：控件值到 argv 的拼装与校验、预览、超时、是否写配置。
use super::*;

/// 命令面板的完整表单。
pub struct CommandForm {
    pub sub: SubCmd,
    pub bright_action: BrightAction,
    pub bright_step: String,
    pub bright_follow_on: bool,
    pub levels_action: LevelsAction,
    pub levels_step: String,
    pub levels_value: String,
    pub led_action: LedAction,
    pub led_raw: String,
    pub layout_name: String,
    pub layout_ids: bool,
    pub sku_id: String,
    pub lang_code: String,
    pub advanced: Advanced,
}

impl Default for CommandForm {
    fn default() -> Self {
        Self {
            sub: SubCmd::Check,
            bright_action: BrightAction::Status,
            bright_step: "2".to_string(),
            bright_follow_on: true,
            levels_action: LevelsAction::Table,
            levels_step: "1".to_string(),
            levels_value: "0x41".to_string(),
            led_action: LedAction::Status,
            led_raw: "0x41".to_string(),
            layout_name: String::new(),
            layout_ids: false,
            sku_id: String::new(),
            lang_code: String::new(),
            advanced: Advanced::default(),
        }
    }
}

impl CommandForm {
    /// 拼出参数列表（不含可执行文件路径）。
    pub fn argv(&self) -> Result<Vec<String>, String> {
        let mut argv = Vec::new();
        self.advanced.append(&mut argv)?;

        let sub = self.sub;
        argv.push(sub.name().to_string());

        match sub {
            SubCmd::Check | SubCmd::Dump => {}
            SubCmd::Bright => {
                if let Some(word) = self.bright_action.word() {
                    argv.push(word.to_string());
                    if self.bright_action == BrightAction::Follow {
                        argv.push(if self.bright_follow_on { "on" } else { "off" }.to_string());
                    }
                }
                if self.bright_action == BrightAction::Step {
                    let step = self.bright_step.trim();
                    let parsed: u8 = step.parse().map_err(|_| format!("档位需要数字：{step}"))?;
                    argv.push(parsed.to_string());
                }
            }
            SubCmd::Levels => match self.levels_action {
                LevelsAction::Table => {}
                LevelsAction::Reset => argv.push("reset".to_string()),
                LevelsAction::Set => {
                    let step = self.levels_step.trim();
                    let value = self.levels_value.trim();
                    if step.is_empty() || value.is_empty() {
                        return Err("改写某一档需要同时给出档位与命令字".to_string());
                    }
                    let parsed: usize =
                        step.parse().map_err(|_| format!("档位需要数字：{step}"))?;
                    argv.push(parsed.to_string());
                    argv.push(value.to_string());
                }
            },
            SubCmd::Led => match self.led_action {
                LedAction::Status => {}
                LedAction::On => argv.push("on".to_string()),
                LedAction::Off => argv.push("off".to_string()),
                LedAction::Raw => {
                    let raw = self.led_raw.trim();
                    if raw.is_empty() {
                        return Err("直接下发命令字时要填一个字节，如 0x41".to_string());
                    }
                    argv.push(raw.to_string());
                }
            },
            SubCmd::Layout => {
                let name = self.layout_name.trim();
                if !name.is_empty() {
                    argv.push(name.to_string());
                }
                if self.layout_ids {
                    argv.push("--ids".to_string());
                }
            }
            SubCmd::Sku => {
                let id = self.sku_id.trim();
                if !id.is_empty() {
                    argv.push(id.to_string());
                }
            }
            SubCmd::Lang => {
                let code = self.lang_code.trim();
                if !code.is_empty() {
                    argv.push(code.to_string());
                }
            }
        }

        Ok(argv)
    }

    /// 界面上要显示的一行预览，便于复制到终端复现。
    pub fn preview(&self, bin: &str) -> String {
        match self.argv() {
            Ok(argv) => {
                let mut text = bin.to_string();
                for arg in argv {
                    text.push(' ');
                    // 带空格的要加引号；本身含引号的还得转义一下，否则这一行贴回终端
                    // 会把参数切断（预览是给人复制的，得能直接用）。
                    if arg.contains(' ') || arg.contains('"') {
                        text.push('"');
                        text.push_str(&arg.replace('"', "\\\""));
                        text.push('"');
                    } else {
                        text.push_str(&arg);
                    }
                }
                text
            }
            Err(err) => format!("参数有误：{err}"),
        }
    }

    /// 这次执行要不要给个时间上限。
    ///
    /// `dump` 在 CLI 里是「一直打印事件直到 Ctrl+C」，界面上必须有个头，否则面板会
    /// 永远停在「执行中」；其余子命令都是跑完就退出，交回 `None`。
    pub fn timeout(&self) -> Option<std::time::Duration> {
        match self.sub {
            SubCmd::Dump => Some(std::time::Duration::from_secs(5)),
            _ => None,
        }
    }

    /// 这次执行会不会落盘改配置，界面据此把按钮标成需要留心的一项。
    pub fn writes_config(&self) -> bool {
        if !self.sub.writes_config() {
            return false;
        }
        match self.sub {
            SubCmd::Levels => self.levels_action != LevelsAction::Table,
            SubCmd::Layout => !self.layout_name.trim().is_empty(),
            SubCmd::Sku => !self.sku_id.trim().is_empty(),
            SubCmd::Lang => !self.lang_code.trim().is_empty(),
            _ => false,
        }
    }
}
