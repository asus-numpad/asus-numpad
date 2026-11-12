//! CLI 各子命令共用的小工具：写回配置键、问 user bus、起 systemctl。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use numdeck::config::Config;
use numdeck::i18n::{self};

pub(crate) fn parse_addr(s: &str) -> Option<u8> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u8>().ok()
    }
}

/// 只改配置文件里指定的键，其余字段原样保留；写前留 `.bak` 备份。
pub(crate) fn persist_config_keys(
    path: &Path,
    updates: &[(&str, serde_json::Value)],
) -> Result<PathBuf> {
    // 普通用户写自己的 XDG 配置（免 sudo），只有 root 才落 /etc——见 Config::writable_path
    let target = Config::writable_path(path);
    let path = target.as_path();
    let base = match std::fs::read_to_string(path) {
        Ok(text) => text,
        // 配置文件不存在：以默认配置为底再写入差异项
        Err(_) => Config::default().to_json(),
    };
    let mut value: serde_json::Value = serde_json::from_str(&base)
        .with_context(|| i18n::tf("persist.bad_json", &[("path", &path.display().to_string())]))?;
    if !value.is_object() {
        value = serde_json::json!({});
    }
    for (key, v) in updates {
        value[*key] = v.clone();
    }
    let mut text = serde_json::to_string_pretty(&value)?;
    text.push('\n');

    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).with_context(|| {
                i18n::tf(
                    "persist.mkdir_failed",
                    &[("dir", &dir.display().to_string())],
                )
            })?;
        }
    }
    let backup = PathBuf::from(format!("{}.bak", path.display()));
    if path.exists() {
        std::fs::copy(path, &backup).with_context(|| {
            i18n::tf(
                "persist.backup_failed",
                &[("path", &path.display().to_string())],
            )
        })?;
    }
    std::fs::write(path, text).with_context(|| {
        i18n::tf(
            "persist.write_failed",
            &[("path", &path.display().to_string())],
        )
    })?;
    Ok(backup)
}

/// 若 numdeck 服务正在运行，就重启它让新配置立即生效；未运行时只提示，不主动拉起。
/// 当前是否在图形/登录会话里（`systemd --user` 可用）。
pub(crate) fn has_user_bus() -> bool {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|v| !v.as_os_str().is_empty())
        .unwrap_or(false)
}

/// 调 systemctl：会话内走用户级实例（免 sudo），否则退系统级（root 场景）。
pub(crate) fn systemctl(args: &[&str]) -> bool {
    let mut cmd = std::process::Command::new("systemctl");
    if has_user_bus() {
        cmd.arg("--user");
    }
    cmd.args(args);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// 当前在跑的服务单元名。
///
/// 更名前装的服务叫 `asus-numpad`，升级后用户在换包前可能还在跑旧的，
/// 这时 `restart` 得打到旧单元上，否则「重启服务」会报没这个单元。
fn running_service_unit() -> &'static str {
    for unit in ["numdeck", numdeck::compat::LEGACY_SERVICE] {
        if systemctl(&["is-active", "--quiet", unit]) {
            return unit;
        }
    }
    "numdeck"
}

pub(crate) fn try_restart_service() {
    let unit = running_service_unit();
    if !systemctl(&["is-active", "--quiet", unit]) {
        println!("{}", i18n::t("svc.not_running"));
        return;
    }
    if systemctl(&["restart", unit]) {
        println!("{}", i18n::t("svc.restarted"));
    } else {
        println!("{}", i18n::t("svc.restart_failed"));
    }
}
