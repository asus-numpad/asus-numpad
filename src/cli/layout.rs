//! `layout`：查看/切换键位布局。

use std::path::Path;

use anyhow::Result;
use numdeck::config::Config;
use numdeck::i18n::{self};
use numdeck::keycodes as kc;
use numdeck::layout::{Geometry, KEY_GRID};

use super::*;

/// `layout`：查看 / 固定本机布局。
///
/// 不带参数时列出全部官方布局与支持级别；`layout auto` 恢复按 HID 硬件 ID 自动识别；
/// `layout GH3XLNDZ` 写死某套布局；`layout --ids` 额外列出 HID 硬件 ID 对照表。
pub(crate) fn cmd_layout(
    cfg: &Config,
    cfg_path: &Path,
    name: Option<&str>,
    ids: bool,
) -> Result<i32> {
    use numdeck::layout::{self, LayoutSupport};

    println!(
        "{}",
        i18n::tf(
            "layout.current",
            &[
                ("layout", &cfg.layout_name),
                ("resolved", cfg.layout_entry().name),
                ("support", cfg.layout_entry().support.as_str()),
            ]
        )
    );
    println!();
    println!("{}", i18n::t("layout.list_title"));
    for e in layout::LAYOUTS {
        let mark = match e.support {
            LayoutSupport::Verified => "*",
            LayoutSupport::Family => "~",
            _ => "-",
        };
        println!(
            "  {mark} {:<12} {:<12} cols={} i0={} i4={} l0={} l3={} nl={}/{} edges={}/{}/{}/{}",
            e.name,
            e.support.as_str(),
            e.layout.columns,
            e.layout.index0,
            e.layout.index4,
            e.layout.level0,
            e.layout.level3,
            e.layout.numlock_index,
            e.layout.numlock_level,
            e.layout.left,
            e.layout.right,
            e.layout.up,
            e.layout.down
        );
    }
    println!();
    println!("{}", i18n::t("layout.legend"));

    // 配置里装上的自定义布局也要列出来：否则用户配完 `custom_layouts` 在
    // `layout list` 里看不到任何痕迹，只能靠猜是否生效。
    let custom = numdeck::custom::list_custom();
    if !custom.is_empty() {
        println!();
        println!("{}", i18n::t("layout.custom_title"));
        for (cname, hids, rows) in &custom {
            println!(
                "  {}",
                i18n::tf(
                    "layout.custom_line",
                    &[
                        ("name", cname),
                        (
                            "cols",
                            &rows
                                .first()
                                .map(|r| r.split_whitespace().count())
                                .unwrap_or(0)
                                .to_string()
                        ),
                        (
                            "hids",
                            &if hids.is_empty() {
                                i18n::t("common.auto_detect")
                            } else {
                                hids.join("/")
                            }
                        ),
                    ]
                )
            );
        }
    }

    if ids {
        println!();
        println!("{}", i18n::t("layout.ids_title"));
        for (hid, lay) in layout::hid_list() {
            println!("  {hid} -> {lay}");
        }
    }

    let Some(raw) = name else {
        println!();
        println!("{}", i18n::t("layout.usage"));
        return Ok(0);
    };
    let raw = raw.trim();
    // `layout list` 是很多人的肌肉记忆，直接当"只看不改"，别回一句 Unknown layout name。
    if raw.eq_ignore_ascii_case("list") || raw.eq_ignore_ascii_case("show") {
        println!();
        println!("{}", i18n::t("layout.usage"));
        return Ok(0);
    }
    if raw.eq_ignore_ascii_case("auto") {
        let backup = persist_config_keys(cfg_path, &[("layout_name", serde_json::json!("auto"))])?;
        println!();
        println!(
            "{}",
            i18n::tf(
                "layout.wrote",
                &[
                    ("path", &cfg_path.display().to_string()),
                    ("value", "auto"),
                    ("backup", &backup.display().to_string()),
                ]
            )
        );
        println!("{}", i18n::t("layout.effect_auto"));
        try_restart_service();
        return Ok(0);
    }
    match layout::by_name(raw) {
        Some(entry) => {
            let backup =
                persist_config_keys(cfg_path, &[("layout_name", serde_json::json!(entry.name))])?;
            println!();
            println!(
                "{}",
                i18n::tf(
                    "layout.wrote",
                    &[
                        ("path", &cfg_path.display().to_string()),
                        ("value", entry.name),
                        ("backup", &backup.display().to_string()),
                    ]
                )
            );
            if entry.support.needs_confirm() {
                println!(
                    "{}",
                    i18n::tf(
                        "layout.unverified_warn",
                        &[("layout", entry.name), ("support", entry.support.as_str()),]
                    )
                );
                println!("{}", i18n::t("err.layout_unverified.hint"));
            }
            try_restart_service();
            Ok(0)
        }
        None => {
            println!();
            println!("{}", i18n::tf("layout.bad_name", &[("v", raw)]));
            println!("{}", i18n::t("layout.usage"));
            Ok(2)
        }
    }
}

pub(crate) fn render_keygrid(cfg: &Config) -> String {
    let mut out = String::new();
    for (r, row) in KEY_GRID.iter().enumerate() {
        let mut line = format!(
            "  {}: ",
            i18n::tf("check.keygrid_row", &[("r", &r.to_string())])
        );
        for (c, cell) in row.iter().enumerate() {
            let text = match cell {
                None => i18n::t("check.keygrid_numlock"),
                Some(cell) => numdeck::layout::key_for_cell(*cell, cfg.key_mode)
                    .map(|k| kc::name_of(k).to_string())
                    .unwrap_or_else(|| "?".into()),
            };
            let col = i18n::tf("check.keygrid_col", &[("c", &c.to_string())]);
            line.push_str(&format!("[{col} {text}] "));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

pub(crate) fn fallback_geometry(cfg: &Config) -> Geometry {
    // 无真机时用配置里的物理尺寸构造一个 0..1000 的虚拟坐标系。
    // 配了自定义分区（绝对坐标）时把虚拟面板撑到能容下它——否则在 mac 上跑 `check`
    // 永远看到「分区越界被拒」，等于没法离线验证分区配置。先撑到 1000 的整数倍。
    let (mut side_x, mut side_y) = (1000i32, 1000i32);
    if let Some(r) = cfg.partition.rect {
        if r[2] > side_x as f64 {
            side_x = (r[2] * 1.05).ceil() as i32;
        }
        if r[3] > side_y as f64 {
            side_y = (r[3] * 1.05).ceil() as i32;
        }
    }
    let width_um = cfg.touchpad_width_mm * 1000.0;
    let height_um = cfg.touchpad_height_mm * 1000.0;
    let ux = side_x as f64 / width_um;
    let uy = side_y as f64 / height_um;
    let panel = numdeck::partition::Panel {
        xmin: 0,
        xmax: side_x,
        ymin: 0,
        ymax: side_y,
        ux,
        uy,
    };
    cfg.build_geometry(panel, cfg.layout_entry().name).0
}
