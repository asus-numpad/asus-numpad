//! 旧名兼容：`asus-numpad` 时代留下的配置目录、语言目录、环境变量与 systemd 单元。
//!
//! 项目更名不可避免，但已经装了旧版的机器不该因为一次升级就丢配置、丢自启、
//! 或者发现自己写进脚本里的 `ASUS_NUMPAD_LANG` 突然失效。这里集中做两件事：
//!
//! * **读**：新名优先，找不到时回退到旧名（只读，不修改旧文件）；
//! * **写**：一律写新名，旧名只作为一次性迁移的来源。
//!
//! 旧名统一以 [`LEGACY_APP_DIR`] 为准，不要在别处再手写这个字符串。

use std::path::{Path, PathBuf};

/// 旧项目名（目录名、systemd 单元名、环境变量前缀都从它派生）。
pub const LEGACY_APP_DIR: &str = "asus-numpad";

/// 旧环境变量 → 新环境变量。
///
/// 只做「新名没设时继承旧值」的单向映射，不反向写回，
/// 免得子进程看到两份互相矛盾的变量。
const ENV_ALIASES: &[(&str, &str)] = &[
    ("ASUS_NUMPAD_LANG", "NUMDECK_LANG"),
    ("ASUS_NUMPAD_LANG_DIR", "NUMDECK_LANG_DIR"),
    ("ASUS_NUMPAD_BASE", "NUMDECK_BASE"),
    ("ASUS_NUMPAD_BIN", "NUMDECK_BIN"),
    ("ASUS_NUMPAD_GUI_FULL", "NUMDECK_GUI_FULL"),
    ("ASUS_NUMPAD_GUI_HOVER", "NUMDECK_GUI_HOVER"),
    ("ASUS_NUMPAD_GUI_HOVER_INDEX", "NUMDECK_GUI_HOVER_INDEX"),
    ("ASUS_NUMPAD_GUI_HOVER_TEXT", "NUMDECK_GUI_HOVER_TEXT"),
    ("ASUS_NUMPAD_GUI_PAGE", "NUMDECK_GUI_PAGE"),
    ("ASUS_NUMPAD_GUI_SCROLL", "NUMDECK_GUI_SCROLL"),
    ("ASUS_NUMPAD_GUI_SHOT", "NUMDECK_GUI_SHOT"),
    ("ASUS_NUMPAD_GUI_SIZE", "NUMDECK_GUI_SIZE"),
    ("ASUS_NUMPAD_GUI_THEME", "NUMDECK_GUI_THEME"),
];

/// 把旧环境变量继承到新名下。必须在读取任何配置/语言之前调用。
///
/// 幂等：新名已有值时不动，可重复调用。
pub fn adopt_legacy_env() {
    for (old, new) in ENV_ALIASES {
        if std::env::var_os(new).is_some() {
            continue;
        }
        if let Some(value) = std::env::var_os(old) {
            std::env::set_var(new, value);
        }
    }
}

/// 在用户配置根目录（`$XDG_CONFIG_HOME` 或 `~/.config`）下挑一个真正可用的子目录：
/// 新目录存在就用新的，否则旧目录存在就用旧的，都没有则返回新目录（供新建）。
pub fn pick_user_dir(config_root: &Path, current: &str) -> PathBuf {
    let cur = config_root.join(current);
    if cur.exists() {
        return cur;
    }
    let old = config_root.join(LEGACY_APP_DIR);
    if old.exists() {
        return old;
    }
    cur
}

/// 旧 systemd 单元名（含用户级与系统级，安装脚本与 `restart` 都会用到）。
pub const LEGACY_SERVICE: &str = "asus-numpad";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_env_is_inherited_only_when_new_is_unset() {
        // 单测共享进程环境，用完立刻清理，避免污染同进程内其它用例
        std::env::remove_var("NUMDECK_LANG");
        std::env::set_var("ASUS_NUMPAD_LANG", "zh_TW");
        adopt_legacy_env();
        assert_eq!(std::env::var("NUMDECK_LANG").as_deref(), Ok("zh_TW"));

        std::env::set_var("NUMDECK_LANG", "en");
        adopt_legacy_env();
        assert_eq!(std::env::var("NUMDECK_LANG").as_deref(), Ok("en"));

        std::env::remove_var("NUMDECK_LANG");
        std::env::remove_var("ASUS_NUMPAD_LANG");
    }

    #[test]
    fn pick_user_dir_prefers_existing_current_then_legacy() {
        let base = std::env::temp_dir().join(format!("numdeck-compat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join(LEGACY_APP_DIR)).unwrap();

        // 只有旧目录 → 用旧的
        assert_eq!(pick_user_dir(&base, "numdeck"), base.join(LEGACY_APP_DIR));

        // 新目录出现后立刻切新的
        std::fs::create_dir_all(base.join("numdeck")).unwrap();
        assert_eq!(pick_user_dir(&base, "numdeck"), base.join("numdeck"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
