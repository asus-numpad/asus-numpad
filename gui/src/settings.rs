//! 界面设置的持久化。
//!
//! 只存「用户的选择」，不存硬件状态：档位、联动开关、上次选中的接口。
//! Windows 下放 `%APPDATA%\numdeck-gui\settings.json`，其他平台放
//! `~/.config/numdeck-gui/settings.json`。
//!
//! 界面设置与驱动的配置文件（`numdeck.toml`）是两份东西：驱动那份写机型与
//! 布局，这里只写「这个界面自己记的事」，改这里不会影响驱动行为。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use numdeck::config::VAL_BRIGHTNESS_MAX;

const APP_DIR: &str = "numdeck-gui";
const FILE: &str = "settings.json";

/// 界面设置。字段全部带默认值，旧版本配置缺项也能读。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 当前亮度档（1..=`steps`）
    pub step: u8,
    /// 键盘侧档数：与键盘背光对齐时通常是 4 档，单独控制可用 8 档
    pub steps: u8,
    /// 键盘 4 档各自对应的触摸板亮度 level（`0x00` 或 `0x41..=0x48`）。
    ///
    /// 只在 `steps == 4`（对齐键盘四档）时生效；8 档组仍按线性等分。表结构与驱动
    /// 配置里的 `manual_brightness_levels` 相同，可以整表写过去。
    pub kbd_map: Vec<u8>,
    /// 当前是否点亮
    pub on: bool,
    /// NumLock 关闭时自动熄灯（Windows 没有键盘背光 sysfs，用它替代跟随逻辑）
    pub follow_numlock: bool,
    /// 联动轮询间隔（毫秒）
    pub poll_ms: u64,
    /// 上次选中的设备接口路径
    pub device_path: Option<String>,
    /// 启动时把上次的档位重新下发一次（休眠/热插拔后 EC 会丢状态）
    pub apply_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            step: crate::kbdmap::KBD_STEPS as u8,
            steps: crate::kbdmap::KBD_STEPS as u8,
            kbd_map: crate::kbdmap::default_map(),
            on: true,
            follow_numlock: false,
            poll_ms: 500,
            device_path: None,
            apply_on_start: true,
        }
    }
}

impl Settings {
    /// 把外部写入的越界值夹回合法区间，避免配置被手改成 0 或 99 后直接下发。
    pub fn sanitize(&mut self) {
        if !matches!(self.steps, 4 | 8) {
            self.steps = VAL_BRIGHTNESS_MAX;
        }
        if self.step == 0 || self.step > self.steps {
            self.step = self.steps.clamp(1, self.steps);
        }
        crate::kbdmap::sanitize(&mut self.kbd_map);
        // 档表必须「从暗到亮」。单项合法还不够：手改成 `[0x48, 0x41, 0x43, 0x48]` 时每项
        // 都在硬件档位内，但第 1 档比第 2 档亮，键盘背光一档档往上按反而变暗。界面直接
        // 拿这张表下发，驱动那份配置又不会被回头校验，所以在这里排一次序收干净。
        if !crate::kbdmap::monotonic(&self.kbd_map) {
            self.kbd_map.sort_unstable();
        }
        self.poll_ms = self.poll_ms.clamp(100, 5000);
    }
}

/// 配置文件路径。
pub fn config_path() -> PathBuf {
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join(APP_DIR).join(FILE);
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".config").join(APP_DIR).join(FILE);
        }
    }
    PathBuf::from(APP_DIR).join(FILE)
}

/// 读配置；文件不存在或损坏时返回默认值（不阻断界面启动）。
pub fn load() -> Settings {
    load_with_notes().0
}

/// 解析一份设置文本。坏文本不 panic、不半途而废，退回默认值并给出原因。
///
/// 抽成纯函数是为了能直接测「文件坏了会怎样」——这条路径过去没有任何测试，
/// 而它恰恰是最容易静默吃掉用户设置的地方。
pub(crate) fn parse_settings(text: &str) -> (Settings, Option<String>) {
    match serde_json::from_str::<Settings>(text) {
        Ok(mut settings) => {
            settings.sanitize();
            (settings, None)
        }
        Err(err) => (Settings::default(), Some(err.to_string())),
    }
}

/// 读设置，并把「读不回来」的原因一并交出来。
///
/// 过去这里读失败或解析失败一律安静地退回默认值：用户的档位、联动开关、上次选的
/// 接口全部消失，界面看起来一切正常，下一次保存还会把那份坏文件直接覆盖掉，连
/// 事后排查的线索都不剩。现在解析失败时先把原文件留一份副本，再把原因交给界面显示。
pub fn load_with_notes() -> (Settings, Vec<String>) {
    let path = config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), Vec::new())
        }
        Err(err) => {
            return (
                Settings::default(),
                vec![format!(
                    "读界面设置失败：{err}（本次按默认值显示，保存会覆盖 {}）",
                    path.display()
                )],
            )
        }
    };
    let (settings, err) = parse_settings(&text);
    let Some(err) = err else {
        return (settings, Vec::new());
    };
    let mut note = format!("界面设置解析失败：{err}（本次按默认值显示）");
    match keep_broken_copy(&path) {
        Some(copy) => note.push_str(&format!("，原文件已另存为 {}", copy.display())),
        None => note.push_str("，但没能另存原文件"),
    }
    (settings, vec![note])
}

/// 把坏掉的原文件另存一份，返回副本路径。
///
/// 已有同名副本时不覆盖——用户可能连着几次都起不来，第一份才是原始现场。
fn keep_broken_copy(path: &std::path::Path) -> Option<PathBuf> {
    let base = path.to_string_lossy().to_string();
    let mut candidate = PathBuf::from(format!("{base}.broken"));
    let mut n = 2;
    while candidate.exists() {
        candidate = PathBuf::from(format!("{base}.broken-{n}"));
        n += 1;
        if n > 32 {
            return None;
        }
    }
    std::fs::copy(path, &candidate).ok()?;
    Some(candidate)
}

/// 写配置。
/// 原子写：先写同目录临时文件再 rename 覆盖。
///
/// 直写目标文件一旦在写入中途失败（断电、磁盘满、进程被杀），配置就剩半截，
/// 下次启动解析失败、用户调好的设置全丢。临时文件 + rename 保证目标要么是旧版
/// 要么是完整新版，不会出现中间态。rename 之后权限随临时文件走，因此 unix 上
/// 在这里顺手把临时文件设成 0o644。
pub(crate) fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.json".to_string());
    let tmp = dir.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let write_result = (|| -> std::io::Result<()> {
        // 先写临时文件并同步到盘，再改名。只做 rename 而不 fsync，掉电后目录项可能
        // 指向一个内容还没落盘的空文件——配置直接变成 0 字节，比没保存还糟。
        {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o644))?;
        }
        std::fs::rename(&tmp, path)?;
        // 改名本身也要落盘，否则掉电后可能还是旧文件名。目录不是所有文件系统都支持
        // fsync（部分网络盘会返回 EINVAL），失败就算了，不能因此把保存判成失败。
        #[cfg(unix)]
        if let Ok(handle) = std::fs::File::open(dir) {
            let _ = handle.sync_all();
        }
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    write_result
}

pub fn save(settings: &Settings) -> Result<()> {
    let path = config_path();
    let text = serde_json::to_string_pretty(settings).context("序列化设置失败")?;
    write_atomic(&path, &text).with_context(|| format!("写入设置失败：{}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 原子写：目标已存在时被完整替换，且不留临时文件。
    #[test]
    fn write_atomic_replaces_and_cleans_up() {
        let dir = std::env::temp_dir().join(format!("numdeck-test-{}", std::process::id()));
        let path = dir.join("cfg.json");
        let _ = std::fs::remove_dir_all(&dir);
        write_atomic(&path, "first").expect("首次写入");
        write_atomic(&path, "second-and-longer").expect("覆盖写入");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second-and-longer");
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|name| name != "cfg.json")
            .collect();
        assert!(leftovers.is_empty(), "残留临时文件：{leftovers:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 原子写：文件夹本身不存在时能自己建出来（首次运行 / 用户删了配置目录）。
    #[test]
    fn write_atomic_creates_missing_dir() {
        let dir = std::env::temp_dir().join(format!("numdeck-mkdir-{}", std::process::id()));
        let path = dir.join("nested").join("cfg.json");
        let _ = std::fs::remove_dir_all(&dir);
        write_atomic(&path, "hello").expect("应能自建目录");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "hello");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 原子写：写的是空串也要真的把文件清空，不能 rename 成 0 长度的临时文件后失败。
    #[test]
    fn write_atomic_accepts_empty_text() {
        let dir = std::env::temp_dir().join(format!("numdeck-empty-{}", std::process::id()));
        let path = dir.join("cfg.json");
        let _ = std::fs::remove_dir_all(&dir);
        write_atomic(&path, "先写点东西").expect("首次写入");
        write_atomic(&path, "").expect("写空");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn defaults_are_consistent() {
        let s = Settings::default();
        assert!(s.step >= 1 && s.step <= s.steps);
        assert!((4..=8).contains(&s.steps));
    }

    #[test]
    fn sanitize_clamps_bad_values() {
        let mut s = Settings {
            step: 99,
            steps: 5,
            poll_ms: 5,
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.steps, 8);
        assert_eq!(s.step, 8);
        assert_eq!(s.poll_ms, 100);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"step": 2}"#).unwrap();
        assert_eq!(s.step, 2);
        assert_eq!(s.steps, crate::kbdmap::KBD_STEPS as u8);
        assert_eq!(s.kbd_map, crate::kbdmap::default_map());
        assert!(s.apply_on_start);
    }

    #[test]
    fn old_settings_without_kbd_map_get_defaults() {
        // 旧版设置文件没有 kbd_map 字段，读出来必须补齐成默认四档，而不是空表
        let s: Settings = serde_json::from_str(r#"{"step": 3, "steps": 4}"#).unwrap();
        assert_eq!(s.kbd_map.len(), crate::kbdmap::KBD_STEPS);
        assert_eq!(s.kbd_map, crate::kbdmap::default_map());
    }

    #[test]
    fn sanitize_fixes_kbd_map() {
        let mut s = Settings {
            kbd_map: vec![0x48, 0x10],
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.kbd_map, crate::kbdmap::default_map());

        let mut s = Settings {
            kbd_map: vec![0x10, 0x41, 0x45, 0x99],
            ..Settings::default()
        };
        s.sanitize();
        assert_eq!(s.kbd_map, vec![0x41, 0x41, 0x45, 0x48]);
    }
}

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use numdeck::config::{VAL_BRIGHTNESS_BASE, VAL_OFF};

    #[test]
    fn broken_text_falls_back_with_reason() {
        let (settings, err) = parse_settings("{ 这不是 json");
        assert!(err.is_some(), "坏文件必须说出原因，不能当作正常");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn partial_text_keeps_known_fields() {
        // 旧版本留下的文件只有一部分字段，其余要落回默认值而不是整份丢掉
        let (settings, err) = parse_settings(r#"{"step": 2, "steps": 4}"#);
        assert!(err.is_none());
        assert_eq!(settings.step, 2);
        assert_eq!(settings.steps, 4);
        assert_eq!(settings.poll_ms, Settings::default().poll_ms);
    }

    #[test]
    fn out_of_range_values_get_clamped() {
        let (settings, err) =
            parse_settings(r#"{"step": 99, "steps": 4, "poll_ms": 1, "kbd_map": [9, 9, 9, 9]}"#);
        assert!(err.is_none());
        assert_eq!(settings.step, 4);
        assert_eq!(settings.poll_ms, 100);
        // 合法值只有两种：熄灭 0x00，或硬件亮度档 0x41..=0x48
        assert!(settings.kbd_map.iter().all(|v| *v == VAL_OFF
            || (VAL_BRIGHTNESS_BASE + 1..=VAL_BRIGHTNESS_BASE + VAL_BRIGHTNESS_MAX).contains(v)));
    }

    #[test]
    fn unordered_table_gets_sorted() {
        // 每项都在硬件档位内，但顺序是乱的：排完必须是单调不递减
        let (settings, err) =
            parse_settings(r#"{"kbd_map": [72, 65, 67, 72], "steps": 4, "step": 4}"#);
        assert!(err.is_none());
        assert_eq!(settings.kbd_map, vec![65, 67, 72, 72]);
        assert!(crate::kbdmap::monotonic(&settings.kbd_map));
    }

    #[test]
    fn broken_copy_never_overwrites_first_scene() {
        let dir =
            std::env::temp_dir().join(format!("numdeck-gui-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "first").unwrap();
        let one = keep_broken_copy(&path).unwrap();
        std::fs::write(&path, "second").unwrap();
        let two = keep_broken_copy(&path).unwrap();
        let first = std::fs::read_to_string(&one).unwrap();
        let second = std::fs::read_to_string(&two).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(first, "first");
        assert_eq!(second, "second");
        assert_ne!(one, two, "两份副本不能互相覆盖");
    }
}
