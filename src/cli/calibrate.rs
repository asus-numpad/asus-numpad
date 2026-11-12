//! `calibrate`：手动校准。

use std::path::Path;

use anyhow::{Context, Result};
use numdeck::banner;
use numdeck::config::Config;
use numdeck::i18n::{self};

use super::*;

/// 读 `--samples` 文件：只认 `corners` / `first_point` / `last_point`。
///
/// 样本数据是纯坐标，不含任何机型信息——同一次采集在 mac 上解算和在真机上解算
/// 走的是同一段代码，离线调算法时不用占着触摸板。
pub(crate) fn read_raw_samples(path: &Path) -> Result<numdeck::partition::RawSamples> {
    use numdeck::partition::{CornerSamples, RawSamples};

    fn pair_at(obj: &serde_json::Value, name: &str) -> Result<(i32, i32)> {
        // anyhow 直接把 String 当错误装进去时 Display 会自带引号
        // （`Error: "..." ...`），用格式化包一层，报错文案就是语言文件里那句。
        fn bad(name: &str) -> anyhow::Error {
            anyhow::anyhow!("{}", i18n::tf("calibrate.samples.bad_pair", &[("k", name)]))
        }
        let arr = obj
            .get(name)
            .and_then(|x| x.as_array())
            .filter(|a| a.len() == 2)
            .ok_or_else(|| bad(name))?;
        let x = arr[0].as_i64().ok_or_else(|| bad(name))?;
        let y = arr[1].as_i64().ok_or_else(|| bad(name))?;
        Ok((x as i32, y as i32))
    }

    let text = std::fs::read_to_string(path).with_context(|| {
        i18n::tf(
            "calibrate.samples.read_failed",
            &[("path", &path.display().to_string())],
        )
    })?;
    let v: serde_json::Value = serde_json::from_str(&text).with_context(|| {
        i18n::tf(
            "calibrate.samples.bad_json",
            &[("path", &path.display().to_string())],
        )
    })?;
    let corners = v.get("corners").ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            i18n::tf("calibrate.samples.missing", &[("k", "corners")])
        )
    })?;
    Ok(RawSamples {
        corners: CornerSamples {
            top_left: pair_at(corners, "top_left")?,
            top_right: pair_at(corners, "top_right")?,
            bottom_left: pair_at(corners, "bottom_left")?,
            bottom_right: pair_at(corners, "bottom_right")?,
        },
        first_point: pair_at(&v, "first_point")?,
        last_point: pair_at(&v, "last_point")?,
    })
}

pub(crate) fn cmd_calibrate(
    cfg: &Config,
    cfg_path: &Path,
    samples: Option<&str>,
    write: bool,
    cols: Option<usize>,
    rows: Option<usize>,
) -> Result<i32> {
    use numdeck::partition::{solve_calibration, GridProbe};

    let table = cfg.key_table();
    let keys = (
        cols.unwrap_or_else(|| table.cols()),
        rows.unwrap_or_else(|| table.rows().len()),
    );
    println!("{}", banner());
    println!();
    if !(1..=8).contains(&keys.0) || !(1..=8).contains(&keys.1) {
        println!(
            "{}",
            i18n::tf(
                "calibrate.bad_keys",
                &[("cols", &keys.0.to_string()), ("rows", &keys.1.to_string())]
            )
        );
        return Ok(2);
    }

    // 取样本：文件优先，其次真机交互采集（仅 Linux）
    let raw = match samples {
        Some(p) => {
            println!("{}", i18n::tf("calibrate.from_samples", &[("path", p)]));
            println!();
            read_raw_samples(Path::new(p))?
        }
        None => {
            #[cfg(target_os = "linux")]
            {
                let dev = numdeck::linux::find_touchpad(cfg)?;
                let raw = numdeck::linux::calibrate::collect(&dev)?;
                println!();
                raw
            }
            #[cfg(not(target_os = "linux"))]
            {
                println!("{}", i18n::t("calibrate.need_linux"));
                return Ok(2);
            }
        }
    };

    // 两个探针固定取左上格与右下格：对角两点就能同时定出格宽格高
    let probe = GridProbe {
        first_cell: (0, 0),
        first_point: raw.first_point,
        last_cell: (keys.1 - 1, keys.0 - 1),
        last_point: raw.last_point,
        cols: keys.0,
        rows: keys.1,
    };
    let cal = match solve_calibration(&raw.corners, &probe) {
        Ok(c) => c,
        Err(err) => {
            println!("{}", i18n::tf("calibrate.fail", &[("err", &err)]));
            return Ok(2);
        }
    };

    let rect = cal
        .partition
        .rect
        .expect("solve_calibration 必然给出绝对矩形");
    let col_w = (rect[2] - rect[0]) / keys.0 as f64;
    let row_h = (rect[3] - rect[1]) / keys.1 as f64;

    println!("{}", i18n::t("calibrate.ok_title"));
    println!();
    println!(
        "{}",
        i18n::tf(
            "calibrate.panel",
            &[
                ("x0", &cal.panel.xmin.to_string()),
                ("x1", &cal.panel.xmax.to_string()),
                ("y0", &cal.panel.ymin.to_string()),
                ("y1", &cal.panel.ymax.to_string()),
            ]
        )
    );
    println!(
        "{}",
        i18n::tf(
            "calibrate.grid",
            &[
                ("cols", &keys.0.to_string()),
                ("rows", &keys.1.to_string()),
                ("x0", &format!("{:.0}", rect[0])),
                ("y0", &format!("{:.0}", rect[1])),
                ("x1", &format!("{:.0}", rect[2])),
                ("y1", &format!("{:.0}", rect[3])),
            ]
        )
    );
    println!(
        "{}",
        i18n::tf(
            "calibrate.cell_size",
            &[("w", &format!("{col_w:.1}")), ("h", &format!("{row_h:.1}")),]
        )
    );
    println!();
    // 格中心表：真机上照着点一遍，哪一个格子不出键一眼就能看出来
    println!("{}", i18n::t("calibrate.grid_title"));
    for r in 0..keys.1 {
        let mut line = String::new();
        for c in 0..keys.0 {
            let x = rect[0] + (c as f64 + 0.5) * col_w;
            let y = rect[1] + (r as f64 + 0.5) * row_h;
            line.push_str(&format!("[{c},{r}] ({x:.0},{y:.0})  "));
        }
        println!("  {}", line.trim_end());
    }
    println!();

    let partition = serde_json::to_value(&cal.partition)?;
    if write {
        // 打印真实写入的那份配置，而不是 persist 返回的 .bak（那是备份路径）
        persist_config_keys(cfg_path, &[("partition", partition.clone())])?;
        let real = Config::writable_path(cfg_path);
        println!(
            "{}",
            i18n::tf(
                "calibrate.written",
                &[("path", &real.display().to_string())]
            )
        );
        return Ok(0);
    }
    println!("{}", i18n::t("calibrate.snippet_title"));
    println!();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({ "partition": partition }))?
    );
    println!();
    println!("{}", i18n::t("calibrate.snippet_hint"));
    Ok(0)
}
