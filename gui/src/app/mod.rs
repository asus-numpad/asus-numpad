//! 主界面。
//!
//! 界面按平台分两套，共用一个窗口骨架：
//!
//! * **Windows**：只做灯光（选接口、调档位、NumLock 联动、看报文）。键位与手势交给
//!   ASUS 官方驱动，界面一个字节都不碰 FEATURE 0x0d 之外的东西。
//! * **Linux**：全套。除灯光之外，驱动配置的每一项（机型、布局、键码风格、混合模式、
//!   手势阈值、背光通道、跟灯参数、时序、语言）都在 [`full`] 里有对应控件，另加系统
//!   状态、服务启停与命令行兜底。
//!
//! 所有档位映射都走 [`numdeck::brightness`] 与 [`numdeck::led_proto`]——和
//! Linux 侧同一份函数，避免两边灯效暗中不一致。

mod action;
mod calib;
mod calib_view;
mod command;
mod driver;
mod extend;
mod full;
mod light;
mod pages;
mod tasks;
mod widgets;
mod windows;

pub(in crate::app) use action::Action;

use tasks::STATE_REFRESH;
pub use widgets::{ui_full, window_title};

use widgets::{field, hex_table, now_hms, small};

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::backend::Backend;
use crate::cli_bridge::{self, Completion};
use crate::cmdform::CommandForm;
use crate::settings::{self, Settings};
use crate::sysinfo::{self, Snapshot};
use crate::theme::{self, Palette};
use crate::{autostart, backend, ui_font};

const MAX_LOG: usize = 200;

struct LogEntry {
    at: String,
    frame: String,
    note: String,
    ok: bool,
}

/// 右侧页签。Windows 侧只有灯光，Linux 侧五页齐全。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Page {
    Light,
    Driver,
    /// 扩展：自定义布局 / 报文 / 压感 / 热键
    Extend,
    /// 校准：自定义分区与手动校准（非原生机型）
    Calib,
    System,
    Command,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Light => "灯光",
            Page::Driver => "驱动配置",
            Page::Extend => "扩展",
            Page::Calib => "校准",
            Page::System => "系统状态",
            Page::Command => "命令行",
            Page::Settings => "设置",
        }
    }

    /// 启动时落在哪一页。默认灯光；`NUMDECK_GUI_PAGE=driver|system|command|settings`
    /// 可以直接进对应页（文档截图与排障都省一次点击）。
    fn from_env() -> Self {
        let Ok(value) = std::env::var("NUMDECK_GUI_PAGE") else {
            return Page::Light;
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "driver" | "cfg" | "config" => Page::Driver,
            "extend" | "ext" | "extension" => Page::Extend,
            "calib" | "cal" | "calibration" => Page::Calib,
            "system" | "sys" => Page::System,
            "command" | "cmd" | "cli" => Page::Command,
            "settings" | "prefs" => Page::Settings,
            _ => Page::Light,
        }
    }
}

pub struct App {
    /// 当前生效的调色板（深浅色各一套，见 [`crate::theme`]）
    pub(in crate::app) palette: Palette,
    pub(in crate::app) backend: Box<dyn Backend>,
    pub(in crate::app) devices: Vec<backend::DeviceInfo>,
    pub(in crate::app) selected: Option<usize>,
    pub(in crate::app) settings: Settings,
    /// 设置改过但还没落盘
    pub(in crate::app) settings_dirty: bool,
    /// 最后一次改动的时间，用来判断「停手了没有」
    pub(in crate::app) settings_dirty_at: Option<Instant>,
    pub(in crate::app) log: VecDeque<LogEntry>,
    pub(in crate::app) status: String,
    pub(in crate::app) status_bad: bool,
    pub(in crate::app) last_poll: Instant,
    pub(in crate::app) last_numlock: Option<bool>,
    pub(in crate::app) sent_numlock: Option<bool>,
    pub(in crate::app) autostart_on: bool,
    pub(in crate::app) font_note: Option<String>,
    /// 平台状态快照（机型、布局、背光通道、后台服务）
    pub(in crate::app) state: Snapshot,
    pub(in crate::app) state_at: Instant,
    /// 后台重读平台状态的收件口。`snapshot()` 在 Linux 上要起几个 `systemctl`
    /// 子进程，同步跑会每隔 10 秒把界面卡住几十毫秒（systemd 忙的时候更久），
    /// 所以改成「发给线程、下一帧取回」，与命令面板同一套路。
    pub(in crate::app) state_rx: Option<std::sync::mpsc::Receiver<Snapshot>>,
    /// 有人要求「刷新一下状态」。放在这里而不是直接调用，是因为发起刷新的地方
    /// （命令执行完、点了刷新按钮）未必手上有 `egui::Context`。
    pub(in crate::app) state_want: bool,
    /// 服务动作（启停 / 重启 / 开机自启）的收件口。`systemctl restart` 要等服务真
    /// 起来才返回，慢盘或者 systemd 忙的时候一两秒，同步跑就是明显的卡顿；同一时刻
    /// 只放行一件，免得连点几下「重启」派出几条 systemctl 互相打架。
    pub(in crate::app) op_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>,
    /// 上一次设置写盘失败的原因：同一条错误只往状态栏报一次，避免每 400 ms 刷一遍。
    pub(in crate::app) settings_save_err: Option<String>,
    /// 是否画 Linux 全套面板；Windows 只画灯光
    pub(in crate::app) ui_full: bool,
    /// 右侧当前页（只有全套模式才显示页签）
    pub(in crate::app) tab: Page,
    /// Linux 侧驱动配置（`numdeck.json`）的编辑草稿
    pub(in crate::app) driver: driver::Draft,
    /// 扩展页「校验扩展项」的结果（布局装没装上、HID 打没打架、报文合不合法）
    pub(in crate::app) ext_notes: Vec<String>,
    /// 扩展页里自定义报文模板的十六进制草稿
    pub(in crate::app) ext_proto: String,
    /// 草稿是否正被编辑：编辑期间不用配置里的值覆盖，免得打到一半跳一下
    pub(in crate::app) ext_proto_dirty: bool,
    pub(in crate::app) ext_proto_err: Option<String>,
    /// 扩展页「配置片段」文本框：四项打包成一段 JSON，方便同机型之间互传
    pub(in crate::app) ext_snippet: String,
    /// 片段导入结果：`(文案, 是否出错)`
    pub(in crate::app) ext_snippet_msg: Option<(String, bool)>,
    /// 校准页：样本文件路径（真机采到的落点）
    pub(in crate::app) calib_samples_path: String,
    /// 校准页：解算时按几列几行（与键表一致）
    pub(in crate::app) calib_cols: usize,
    pub(in crate::app) calib_rows: usize,
    /// 校准页：回放结果 `(文案, 是否出错)`
    pub(in crate::app) calib_msg: Option<(String, bool)>,
    /// 校准页：上次解析过的样本路径，与 `calib_samples_path` 不同就重解析，
    /// 样本点因此能跟着输入框变，不用再点一次「读取并解算」
    pub(in crate::app) calib_probe_path: String,
    /// 校准页：样本点（原始坐标 + 应落格），画在预览图上
    pub(in crate::app) calib_probes: Vec<calib_view::Probe>,
    /// 校准页：样本文件解析失败的原因
    pub(in crate::app) calib_probe_msg: Option<String>,
    /// 校准页「配置片段」：partition + calibration 打包
    pub(in crate::app) calib_snippet: String,
    pub(in crate::app) calib_snippet_msg: Option<(String, bool)>,
    /// 后台服务是否开机自启；Windows 用注册表那份，Linux 用这一份
    pub(in crate::app) service_enabled: bool,
    /// 命令面板：与 `numdeck` CLI 同一套实现，界面只负责拼参数
    pub(in crate::app) cmd: CommandForm,
    pub(in crate::app) cmd_bin: Option<std::path::PathBuf>,
    pub(in crate::app) cmd_rx: Option<std::sync::mpsc::Receiver<Completion>>,
    pub(in crate::app) cmd_running: bool,
    pub(in crate::app) cmd_out: Option<String>,
    pub(in crate::app) cmd_ok: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let font_note = ui_font::install(&cc.egui_ctx);
        // 主题要在建界面之前装好，首帧就是定稿的样子（截图也才稳定）
        let palette = theme::install(&cc.egui_ctx);
        let settings = settings::load();
        let state = sysinfo::snapshot();
        let service_enabled = state
            .service
            .as_ref()
            .map(|s| s.enabled.trim() == "enabled")
            .unwrap_or(false);
        let driver = driver::Draft::load();
        // 报文草稿从配置里的模板起步；之后的编辑由 draw_ext_proto 自己同步
        let ext_proto = extend::hex_of(&driver.cfg.led_proto.template);
        let mut app = Self {
            palette,
            backend: backend::create(),
            devices: Vec::new(),
            selected: None,
            settings,
            settings_dirty: false,
            settings_dirty_at: None,
            log: VecDeque::new(),
            status: String::new(),
            status_bad: false,
            last_poll: Instant::now(),
            last_numlock: None,
            sent_numlock: None,
            autostart_on: autostart::enabled(),
            font_note,
            state,
            state_at: Instant::now(),
            state_rx: None,
            state_want: false,
            op_rx: None,
            settings_save_err: None,
            ui_full: ui_full(),
            tab: Page::from_env(),
            driver,
            ext_notes: Vec::new(),
            ext_proto,
            ext_proto_dirty: false,
            ext_proto_err: None,
            ext_snippet: String::new(),
            ext_snippet_msg: None,
            calib_samples_path: String::new(),
            calib_cols: 5,
            calib_rows: 4,
            calib_msg: None,
            calib_probe_path: String::new(),
            calib_probes: Vec::new(),
            calib_probe_msg: None,
            calib_snippet: String::new(),
            calib_snippet_msg: None,
            service_enabled,
            cmd: CommandForm::default(),
            cmd_bin: cli_bridge::locate(),
            cmd_rx: None,
            cmd_running: false,
            cmd_out: None,
            cmd_ok: true,
        };
        app.rescan();
        if app.settings.apply_on_start && app.settings.on {
            app.apply_current();
        }
        app
    }

    fn set_status(&mut self, text: String, bad: bool) {
        self.status = text;
        self.status_bad = bad;
    }

    fn window_title(&self) -> &'static str {
        if self.ui_full {
            "ASUS NumberPad 控制中心"
        } else {
            "ASUS NumberPad 灯光控制"
        }
    }
}

impl eframe::App for App {
    /// 截图模式用：每帧先把指针放到「要拍气泡的那个控件」上（正常启动是空操作）
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        crate::shot::raw_input_hook(raw_input);
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 文档截图模式（设置了 NUMDECK_GUI_SHOT 才生效）
        crate::shot::tick(root.ctx());

        let before = self.settings.clone();
        let mut action: Option<Action> = None;

        self.poll_numlock();
        self.poll_command();
        self.poll_op(root.ctx());
        self.poll_state(root.ctx());

        // 机型 / 服务状态在会话里会变（插拔、systemctl 被别处调用），定期重读一次
        if self.state_want || self.state_at.elapsed() > STATE_REFRESH {
            self.refresh_state(root.ctx());
        }

        self.draw_header(root, &mut action);
        self.draw_status(root);
        self.draw_devices(root);

        let p = self.palette;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.bg)
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(root, |ui| {
                if self.ui_full {
                    let pages = [
                        (
                            Page::Light,
                            Page::Light.title(),
                            "灯光：档位条、四档映射、NumLock 联动、报文日志",
                        ),
                        (
                            Page::Driver,
                            Page::Driver.title(),
                            "驱动配置：机型、布局、键码风格、手势阈值、背光通道、时序、语言",
                        ),
                        (
                            Page::Extend,
                            Page::Extend.title(),
                            "扩展：给表外机型补自定义布局、换点亮报文、压感切开关、绑全局热键",
                        ),
                        (
                            Page::Calib,
                            Page::Calib.title(),
                            "校准：给表外机型自定义网格分区，并按实测落点校准面板范围与分辨率",
                        ),
                        (
                            Page::System,
                            Page::System.title(),
                            "系统状态：认出哪台机、走哪条背光通道、后台服务状态、诊断导出",
                        ),
                        (
                            Page::Command,
                            Page::Command.title(),
                            "命令行：把 numdeck 全部子命令拼好再执行，输出与终端一致",
                        ),
                        (
                            Page::Settings,
                            Page::Settings.title(),
                            "设置：启动行为、开机自启、配置文件路径与版本",
                        ),
                    ];
                    if let Some(page) = theme::segmented(ui, &p, "tabs", self.tab, &pages) {
                        self.tab = page;
                    }
                    // 页签下面补一行当前页的说明：页签只有两个字，光靠它进来会不知道
                    // 这一页在管什么，又不该用大标题再重复一遍页签名，所以只留小字副题。
                    if let Some((_, _, help)) = pages.iter().find(|(page, _, _)| *page == self.tab)
                    {
                        if let Some((_, lead)) = help.split_once('：') {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(lead).size(12.5).color(p.text_weak));
                        }
                    }
                    ui.add_space(12.0);
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .id_salt(("page", self.tab))
                    .show(ui, |ui| {
                        // 控件列宽度整页只定一次，各卡片、各折叠区里的行都照这一个宽度排
                        theme::begin_page(ui);
                        match (self.ui_full, self.tab) {
                            (false, _) => {
                                self.draw_lighting(ui, &mut action);
                                ui.add_space(10.0);
                                self.draw_follow(ui);
                                ui.add_space(10.0);
                                self.draw_log(ui);
                                ui.add_space(10.0);
                                self.draw_lighting_note(ui, &mut action);
                                ui.add_space(10.0);
                                self.draw_settings(ui, &mut action);
                            }
                            (true, Page::Light) => {
                                self.draw_lighting(ui, &mut action);
                                ui.add_space(10.0);
                                self.draw_follow(ui);
                                ui.add_space(10.0);
                                self.draw_log(ui);
                            }
                            (true, Page::Driver) => self.draw_full_controls(ui, &mut action),
                            (true, Page::Extend) => self.draw_extension(ui, &mut action),
                            (true, Page::Calib) => self.draw_calib(ui, &mut action),
                            (true, Page::System) => self.draw_system(ui, &mut action),
                            (true, Page::Command) => self.draw_commands(ui, &mut action),
                            (true, Page::Settings) => self.draw_settings(ui, &mut action),
                        }
                    });
            });

        if let Some(action) = action {
            self.run_action(root.ctx(), action);
        }

        // 设置改动等停手再落盘：拖动中的中间值没有必要每次都写文件
        if self.settings != before {
            self.settings.sanitize();
            self.settings_dirty = true;
            self.settings_dirty_at = Some(Instant::now());
        }
        self.flush_settings(root.ctx());

        // 界面空闲时也要定期醒来：NumLock 跟随要按 poll_ms 轮询，机型/服务状态
        // 也靠 `STATE_REFRESH` 过期重读。少了这个请求，窗口静止时状态会一直停在
        // 打开那一刻，插拔设备、在别处启停服务都看不见。
        let wait = if self.settings.follow_numlock {
            Duration::from_millis(self.settings.poll_ms).min(Duration::from_secs(1))
        } else {
            Duration::from_secs(1)
        };
        root.ctx().request_repaint_after(wait);
    }
}

impl Drop for App {
    /// 关窗口时兜一次：节流窗口内（[`SETTINGS_SAVE_QUIET`]）改完就退出的设置也要落盘。
    fn drop(&mut self) {
        if self.settings_dirty {
            let _ = settings::save(&self.settings);
            self.settings_dirty = false;
        }
    }
}

/// 命令输出框最多排多少行。
const VISIBLE_OUTPUT_LINES: usize = 400;

/// 品牌名：与官网 `site/index.html` 的 `.brand-name` 一致。
const BRAND: &str = "numdeck";

/// 「会写配置」「没找到可执行文件」这类提示色。
///
/// 取深色主题的告警色：这些提示都落在深色卡片上的实心文字，浅色主题下同一枚
/// 琥珀色在白底上也有 3.9:1，够用；这样界面里只剩一套状态色，不再各处各写一个 RGB。
const WARN: egui::Color32 = crate::theme::DARK.warn;
