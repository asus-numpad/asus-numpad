//! `sku`：机型识别结果与手动指定。

use std::path::Path;

use anyhow::Result;
// 只被 linux 分支的闸门用到，非 Linux 上导入会变成未使用告警。
#[cfg(target_os = "linux")]
use numdeck::banner;
use numdeck::config::Config;
use numdeck::i18n::{self};
use numdeck::sku;

use super::*;

/// 机型闸门：这台机器根本跑不起来的，就在启动前说清楚。
///
/// 本包只跑机型表里的机器：DMI 型号与触摸板模组都认不出时不再兜底，直接拦下。
/// 返回 `Some(退出码)` 表示已拦下，`None` 表示放行（`2` = `--sku` 给的 id 无效，
/// `3` = 本机不在支持列表）。
#[cfg(target_os = "linux")]
pub(crate) fn sku_gate(cfg: &Config) -> Option<i32> {
    let identity = sku::dmi_identity();
    let hw = sku::probe_hardware();
    let ids = sku::known_ids().join(", ");
    match sku::resolve(&identity, &hw, cfg.sku.as_deref()) {
        Ok(_) => None,
        Err(sku::ResolveError::UnknownId(e)) => {
            println!("{}", banner());
            println!("{}", i18n::tf("sku.bad_id", &[("id", &e.0), ("ids", &ids)]));
            Some(2)
        }
        Err(sku::ResolveError::Unsupported(u)) => {
            println!("{}", banner());
            println!("{}", unsupported_text(&u, &ids));
            // 「已拒绝启动」只在真去启动时才说：unsupported_text 也被 check 复用，
            // 那里没有可拒绝的启动过程。
            println!("{}", i18n::t("err.unsupported_stop"));
            println!("{}", i18n::t("err.unsupported_hint"));
            Some(3)
        }
    }
}

/// 挡掉没有 NumberPad 硬件的机器：DMI 型号对得上也不放行。
///
/// 判据是触摸板 HID 接口里有没有 FEATURE 报告 `0x0d`。4 区 RGB 键盘的配置（国行
/// 「魔霸新锐」、同型号的减配档）用同一块触摸板模组，DMI 与模组都分不出来，但描述符里
/// 没有这条 FEATURE——数字区背光和图标区都不存在，启动起来只会白抓触摸板。
///
/// **ARM64 构建不做这道闸门**：`0x0d` 是华硕自家 NumberPad 模组的报告，非华硕触摸板
/// 上根本不会出现；ARM 包既没有机型表也不认硬件，拦下去等于把每一台机器都挡在门外。
#[cfg(target_os = "linux")]
pub(crate) fn numpad_gate() -> Option<i32> {
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        // 自定义模式下硬件在不在、报文长什么样都由配置决定，启动阶段没有可判的东西。
        None
    }

    #[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
    {
        use numdeck::linux::{probe_numpad_presence, NumpadPresence};
        match probe_numpad_presence() {
            NumpadPresence::Absent => {
                println!("{}", banner());
                println!("{}", i18n::t("err.no_numpad"));
                println!("{}", i18n::t("err.no_numpad.hint"));
                Some(3)
            }
            NumpadPresence::NoAccess => {
                // 读不到描述符是权限问题，不能当成「没有这块硬件」把人挡回去
                eprintln!("{}", i18n::t("err.numpad_no_access"));
                None
            }
            NumpadPresence::Present | NumpadPresence::Unknown => None,
        }
    }
}

/// 「这台不在支持列表」的两条提示：厂商明确不是 ASUS / 只是认不出机型。
pub(crate) fn unsupported_text(u: &sku::Unsupported, ids: &str) -> String {
    match &u.non_asus {
        Some(vendor) => i18n::tf("err.not_asus", &[("vendor", vendor)]),
        None => i18n::tf("err.unsupported_model", &[("ids", ids)]),
    }
}

/// 输出「本机用哪套机型配置 + 凭什么判定」。
pub(crate) fn sku_block(cfg: &Config) -> String {
    // DMI 与 hidraw 只有 Linux 上有：在别的系统上跑自检时如实说「判定不了」，
    // 不能拿「认不出」当「不支持」，那会把开发机误导成硬件不兼容。
    if !cfg!(target_os = "linux") {
        return i18n::t("sku.nonlinux");
    }
    let identity = sku::dmi_identity();
    let hw = sku::probe_hardware();
    let ids = sku::known_ids().join(", ");
    let mut out = match sku::resolve(&identity, &hw, cfg.sku.as_deref()) {
        Ok(r) => {
            let mut s = i18n::tf(
                "sku.model",
                &[
                    ("sku", &r.sku.localized_display()),
                    ("source", &i18n::t(r.source.i18n_key())),
                ],
            );
            s.push('\n');
            if let Some(detail) = &r.detail {
                s.push_str(&i18n::tf("sku.detail", &[("detail", detail)]));
                s.push('\n');
            }
            s.push_str(&i18n::tf("sku.note", &[("note", &r.sku.localized_note())]));
            s
        }
        Err(sku::ResolveError::UnknownId(e)) => {
            i18n::tf("sku.bad_id", &[("id", &e.0), ("ids", &ids)])
        }
        Err(sku::ResolveError::Unsupported(u)) => unsupported_text(&u, &ids),
    };
    out.push('\n');
    if identity.haystacks().is_empty() {
        out.push_str(&i18n::t("sku.dmi_unknown"));
    } else {
        out.push_str(&i18n::tf("sku.dmi", &[("dmi", &identity.label())]));
    }
    out.push('\n');
    if !hw.is_empty() {
        out.push_str(&i18n::tf("sku.hw", &[("hw", &hw.label())]));
        out.push('\n');
    }
    // 机型兼容情况只维护在官网，程序内不再写死「哪台机器上过真机」
    out.push_str(&i18n::t("sku.compat"));
    out.push('\n');
    out
}

/// `sku` 子命令：查看自动识别结果 / 列出可选机型 / 手动指定或恢复自动识别。
pub(crate) fn cmd_sku(cfg: &Config, cfg_path: &Path, arg: Option<&str>) -> Result<i32> {
    let ids = sku::known_ids().join(", ");
    match arg.map(str::trim).filter(|a| !a.is_empty()) {
        None => {
            println!("{}", sku_block(cfg));
            Ok(0)
        }
        Some(a) if a.eq_ignore_ascii_case("list") => {
            println!("{}", i18n::tf("sku.list", &[("ids", &ids)]));
            for entry in sku::SKUS {
                println!("  {:<10} {}", entry.id, entry.display);
            }
            Ok(0)
        }
        Some(a) if a.eq_ignore_ascii_case("auto") => {
            let target = Config::writable_path(cfg_path);
            persist_config_keys(cfg_path, &[("sku", serde_json::Value::Null)])?;
            println!(
                "{}",
                i18n::tf("sku.clear", &[("path", &target.display().to_string())])
            );
            Ok(0)
        }
        Some(a) => match sku::by_id(a) {
            Some(entry) => {
                let target = Config::writable_path(cfg_path);
                persist_config_keys(cfg_path, &[("sku", serde_json::json!(entry.id))])?;
                println!(
                    "{}",
                    i18n::tf(
                        "sku.set",
                        &[
                            ("sku", entry.display),
                            ("path", &target.display().to_string()),
                        ]
                    )
                );
                Ok(0)
            }
            None => {
                eprintln!("{}", i18n::tf("sku.bad_id", &[("id", a), ("ids", &ids)]));
                Ok(2)
            }
        },
    }
}
