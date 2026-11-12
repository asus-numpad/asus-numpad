//! 解算入口：把角点与网格样本解成面板参数和分区。
use serde::{Deserialize, Serialize};

use super::*;

/// 四角实测样本（用户按住触摸板四角时读到的坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct CornerSamples {
    pub top_left: (i32, i32),
    pub top_right: (i32, i32),
    pub bottom_left: (i32, i32),
    pub bottom_right: (i32, i32),
}

/// 一次交互采集的原始样本：四角 + 两个对角格中心。
///
/// 只装坐标，不装「是哪一格」——格由参数决定（左上格与右下格），
/// 免得用户在采集途中还要回答跟坐标无关的问题。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSamples {
    pub corners: CornerSamples,
    pub first_point: (i32, i32),
    pub last_point: (i32, i32),
}

/// 格中心实测样本：按住两个已知格的中心，用来反解格宽与网格原点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridProbe {
    /// 第一格 `(row, col)` 的中心落点
    pub first_cell: (usize, usize),
    pub first_point: (i32, i32),
    /// 最后一格 `(row, col)` 的中心落点
    pub last_cell: (usize, usize),
    pub last_point: (i32, i32),
    /// 键表列数 / 行数
    pub cols: usize,
    pub rows: usize,
}

/// 校准输出：直接落盘的两段配置。
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    pub panel: Panel,
    pub partition: PartitionSpec,
}

/// 由实测样本解算面板范围与网格矩形。
///
/// 数学很简单，难的是「样本本身是否可信」，所以这里把能查的都查一遍：
///
/// * 四角必须构成一个正的矩形（左右、上下不能反）；
/// * 两个探针格必须不等且落在键表内；
/// * 由探针反解出的格宽格高必须为正，且拼出的网格不能翻出面板。
///
/// 反解公式：格中心 `c = x0 + (col + 0.5) * w`，两式相减即
/// `w = (c_last - c_first) / (col_last - col_first)`，再回代求 `x0`。
pub fn solve_calibration(
    corners: &CornerSamples,
    probe: &GridProbe,
) -> Result<Calibration, String> {
    if probe.first_cell == probe.last_cell {
        return Err("calibrate: the two probes must be different cells".to_string());
    }
    if probe.first_cell.0 >= probe.rows
        || probe.last_cell.0 >= probe.rows
        || probe.first_cell.1 >= probe.cols
        || probe.last_cell.1 >= probe.cols
    {
        return Err(format!(
            "calibrate: probe cell outside {}x{} key table",
            probe.rows, probe.cols
        ));
    }
    let xs = [
        corners.top_left.0,
        corners.top_right.0,
        corners.bottom_left.0,
        corners.bottom_right.0,
    ];
    let ys = [
        corners.top_left.1,
        corners.top_right.1,
        corners.bottom_left.1,
        corners.bottom_right.1,
    ];
    let xmin = *xs.iter().min().expect("4 corners");
    let xmax = *xs.iter().max().expect("4 corners");
    let ymin = *ys.iter().min().expect("4 corners");
    let ymax = *ys.iter().max().expect("4 corners");
    if xmax == xmin || ymax == ymin {
        return Err("calibrate: corners collapse to a line/point".to_string());
    }
    // 左上角必须真的在左上：否则说明用户按错了顺序，或该轴是反的
    if corners.top_left.0 > corners.top_right.0 || corners.top_left.1 > corners.bottom_left.1 {
        return Err("calibrate: corner order looks wrong (is the Y axis inverted?)".to_string());
    }

    let dcol = probe.last_cell.1 as f64 - probe.first_cell.1 as f64;
    let drow = probe.last_cell.0 as f64 - probe.first_cell.0 as f64;
    let col_w = if dcol.abs() > 0.5 {
        (probe.last_point.0 - probe.first_point.0) as f64 / dcol
    } else {
        (xmax - xmin) as f64 / probe.cols as f64
    };
    let row_h = if drow.abs() > 0.5 {
        (probe.last_point.1 - probe.first_point.1) as f64 / drow
    } else {
        (ymax - ymin) as f64 / probe.rows as f64
    };
    if col_w <= 0.0 {
        return Err(format!("calibrate: derived column width {col_w:.1} <= 0"));
    }
    if row_h <= 0.0 {
        return Err(format!("calibrate: derived row height {row_h:.1} <= 0"));
    }
    let grid_x0 = probe.first_point.0 as f64 - (probe.first_cell.1 as f64 + 0.5) * col_w;
    let grid_y0 = probe.first_point.1 as f64 - (probe.first_cell.0 as f64 + 0.5) * row_h;
    let grid_x1 = grid_x0 + probe.cols as f64 * col_w;
    let grid_y1 = grid_y0 + probe.rows as f64 * row_h;

    // 网格允许比四角落点略外一点（用户按不到物理边缘），但不能离谱
    let slack_x = col_w * 0.5;
    let slack_y = row_h * 0.5;
    if grid_x0 < xmin as f64 - slack_x || grid_x1 > xmax as f64 + slack_x {
        return Err(format!(
            "calibrate: derived grid x {grid_x0:.0}..{grid_x1:.0} outside panel {xmin}..{xmax}"
        ));
    }
    if grid_y0 < ymin as f64 - slack_y || grid_y1 > ymax as f64 + slack_y {
        return Err(format!(
            "calibrate: derived grid y {grid_y0:.0}..{grid_y1:.0} outside panel {ymin}..{ymax}"
        ));
    }

    let panel = Panel {
        xmin,
        xmax,
        ymin,
        ymax,
        // 分辨率未知：交给上层按标称物理尺寸换算（与内核没报 resolution 时同一路径）
        ux: 0.0,
        uy: 0.0,
    };
    let partition = PartitionSpec {
        layout: "auto".to_string(),
        rect: Some([
            grid_x0.round(),
            grid_y0.round(),
            grid_x1.round(),
            grid_y1.round(),
        ]),
        rect_mm: None,
        cols: Some(probe.cols),
        rows: Some(probe.rows),
        col_ratios: Vec::new(),
        row_ratios: Vec::new(),
        numlock_cell: None,
        snap_outside: false,
    };
    Ok(Calibration { panel, partition })
}
