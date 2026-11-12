//! 网格分区：切几列几行、边界权重、解算出的切点，以及分区提示。
use serde::{Deserialize, Serialize};

use super::*;

/// 自定义分区：把网格换成任意行列划分。
///
/// 四个边界来源按优先级：`rect`（绝对坐标）> `rect_mm`（相对面板左上角的毫米）>
/// 官方参数推算出的网格。只写 `cols` / `rows` / 比例而不写矩形，就只改划分、
/// 不动位置——「触摸板对、只是键位排布不一样」的机型走这条。
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct PartitionSpec {
    /// 对哪套布局生效：`auto`（当前生效布局）/ 具体布局名 / 自定义布局名。
    pub layout: String,
    /// 网格矩形 `[x0, y0, x1, y1]`，触摸板绝对坐标。
    pub rect: Option<[f64; 4]>,
    /// 网格矩形 `[x0, y0, x1, y1]`，相对面板左上角的毫米。
    pub rect_mm: Option<[f64; 4]>,
    /// 列数；留空跟随键表。
    pub cols: Option<usize>,
    /// 行数；留空跟随键表。
    pub rows: Option<usize>,
    /// 列宽权重（长度须等于列数，可不等分）；留空 = 等分。
    pub col_ratios: Vec<f64>,
    /// 行高权重；留空 = 等分。
    pub row_ratios: Vec<f64>,
    /// NumLock 图标所在格 `[row, col]`；留空 = 第 0 行最后一列。
    pub numlock_cell: Option<[usize; 2]>,
    /// 网格外落点吸附到最近一格（默认关闭，与官方「网格外属于光标区」一致）。
    pub snap_outside: bool,
}

/// 分区生效范围判定是否有意义——全空时整段不参与。
impl PartitionSpec {
    pub fn is_unset(&self) -> bool {
        self.rect.is_none()
            && self.rect_mm.is_none()
            && self.cols.is_none()
            && self.rows.is_none()
            && self.col_ratios.is_empty()
            && self.row_ratios.is_empty()
            && self.numlock_cell.is_none()
            && !self.snap_outside
    }

    /// 是否对这套布局生效（`auto` / 空 = 一律生效）。
    pub fn applies_to(&self, layout_name: &str) -> bool {
        let want = self.layout.trim();
        want.is_empty()
            || want.eq_ignore_ascii_case("auto")
            || want.eq_ignore_ascii_case(layout_name)
    }

    /// 解算成可直接喂给 [`crate::layout::Geometry::with_partition`] 的边界。
    ///
    /// `base_grid` 是官方参数推出来的网格矩形，`keys` 是键表的 `(列数, 行数)`。
    pub fn resolve(
        &self,
        panel: Panel,
        base_grid: (f64, f64, f64, f64),
        keys: (usize, usize),
    ) -> Result<ResolvedPartition, String> {
        let cols = self.cols.unwrap_or(keys.0.max(1));
        let rows = self.rows.unwrap_or(keys.1.max(1));
        if !(1..=8).contains(&cols) {
            return Err(format!("partition: cols {cols} outside 1..=8"));
        }
        if !(1..=8).contains(&rows) {
            return Err(format!("partition: rows {rows} outside 1..=8"));
        }
        let (x0, y0, x1, y1) = if let Some(r) = self.rect {
            (r[0], r[1], r[2], r[3])
        } else if let Some(m) = self.rect_mm {
            (
                panel.mm_to_x(m[0]),
                panel.mm_to_y(m[1]),
                panel.mm_to_x(m[2]),
                panel.mm_to_y(m[3]),
            )
        } else {
            base_grid
        };
        if x1 <= x0 || y1 <= y0 {
            return Err(format!(
                "partition: rect [{x0:.0}, {y0:.0}, {x1:.0}, {y1:.0}] is empty or inverted"
            ));
        }
        if x0 < panel.xmin as f64 || x1 > panel.xmax as f64 {
            return Err(format!(
                "partition: x span {x0:.0}..{x1:.0} outside panel {}..{}",
                panel.xmin, panel.xmax
            ));
        }
        if y0 < panel.ymin as f64 || y1 > panel.ymax as f64 {
            return Err(format!(
                "partition: y span {y0:.0}..{y1:.0} outside panel {}..{}",
                panel.ymin, panel.ymax
            ));
        }
        let x_edges = split(x0, x1, cols, &self.col_ratios, "col")?;
        let y_edges = split(y0, y1, rows, &self.row_ratios, "row")?;
        let nl_cell = self
            .numlock_cell
            .map(|c| (c[0], c[1]))
            .unwrap_or((0, cols - 1));
        if nl_cell.0 >= rows || nl_cell.1 >= cols {
            return Err(format!(
                "partition: numlock cell ({}, {}) outside {rows}x{cols} grid",
                nl_cell.0, nl_cell.1
            ));
        }
        Ok(ResolvedPartition {
            cols,
            rows,
            x_edges,
            y_edges,
            nl_cell: Some(nl_cell),
            snap_outside: self.snap_outside,
            notes: partition_notes(cols, rows, keys),
        })
    }
}

/// 分区与键表不匹配时给用户看的提醒（键表只覆盖重叠部分，多的那部分不出键）。
pub(super) fn partition_notes(
    cols: usize,
    rows: usize,
    keys: (usize, usize),
) -> Vec<PartitionNote> {
    let mut notes = Vec::new();
    if cols > keys.0 {
        notes.push(PartitionNote::ExtraCols(keys.0, cols));
    }
    if rows > keys.1 {
        notes.push(PartitionNote::ExtraRows(keys.1, rows));
    }
    notes
}

/// 分区提示项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionNote {
    /// 网格列数多于键表列数：多出来的列不出键
    ExtraCols(usize, usize),
    /// 网格行数多于键表行数：多出来的行不出键
    ExtraRows(usize, usize),
}

impl PartitionNote {
    pub fn key(self) -> &'static str {
        match self {
            PartitionNote::ExtraCols(..) => "note.partition_extra_cols",
            PartitionNote::ExtraRows(..) => "note.partition_extra_rows",
        }
    }

    pub fn args(self) -> Vec<(&'static str, String)> {
        match self {
            PartitionNote::ExtraCols(have, want) => {
                vec![("have", have.to_string()), ("want", want.to_string())]
            }
            PartitionNote::ExtraRows(have, want) => {
                vec![("have", have.to_string()), ("want", want.to_string())]
            }
        }
    }
}

/// 解算结果。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPartition {
    pub cols: usize,
    pub rows: usize,
    pub x_edges: Vec<f64>,
    pub y_edges: Vec<f64>,
    pub nl_cell: Option<(usize, usize)>,
    pub snap_outside: bool,
    pub notes: Vec<PartitionNote>,
}

/// 把 `[0,1]` 区间按权重切成 `n` 段的边界（长度 `n+1`）。
pub(super) fn split(
    lo: f64,
    hi: f64,
    n: usize,
    ratios: &[f64],
    what: &str,
) -> Result<Vec<f64>, String> {
    let span = hi - lo;
    if ratios.is_empty() {
        return Ok((0..=n).map(|i| lo + span * i as f64 / n as f64).collect());
    }
    if ratios.len() != n {
        return Err(format!(
            "partition: {what}_ratios has {} entries, need {n}",
            ratios.len()
        ));
    }
    if ratios.iter().any(|r| !(r.is_finite() && *r > 0.0)) {
        return Err(format!("partition: {what}_ratios must all be positive"));
    }
    let total: f64 = ratios.iter().sum();
    let mut out = Vec::with_capacity(n + 1);
    out.push(lo);
    let mut acc = 0.0;
    for r in ratios {
        acc += r;
        out.push(lo + span * acc / total);
    }
    // 浮点累加的末项直接钉到 hi，避免 1 ulp 误差让最后一格比矩形窄
    if let Some(last) = out.last_mut() {
        *last = hi;
    }
    Ok(out)
}

// --------------------------------------------------------------------------- //
// 校准求解
// --------------------------------------------------------------------------- //
