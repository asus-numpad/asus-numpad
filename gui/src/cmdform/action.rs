//! 子命令的动作位：bright 的 follow/cycle、levels 的 step/reset、led 的开关。
/// `bright` 的 VALUE 位。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrightAction {
    /// 不传 VALUE，CLI 打印当前状态
    Status,
    /// 直接跳到某一档
    Step,
    /// 跟随键盘背光，开或关由 `bright_follow_on` 决定
    Follow,
    Cycle,
    Up,
    Down,
}

impl BrightAction {
    pub const ALL: [BrightAction; 6] = [
        BrightAction::Status,
        BrightAction::Step,
        BrightAction::Follow,
        BrightAction::Cycle,
        BrightAction::Up,
        BrightAction::Down,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BrightAction::Status => "查看状态",
            BrightAction::Step => "跳到指定档位",
            BrightAction::Follow => "跟随键盘背光（开 / 关）",
            BrightAction::Cycle => "走到下一档（cycle）",
            BrightAction::Up => "升一档（up）",
            BrightAction::Down => "降一档（down）",
        }
    }

    pub(super) fn word(self) -> Option<&'static str> {
        match self {
            BrightAction::Status => None,
            BrightAction::Step => None,
            BrightAction::Follow => Some("follow"),
            BrightAction::Cycle => Some("cycle"),
            BrightAction::Up => Some("up"),
            BrightAction::Down => Some("down"),
        }
    }
}

/// `levels` 的形态。只写 STEP 数字是非法组合，CLI 会打用法并返回 2，界面因此拆成三态。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LevelsAction {
    /// 不传参数，打印档位表
    Table,
    /// STEP 位写 reset，恢复默认档表
    Reset,
    /// STEP 与 VALUE 同时给，改写某一档
    Set,
}

impl LevelsAction {
    pub const ALL: [LevelsAction; 3] =
        [LevelsAction::Table, LevelsAction::Reset, LevelsAction::Set];

    pub fn label(self) -> &'static str {
        match self {
            LevelsAction::Table => "查看档位表",
            LevelsAction::Reset => "恢复默认档位表（reset）",
            LevelsAction::Set => "改写某一档",
        }
    }
}

/// `led` 的 VALUE 位。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LedAction {
    /// 不传参数，打印当前值与通道
    Status,
    On,
    Off,
    /// 原样下发一个字节，如 `0x41`
    Raw,
}

impl LedAction {
    pub const ALL: [LedAction; 4] = [
        LedAction::Status,
        LedAction::On,
        LedAction::Off,
        LedAction::Raw,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LedAction::Status => "不带值（打印用法）",
            LedAction::On => "点亮（on）",
            LedAction::Off => "熄灭（off）",
            LedAction::Raw => "直接下发命令字",
        }
    }
}
