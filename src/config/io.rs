//! 落盘与路径：读写 JSON 配置、XDG 路径选择、旧路径迁移、示例配置生成。

use std::path::{Path, PathBuf};

use super::*;

impl Config {
    pub fn from_json(text: &str) -> Result<Self, String> {
        let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if let Some(map) = value.as_object_mut() {
            if let Some(legacy) = map.remove("poll_interval_ms") {
                map.entry("frame_ms").or_insert(legacy);
            }
            map.remove("numlock_zone");
            map.remove("hidraw_prefix");
            // 已废弃：这代 NumberPad 触摸板没有压感，压力过滤整条链路已删掉，
            // 老配置里残留的键直接丢掉，免得下次保存又被写回去
            map.remove("pressure_numpad_min");
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    pub fn load(path: &Path) -> (Self, Vec<String>) {
        let mut notes = Vec::new();
        let mut cfg = match std::fs::read_to_string(path) {
            Ok(text) => match Config::from_json(&text) {
                Ok(c) => c,
                Err(err) => {
                    notes.push(crate::i18n::tf(
                        "note.read_failed",
                        &[
                            ("path", &path.display().to_string()),
                            ("err", &err.to_string()),
                        ],
                    ));
                    Config::default()
                }
            },
            Err(err) => {
                if err.kind() != std::io::ErrorKind::NotFound {
                    notes.push(crate::i18n::tf(
                        "note.read_failed",
                        &[
                            ("path", &path.display().to_string()),
                            ("err", &err.to_string()),
                        ],
                    ));
                }
                Config::default()
            }
        };
        notes.extend(cfg.sanitize());
        (cfg, notes)
    }

    /// 用户配置根目录（`$XDG_CONFIG_HOME`，缺省 `~/.config`）。
    fn user_config_root() -> Option<PathBuf> {
        match std::env::var_os("XDG_CONFIG_HOME") {
            Some(xdg) if !xdg.is_empty() => Some(PathBuf::from(xdg)),
            _ => std::env::var_os("HOME")
                .filter(|h| !h.is_empty())
                .map(|home| PathBuf::from(home).join(".config")),
        }
    }

    /// 新配置路径，写入与迁移都以它为准。
    pub fn user_config_path() -> PathBuf {
        match Self::user_config_root() {
            Some(root) => root.join("numdeck").join("numdeck.json"),
            None => PathBuf::from(CONFIG_PATH),
        }
    }

    /// 更名前（1.0.x）用户级配置的位置，只读兼容用。
    pub fn legacy_user_config_paths() -> Vec<PathBuf> {
        let Some(root) = Self::user_config_root() else {
            return Vec::new();
        };
        let dir = root.join(crate::compat::LEGACY_APP_DIR);
        vec![dir.join("asus-numpad.json"), dir.join("numdeck.json")]
    }

    pub fn is_privileged() -> bool {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: geteuid 是纯查询，无副作用
            unsafe { libc::geteuid() == 0 }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // 非 Linux 只做交叉编译与单测，保持历史行为（写 /etc）
            true
        }
    }

    pub fn writable_path(read_path: &Path) -> PathBuf {
        if Self::is_privileged() {
            read_path.to_path_buf()
        } else {
            Self::user_config_path()
        }
    }

    pub fn default_path() -> PathBuf {
        let user = Self::user_config_path();
        if user.exists() {
            return user;
        }
        Self::legacy_config_path().unwrap_or(user)
    }

    /// 现存配置的探测顺序：正式路径 → 历史 `/etc` 路径 → 更名前的用户目录。
    ///
    /// 用户目录排在最后，是因为它的内容属于「某个用户」而不是「这台机器」，
    /// 只有在系统级配置全都不存在时才轮到它。
    fn legacy_config_path() -> Option<PathBuf> {
        std::iter::once(PathBuf::from(CONFIG_PATH))
            .chain(LEGACY_CONFIG_PATHS.iter().map(PathBuf::from))
            .chain(Self::legacy_user_config_paths())
            .find(|p| p.exists())
    }

    pub fn migrate_legacy_config() -> Option<String> {
        let target = Self::user_config_path();
        if target.exists() {
            return None;
        }
        let source = Self::legacy_config_path()?;
        if source == target {
            return None;
        }
        let text = std::fs::read_to_string(&source).ok()?;
        // 只搬合法配置，避免把半截文件带进用户目录
        Config::from_json(&text).ok()?;
        std::fs::create_dir_all(target.parent()?).ok()?;
        std::fs::write(&target, text.as_bytes()).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644));
        }
        let to = target.display().to_string();
        Some(crate::i18n::tf(
            "note.config_migrated",
            &[("from", &source.display().to_string()), ("to", &to)],
        ))
    }

    pub fn to_json(&self) -> String {
        let value = serde_json::json!({
            "frame_ms": self.frame_ms,
            "numlock_trigger_frames": self.numlock_trigger_frames,
            "cont_sendkey_trigger_frames": self.cont_sendkey_trigger_frames,
            "numadv_trigger_frames": self.numadv_trigger_frames,
            "max_work_time_ms": self.max_work_time_ms,
            "app_gesture_trig_um": self.app_gesture_trig_um,
            "numlock_max_valid_dist_um": self.numlock_max_valid_dist_um,
            "numlock_max_fail_num": self.numlock_max_fail_num,
            "gesture_trig_um": self.gesture_trig_um,
            "tap_threshold_mm": self.tap_threshold_mm,
            "scroll_finger_dist_mm": self.scroll_finger_dist_mm,
            "touch_sendkey_frames": self.touch_sendkey_frames,
            "touch_sendkey_ratio": self.touch_sendkey_ratio,
            "tap_interval_min_ms": self.tap_interval_min_ms,
            "gesture_edge_frames": self.gesture_edge_frames,
            "repeat_interval_ms": self.repeat_interval_ms,
            "layout_name": self.layout_name,
            "allow_unverified_layout": self.allow_unverified_layout,
            "key_mode": self.key_mode.as_str(),
            "numlock": self.numlock.as_str(),
            "calc_gesture": self.calc_gesture,
            "calc_gesture_mode": match self.calc_gesture_mode {
                CalcMode::Long => "long",
                CalcMode::Off => "off",
            },
            "mixed_mode": self.mixed_mode.as_str(),
            "grab": self.grab,
            "touchpad_width_mm": self.touchpad_width_mm,
            "touchpad_height_mm": self.touchpad_height_mm,
            "i2c_transfer": self.i2c_transfer,
            "i2c_bus": self.i2c_bus,
            "i2c_addr": self.i2c_addr,
            "hidraw": self.hidraw,
            "language": self.language,
            "follow_keyboard_brightness": self.follow_keyboard_brightness,
            "keyboard_brightness_dir": self.keyboard_brightness_dir,
            // 整数数组：既保证自己读得回来，也不给手改配置添麻烦
            "brightness_levels": self.brightness_levels,
            "brightness_poll_ms": self.brightness_poll_ms,
            "manual_brightness_step": self.manual_brightness_step,
            "manual_brightness_levels": self.manual_brightness_levels,
            "led_kind": self.led_kind,
        });

        // 自定义扩展单独挂上去：塞进 `json!` 里会把宏递归撑爆（serde_json 的
        // `json_internal!` 有递归深度限制），也便于空列表时不落盘。
        let mut value = value;
        if let Ok(v) = serde_json::to_value(&self.custom_layouts) {
            value["custom_layouts"] = v;
        }
        if let Ok(v) = serde_json::to_value(&self.led_proto) {
            value["led_proto"] = v;
        }
        if let Ok(v) = serde_json::to_value(self.pressure_switch) {
            value["pressure_switch"] = v;
        }
        if let Ok(v) = serde_json::to_value(&self.hotkeys) {
            value["hotkeys"] = v;
        }
        if !self.partition.is_unset() {
            if let Ok(v) = serde_json::to_value(&self.partition) {
                value["partition"] = v;
            }
        }
        if !self.calibration.is_unset() {
            if let Ok(v) = serde_json::to_value(self.calibration) {
                value["calibration"] = v;
            }
        }

        // 只在手动指定机型时写出：默认配置里没有这一项，读一眼就知道是自动识别
        if let Some(id) = &self.sku {
            value["sku"] = serde_json::json!(id);
        }

        let mut text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
        text.push('\n');
        text
    }

    pub fn summary(&self) -> String {
        crate::i18n::tf(
            "cfg.summary",
            &[
                ("layout", &self.layout_name),
                ("mode", self.key_mode.as_str()),
                ("frames", &self.numlock_frames().to_string()),
                ("ms", &(self.numlock_frames() * self.frame_ms).to_string()),
                ("idle", &(self.max_work_time_ms / 1000).to_string()),
                ("mixed", self.mixed_mode.as_str()),
                ("numlock", self.numlock.as_str()),
            ],
        )
    }
}
