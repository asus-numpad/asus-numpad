//! `lang`：查看/切换界面语言。

use std::path::Path;

use anyhow::Result;
use numdeck::i18n::{self, Lang};

use super::*;

/// `lang`：查看 / 切换界面语言。
pub(crate) fn cmd_lang(cfg_path: &Path, code: Option<&str>) -> Result<i32> {
    println!("{}", lang_current_line());
    let list = Lang::ALL
        .iter()
        .map(|l| format!("{}（{}）", l.code(), l.native_name()))
        .collect::<Vec<_>>()
        .join(&i18n::t("common.list_sep"));
    println!("{}", i18n::tf("lang.available", &[("list", &list)]));
    let dirs = i18n::search_dirs()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(&i18n::t("common.list_sep"));
    println!("{}", i18n::tf("lang.dirs", &[("dirs", &dirs)]));
    match i18n::current_source() {
        Some(p) => println!(
            "{}",
            i18n::tf("lang.file", &[("path", &p.display().to_string())])
        ),
        None => println!("  {}", i18n::t("lang.file_builtin")),
    }

    let Some(raw) = code else {
        println!();
        println!("{}", i18n::t("lang.usage"));
        return Ok(0);
    };
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("auto") {
        let backup = persist_config_keys(cfg_path, &[("language", serde_json::json!("auto"))])?;
        println!();
        println!(
            "{}",
            i18n::tf(
                "lang.wrote",
                &[
                    ("path", &cfg_path.display().to_string()),
                    ("value", "auto"),
                    ("backup", &backup.display().to_string()),
                ]
            )
        );
        println!(
            "{}",
            i18n::tf(
                "lang.effect_auto",
                &[("name", i18n::detect_system().native_name())]
            )
        );
        println!("{}", i18n::t("lang.restart_hint"));
        try_restart_service();
        return Ok(0);
    }
    match Lang::parse(raw) {
        Some(lang) => {
            let backup =
                persist_config_keys(cfg_path, &[("language", serde_json::json!(lang.code()))])?;
            println!();
            println!(
                "{}",
                i18n::tf(
                    "lang.wrote",
                    &[
                        ("path", &cfg_path.display().to_string()),
                        ("value", lang.code()),
                        ("backup", &backup.display().to_string()),
                    ]
                )
            );
            println!("{}", i18n::t("lang.restart_hint"));
            try_restart_service();
            Ok(0)
        }
        None => {
            println!();
            println!("{}", i18n::tf("lang.bad_code", &[("v", raw)]));
            println!("{}", i18n::t("lang.usage"));
            Ok(2)
        }
    }
}
