//! 子命令枚举：哪些会写配置、显示名与说明。
use std::fmt;

/// 可选的子命令。`run` 不在列表里：它是前台常驻引擎，界面上等价的入口是
/// 「系统状态」区的服务启停。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubCmd {
    Check,
    Dump,
    Bright,
    Levels,
    Led,
    Layout,
    Sku,
    Lang,
}

impl SubCmd {
    pub const ALL: [SubCmd; 8] = [
        SubCmd::Check,
        SubCmd::Dump,
        SubCmd::Bright,
        SubCmd::Levels,
        SubCmd::Led,
        SubCmd::Layout,
        SubCmd::Sku,
        SubCmd::Lang,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SubCmd::Check => "check",
            SubCmd::Dump => "dump",
            SubCmd::Bright => "bright",
            SubCmd::Levels => "levels",
            SubCmd::Led => "led",
            SubCmd::Layout => "layout",
            SubCmd::Sku => "sku",
            SubCmd::Lang => "lang",
        }
    }

    /// 一句说明，显示在参数区上方，省得用户去翻 README。
    pub fn about(self) -> &'static str {
        match self {
            SubCmd::Check => "校验配置与识别结果，只读",
            SubCmd::Dump => "导出当前生效的全部配置，只读",
            SubCmd::Bright => "键盘背光联动：查看状态、开关跟随、按档位增减",
            SubCmd::Levels => "档位表：查看每档命令字，或改档位数、重排映射（写配置）",
            SubCmd::Led => "小键盘背光：点亮、熄灭或直接下发十六进制命令字",
            SubCmd::Layout => "触摸板布局：查看已识别的布局，或指定布局（写配置）",
            SubCmd::Sku => "机型：查看识别结果、列出已知机型，或手动指定 / 取消（写配置）",
            SubCmd::Lang => "界面与日志语言（写配置）",
        }
    }

    pub fn writes_config(self) -> bool {
        matches!(
            self,
            SubCmd::Levels | SubCmd::Layout | SubCmd::Sku | SubCmd::Lang
        )
    }
}

impl fmt::Display for SubCmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
