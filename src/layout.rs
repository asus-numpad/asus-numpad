//! GH3XLNDZ 布局几何：把官方 `NUMPADLAYOUT_GH3XLNDZ_00..14`（单位 0.1 mm）
//! 换算成触摸板绝对坐标矩形，并完成「坐标 → 键码」判定。
//!
//! 官方参数语义（AsusNUMPADFilter.inf 注释）：
//!   _00 列数；_01/_02 第 0/4 列外侧留白；_03/_04 第 0/3 行外侧留白；
//!   _07.._10 有效区四边内缩；_05/_06 NumLockEdge；_13/_14 NumLock 无效边。
//!
//! 实现约定：有效区 = 触摸板 - (left/right/up/down)；
//!           键网格 = 有效区再内缩 index0/index4、level0/level3 后等分 5 列 4 行。

use crate::keycodes as kc;

/// 官方布局参数单位 = 0.1 mm = 100 µm。
pub const UM_PER_LAYOUT_UNIT: i32 = 100;

/// GH3XLNDZ（ROG Strix SCAR 16 G634JY 等）官方 15 个布局参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub columns: i32,
    /// _01 第 0 列左侧留 1.7 mm
    pub index0: i32,
    /// _02 第 4 列右侧留 2.0 mm
    pub index4: i32,
    /// _03 第 0 行上侧留 1.5 mm
    pub level0: i32,
    /// _04 第 3 行下侧留 2.0 mm
    pub level3: i32,
    /// _05 NumLockEdge Index 3.1 mm（当前实现不用于热区，见 `Geometry::new` 说明）
    pub numlock_index: i32,
    /// _06 NumLockEdge Level 1.8 mm
    pub numlock_level: i32,
    /// _07 有效区左边内缩 1.3 mm
    pub left: i32,
    /// _08 有效区右边内缩 1.1 mm
    pub right: i32,
    /// _09 有效区上边内缩 0.9 mm
    pub up: i32,
    /// _10 有效区下边内缩 0.0 mm
    pub down: i32,
    pub numadv_index: i32,
    pub numadv_level: i32,
    /// _13 NumLock InValidEdge Index 1.1 mm
    pub invalid_index: i32,
    /// _14 NumLock InValidEdge Level 0.9 mm
    pub invalid_level: i32,
}

/// 官方 GH3XLNDZ 原值（禁止随意改动，改动即偏离官方）。
pub const GH3XLNDZ: Layout = Layout {
    columns: 5,
    index0: 17,
    index4: 20,
    level0: 15,
    level3: 20,
    numlock_index: 31,
    numlock_level: 18,
    left: 13,
    right: 11,
    up: 9,
    down: 0,
    numadv_index: 0,
    numadv_level: 0,
    invalid_index: 11,
    invalid_level: 9,
};

/// 官方 GVM 原值：冰刃双屏的侧置触摸板（`GX550` / `GX551` / `GX650`，模组 `ASUE1404` /
/// `ASUE1408`）。灵耀双屏 `UX582` / `UX482` 走的是 `NVM`，不用这一份。
///
/// 官方 inf 对 `NUMPADLAYOUT_GVM_*` 只列到 `_06`（列数、左右留白、上下留白、NumLockEdge），
/// `_07` 往后的有效区内缩与 NumAdv / InValid 边没有给出——Windows 驱动读不到的注册表值即为
/// 缺省 0，所以这里同样填 0，不做推测。`_05` / `_06` 的注释写明是给 GVMA 用的，
/// 因此 `GVM_A` 与本布局共用这一份参数。
pub const GVM: Layout = Layout {
    columns: 5,
    index0: 14,
    index4: 14,
    level0: 11,
    level3: 12,
    numlock_index: 14,
    numlock_level: 14,
    left: 0,
    right: 0,
    up: 0,
    down: 0,
    numadv_index: 0,
    numadv_level: 0,
    invalid_index: 0,
    invalid_level: 0,
};

/// 网格格子内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Digit(u8),
    Period,
    Backspace,
    Return,
    Slash,
    Asterisk,
    Minus,
    Plus,
}

/// 格子内容 → 短名（`7` `/` `bs` …），供键表展示复用。
pub fn cell_label(cell: Cell) -> &'static str {
    match cell {
        Cell::Digit(0) => "0",
        Cell::Digit(1) => "1",
        Cell::Digit(2) => "2",
        Cell::Digit(3) => "3",
        Cell::Digit(4) => "4",
        Cell::Digit(5) => "5",
        Cell::Digit(6) => "6",
        Cell::Digit(7) => "7",
        Cell::Digit(8) => "8",
        Cell::Digit(9) => "9",
        Cell::Period => ".",
        Cell::Backspace => "bs",
        Cell::Return => "ret",
        Cell::Slash => "/",
        Cell::Asterisk => "*",
        Cell::Minus => "-",
        Cell::Plus => "+",
        // 键表只使用 0..=9；越界值不构造，真出现时给个明确占位而不是 panic
        Cell::Digit(_) => "?",
    }
}

pub const ROWS: usize = 4;
pub const COLS: usize = 5;

/// GH3XLNDZ 5 列 x 4 行键表；`None` 表示无键（第 0 行第 5 格是 NumLock 图标区）。
pub const KEY_GRID: [[Option<Cell>; COLS]; ROWS] = [
    [
        Some(Cell::Digit(7)),
        Some(Cell::Digit(8)),
        Some(Cell::Digit(9)),
        Some(Cell::Slash),
        None,
    ],
    [
        Some(Cell::Digit(4)),
        Some(Cell::Digit(5)),
        Some(Cell::Digit(6)),
        Some(Cell::Asterisk),
        Some(Cell::Backspace),
    ],
    [
        Some(Cell::Digit(1)),
        Some(Cell::Digit(2)),
        Some(Cell::Digit(3)),
        Some(Cell::Minus),
        Some(Cell::Return),
    ],
    [
        Some(Cell::Digit(0)),
        Some(Cell::Digit(0)),
        Some(Cell::Period),
        Some(Cell::Plus),
        Some(Cell::Return),
    ],
];

/// 键码风格：keypad = KEY_KP*；digits = 主键区 KEY_0-9。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    Keypad,
    Digits,
}

impl KeyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            KeyMode::Keypad => "keypad",
            KeyMode::Digits => "digits",
        }
    }
}

impl std::str::FromStr for KeyMode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "keypad" => Ok(KeyMode::Keypad),
            "digits" => Ok(KeyMode::Digits),
            _ => Err(()),
        }
    }
}

/// 格子 → Linux 键码。
pub fn key_for_cell(cell: Cell, mode: KeyMode) -> Option<u16> {
    let code = match cell {
        Cell::Backspace => kc::KEY_BACKSPACE,
        Cell::Return => kc::KEY_KPENTER,
        Cell::Slash => kc::KEY_KPSLASH,
        Cell::Asterisk => kc::KEY_KPASTERISK,
        Cell::Minus => kc::KEY_KPMINUS,
        Cell::Plus => kc::KEY_KPPLUS,
        Cell::Period => match mode {
            KeyMode::Keypad => kc::KEY_KPDOT,
            KeyMode::Digits => kc::KEY_DOT,
        },
        Cell::Digit(d) => match mode {
            KeyMode::Keypad => match d {
                0 => kc::KEY_KP0,
                1 => kc::KEY_KP1,
                2 => kc::KEY_KP2,
                3 => kc::KEY_KP3,
                4 => kc::KEY_KP4,
                5 => kc::KEY_KP5,
                6 => kc::KEY_KP6,
                7 => kc::KEY_KP7,
                8 => kc::KEY_KP8,
                9 => kc::KEY_KP9,
                _ => return None,
            },
            KeyMode::Digits => match d {
                0 => kc::KEY_0,
                1 => kc::KEY_1,
                2 => kc::KEY_2,
                3 => kc::KEY_3,
                4 => kc::KEY_4,
                5 => kc::KEY_5,
                6 => kc::KEY_6,
                7 => kc::KEY_7,
                8 => kc::KEY_8,
                9 => kc::KEY_9,
                _ => return None,
            },
        },
    };
    Some(code)
}

/// 命中容差：`mm` 为毫米阈值（0 表示不吸附，直接按格子取键），
/// `ux` / `uy` 是按触摸板分辨率换算成采样单位的同一阈值。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tol {
    pub mm: f64,
    pub ux: f64,
    pub uy: f64,
}

impl Tol {
    /// 由毫米阈值与触摸板分辨率换算：两个参数分别是每毫米的 X / Y 采样单位数。
    pub fn new(mm: f64, ux_per_mm: f64, uy_per_mm: f64) -> Self {
        Self {
            mm,
            ux: mm * ux_per_mm,
            uy: mm * uy_per_mm,
        }
    }

    /// 不吸附：直接按落在哪一格取键。
    pub fn exact() -> Self {
        Self {
            mm: 0.0,
            ux: 0.0,
            uy: 0.0,
        }
    }
}

/// 已换算成触摸板单位的几何矩形。
#[derive(Debug, Clone)]
pub struct Geometry {
    pub layout: Layout,
    /// 官方布局名（`GH3XLNDZ` 等），用于日志与自检输出。
    pub layout_name: &'static str,
    pub xmin: i32,
    pub xmax: i32,
    pub ymin: i32,
    pub ymax: i32,
    /// units per µm（X / Y）
    pub ux: f64,
    pub uy: f64,
    pub usable_x0: f64,
    pub usable_x1: f64,
    pub usable_y0: f64,
    pub usable_y1: f64,
    pub grid_x0: f64,
    pub grid_x1: f64,
    pub grid_y0: f64,
    pub grid_y1: f64,
    pub col_w: f64,
    pub row_h: f64,
    pub nl_x0: f64,
    pub nl_x1: f64,
    pub nl_y0: f64,
    pub nl_y1: f64,
    // ---- 自定义分区（官方均匀网格之外的划分） ----
    /// 网格列数。默认等于 `layout.columns`；自定义分区可以与之不同。
    pub grid_cols: usize,
    /// 网格行数。默认 [`ROWS`]；自定义分区可以与之不同。
    pub grid_rows: usize,
    /// 列边界：升序，长度 `grid_cols + 1`；均匀网格由 `grid_x0`/`col_w` 生成。
    pub col_edges: Vec<f64>,
    /// 行边界：升序，长度 `grid_rows + 1`。
    pub row_edges: Vec<f64>,
    /// NumLock 图标所在格 `(row, col)`，默认第 0 行最后一列。
    pub nl_cell: (usize, usize),
    /// 网格外落点是否吸附到最近一格。
    ///
    /// 默认 `false`：与官方一致，网格之外属于触摸板光标区，绝不凭空出键。
    /// 只给「分区未覆盖整块触摸板、但希望整块都算数字区」的表外机型显式开启。
    pub snap_outside: bool,
}

impl Geometry {
    /// `ux`/`uy` 为 units per µm。
    pub fn new(
        xmin: i32,
        xmax: i32,
        ymin: i32,
        ymax: i32,
        ux: f64,
        uy: f64,
        layout: Layout,
    ) -> Self {
        let l = layout;
        let mm = UM_PER_LAYOUT_UNIT as f64; // 官方单位 0.1 mm
        let ux_units = |v: i32| v as f64 * mm * ux;
        let uy_units = |v: i32| v as f64 * mm * uy;

        let usable_x0 = xmin as f64 + ux_units(l.left);
        let usable_x1 = xmax as f64 - ux_units(l.right);
        let usable_y0 = ymin as f64 + uy_units(l.up);
        let usable_y1 = ymax as f64 - uy_units(l.down);

        let cols = l.columns as f64;
        let rows = ROWS as f64;

        let grid_x0 = usable_x0 + ux_units(l.index0);
        let grid_x1 = usable_x1 - ux_units(l.index4);
        let grid_y0 = usable_y0 + uy_units(l.level0);
        let grid_y1 = usable_y1 - uy_units(l.level3);

        let col_w = (grid_x1 - grid_x0) / cols;
        let row_h = (grid_y1 - grid_y0) / rows;

        // NumLock 区：统一按第 0 行第 5 列整格判定。
        // 官方 _05/_06（NumLockEdge 3.1 / 1.8 mm）与 _13/_14（InValidEdge 1.1 / 0.9 mm）
        // 只能确定图标相对触摸板右/上边缘的位置，宽度与高度语义未在 INF 闭合；
        // 由它们反推出的子区仅约 2 mm，真机上几乎点不中，属于凭空造出来的热区。
        // 因此不提供 icon 子区，避免「按了没反应」的错误交互。
        let cell_x0 = grid_x0 + (cols - 1.0) * col_w;
        let nl_x0 = cell_x0;
        let nl_x1 = grid_x1;
        let nl_y0 = grid_y0;
        let nl_y1 = grid_y0 + row_h;

        let grid_cols = l.columns.max(1) as usize;
        let grid_rows = ROWS;
        let col_edges: Vec<f64> = (0..=grid_cols)
            .map(|i| grid_x0 + i as f64 * col_w)
            .collect();
        let row_edges: Vec<f64> = (0..=grid_rows)
            .map(|i| grid_y0 + i as f64 * row_h)
            .collect();

        Self {
            layout: l,
            layout_name: DEFAULT_LAYOUT_NAME,
            xmin,
            xmax,
            ymin,
            ymax,
            ux,
            uy,
            usable_x0,
            usable_x1,
            usable_y0,
            usable_y1,
            grid_x0,
            grid_x1,
            grid_y0,
            grid_y1,
            col_w,
            row_h,
            nl_x0,
            nl_x1,
            nl_y0,
            nl_y1,
            grid_cols,
            grid_rows,
            col_edges,
            row_edges,
            nl_cell: (0, grid_cols - 1),
            snap_outside: false,
        }
    }

    /// 指定布局名（仅用于输出与自检，几何参数仍以 `layout` 为准）。
    pub fn with_layout_name(mut self, name: &'static str) -> Self {
        self.layout_name = name;
        self
    }

    pub fn in_numlock(&self, x: i32, y: i32) -> bool {
        let (x, y) = (x as f64, y as f64);
        let (x0, x1) = (
            self.col_edges[self.nl_cell.1],
            self.col_edges[self.nl_cell.1 + 1],
        );
        let (y0, y1) = (
            self.row_edges[self.nl_cell.0],
            self.row_edges[self.nl_cell.0 + 1],
        );
        x >= x0 && x <= x1 && y >= y0 && y <= y1
    }

    /// 网格矩形 `(x0, y0, x1, y1)`。
    pub fn grid_rect(&self) -> (f64, f64, f64, f64) {
        (
            self.col_edges[0],
            self.row_edges[0],
            self.col_edges[self.grid_cols],
            self.row_edges[self.grid_rows],
        )
    }

    /// 单格矩形 `(x0, y0, x1, y1)`；越界返回 `None`。
    pub fn cell_rect(&self, row: usize, col: usize) -> Option<(f64, f64, f64, f64)> {
        if row >= self.grid_rows || col >= self.grid_cols {
            return None;
        }
        Some((
            self.col_edges[col],
            self.row_edges[row],
            self.col_edges[col + 1],
            self.row_edges[row + 1],
        ))
    }

    /// 坐标落在哪一列；`snap_outside` 时把网格外的坐标夹到最近一列。
    fn col_at(&self, x: f64) -> Option<usize> {
        let last = self.grid_cols.checked_sub(1)?;
        let inside = x >= self.col_edges[0] && x <= self.col_edges[self.grid_cols];
        if !inside && !self.snap_outside {
            return None;
        }
        if x <= self.col_edges[0] {
            return Some(0);
        }
        if x >= self.col_edges[self.grid_cols] {
            return Some(last);
        }
        let col = self
            .col_edges
            .windows(2)
            .position(|w| x >= w[0] && x < w[1])
            .unwrap_or(last);
        Some(col.min(last))
    }

    fn row_at(&self, y: f64) -> Option<usize> {
        let last = self.grid_rows.checked_sub(1)?;
        let inside = y >= self.row_edges[0] && y <= self.row_edges[self.grid_rows];
        if !inside && !self.snap_outside {
            return None;
        }
        if y <= self.row_edges[0] {
            return Some(0);
        }
        if y >= self.row_edges[self.grid_rows] {
            return Some(last);
        }
        let row = self
            .row_edges
            .windows(2)
            .position(|w| y >= w[0] && y < w[1])
            .unwrap_or(last);
        Some(row.min(last))
    }

    /// 返回 (row, col)；不在网格内返回 None（`snap_outside` 时夹到最近一格）。
    pub fn cell_of(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        let (xf, yf) = (x as f64, y as f64);
        let col = self.col_at(xf)?;
        let row = self.row_at(yf)?;
        Some((row, col))
    }

    pub fn cell_center(&self, row: usize, col: usize) -> (f64, f64) {
        let (x0, y0, x1, y1) = match self.cell_rect(row, col) {
            Some(r) => r,
            None => return (f64::NAN, f64::NAN),
        };
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
    }

    /// 格内命中判定（官方 GESTURE_06 `iTAPPING_THRESHOLD_MM = 10 mm` 单键容差）。
    ///
    /// 只裁掉格内四角斜角死区，**绝不**向网格外扩：网格之外属于触摸板光标区，
    /// 一旦外扩吸附，空白处会凭空出键——那是错误逻辑，不是官方行为。
    pub fn hit_cell(&self, x: i32, y: i32, tol_ux: f64, tol_uy: f64) -> Option<(usize, usize)> {
        let cell = self.cell_of(x, y)?;
        if tol_ux > 0.0 || tol_uy > 0.0 {
            let (row, col) = cell;
            let (cx, cy) = self.cell_center(row, col);
            let dx = if tol_ux > 0.0 {
                (x as f64 - cx) / tol_ux
            } else {
                0.0
            };
            let dy = if tol_uy > 0.0 {
                (y as f64 - cy) / tol_uy
            } else {
                0.0
            };
            if (dx * dx + dy * dy).sqrt() > 1.0 {
                return None;
            }
        }
        Some(cell)
    }

    /// 坐标 → 键码（内置键表）。
    pub fn key_at(&self, x: i32, y: i32, mode: KeyMode, tol: Tol) -> Option<u16> {
        self.key_at_with(crate::custom::builtin_table(), x, y, mode, tol)
    }

    /// 坐标 → 键码，键表由调用方给（自定义布局走这一条）。
    ///
    /// NumLock 图标区统一不出键；网格之外的坐标属于触摸板光标区，不吸附。
    /// 第 0 行最后一格在内置键表里本来就是空位，因此不再额外特判。
    pub fn key_at_with(
        &self,
        table: &crate::custom::KeyTable,
        x: i32,
        y: i32,
        mode: KeyMode,
        tol: Tol,
    ) -> Option<u16> {
        if self.in_numlock(x, y) {
            return None;
        }
        let cell = if tol.mm > 0.0 {
            self.hit_cell(x, y, tol.ux, tol.uy)?
        } else {
            self.cell_of(x, y)?
        };
        let (row, col) = cell;
        table.key_at(row, col, mode)
    }

    /// 计算器图标区（左上 1 列 x 1 行）。官方只给联动开关、未给触发区参数。
    pub fn in_calc_zone(&self, x: i32, y: i32) -> bool {
        let Some((x0, y0, x1, y1)) = self.cell_rect(0, 0) else {
            return false;
        };
        let (xf, yf) = (x as f64, y as f64);
        xf >= x0 && xf <= x1 && yf >= y0 && yf <= y1
    }

    /// 用自定义分区替换网格划分（表外机型用）。
    ///
    /// `x_edges` / `y_edges` 是触摸板绝对坐标下的升序边界，长度分别为「列数 + 1」与
    /// 「行数 + 1」；`nl_cell` 为 NumLock 图标所在格。
    ///
    /// 校验只看「能不能算出一张站得住的网格」：边界升序、至少一格、落在触摸板范围内、
    /// 每格宽高非零。宽高是否像一格按键（毫米级）留给上层提示，不在这里拦——贴纸
    /// 方案的格子尺寸由用户自己的贴纸决定。
    pub fn with_partition(
        mut self,
        x_edges: Vec<f64>,
        y_edges: Vec<f64>,
        nl_cell: Option<(usize, usize)>,
    ) -> Result<Self, String> {
        let cols = x_edges.len().saturating_sub(1);
        let rows = y_edges.len().saturating_sub(1);
        if cols == 0 || rows == 0 {
            return Err("partition: need at least 1 column and 1 row".to_string());
        }
        let mono = |v: &[f64]| v.windows(2).all(|w| w[1] > w[0]);
        if !mono(&x_edges) {
            return Err("partition: x edges must be strictly ascending".to_string());
        }
        if !mono(&y_edges) {
            return Err("partition: y edges must be strictly ascending".to_string());
        }
        let (xmin, xmax) = (self.xmin as f64, self.xmax as f64);
        let (ymin, ymax) = (self.ymin as f64, self.ymax as f64);
        if x_edges[0] < xmin || x_edges[cols] > xmax {
            return Err(format!(
                "partition: x edges out of panel range {xmin:.0}..{xmax:.0}"
            ));
        }
        if y_edges[0] < ymin || y_edges[rows] > ymax {
            return Err(format!(
                "partition: y edges out of panel range {ymin:.0}..{ymax:.0}"
            ));
        }
        let nl = nl_cell.unwrap_or((0, cols - 1));
        if nl.0 >= rows || nl.1 >= cols {
            return Err(format!(
                "partition: numlock cell ({}, {}) outside {rows}x{cols} grid",
                nl.0, nl.1
            ));
        }
        self.grid_cols = cols;
        self.grid_rows = rows;
        self.col_w = (x_edges[cols] - x_edges[0]) / cols as f64;
        self.row_h = (y_edges[rows] - y_edges[0]) / rows as f64;
        self.grid_x0 = x_edges[0];
        self.grid_x1 = x_edges[cols];
        self.grid_y0 = y_edges[0];
        self.grid_y1 = y_edges[rows];
        self.col_edges = x_edges;
        self.row_edges = y_edges;
        self.nl_cell = nl;
        // NumLock 兼容字段跟随新划分
        let (nx0, ny0, nx1, ny1) = self.cell_rect(nl.0, nl.1).expect("nl cell validated");
        self.nl_x0 = nx0;
        self.nl_x1 = nx1;
        self.nl_y0 = ny0;
        self.nl_y1 = ny1;
        Ok(self)
    }

    /// 网格外落点吸附到最近一格（默认关闭）。
    pub fn with_snap_outside(mut self, on: bool) -> Self {
        self.snap_outside = on;
        self
    }

    pub fn describe(&self) -> String {
        format!(
            "layout {} | grid x[{:.0},{:.0}] y[{:.0},{:.0}] cell {:.1}x{:.1} | numlock x[{:.0},{:.0}] y[{:.0},{:.0}]",
            self.layout_name,
            self.grid_x0,
            self.grid_x1,
            self.grid_y0,
            self.grid_y1,
            self.col_w,
            self.row_h,
            self.nl_x0,
            self.nl_x1,
            self.nl_y0,
            self.nl_y1
        )
    }
}

/// 布局支持级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutSupport {
    /// 与 GH3XLNDZ 相同的网格结构（5 列 4 行、15 个官方参数齐全）。
    Verified,
    /// 5 列 4 行同族布局：官方 inf 给出的 `NUMPADLAYOUT_<NAME>_*` 项全部照录（官方没列的项按 0，
    /// 与 Windows 驱动读不到注册表值时的缺省一致），列数与 GH3XLNDZ 相同；键位排布未在真机上
    /// 逐键核对，所以仍需显式放行。
    Family,
    /// 官方 inf 收录了这套参数，但该键表从未在真机上核对过，也没有键位排布依据，直接套用可能
    /// 按错键——启动时不放行。
    Unsupported,
}

/// 一条布局记录：官方名字 + 15 个参数 + 支持级别。
#[derive(Debug, Clone, Copy)]
pub struct LayoutEntry {
    pub name: &'static str,
    pub layout: Layout,
    pub support: LayoutSupport,
}

/// 官方 AsusNUMPADFilter.inf（v17.0.0.27）里 `NUMPADLAYOUT_<NAME>_00..14` 的全部取值。
///
/// 单位一律 0.1 mm；官方并非每套布局都给满 15 项：`GVM` 只到 `_06`、`NVM` 只到 `_12`，
/// `NH`/`CH` 系列多为 13 项（缺 `_13`/`_14` 的 NumLock InValidEdge）。官方没列的项按 0 填，
/// 与 Windows 驱动读不到该注册表值时的缺省一致，不做数值推测。
pub const LAYOUTS: &[LayoutEntry] = &[
    LayoutEntry {
        name: "CH3XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 22,
            index4: 22,
            level0: 16,
            level3: 18,
            numlock_index: 10,
            numlock_level: 12,
            left: 7,
            right: 8,
            up: 10,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "CHXLNDZ",
        layout: Layout {
            columns: 5,
            index0: 22,
            index4: 22,
            level0: 12,
            level3: 15,
            numlock_index: 8,
            numlock_level: 12,
            left: 8,
            right: 8,
            up: 12,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "GH2XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 22,
            index4: 22,
            level0: 16,
            level3: 20,
            numlock_index: 30,
            numlock_level: 16,
            left: 10,
            right: 10,
            up: 6,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 10,
            invalid_level: 6,
        },
        support: LayoutSupport::Family,
    },
    LayoutEntry {
        name: "GH3XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 17,
            index4: 20,
            level0: 15,
            level3: 20,
            numlock_index: 31,
            numlock_level: 18,
            left: 13,
            right: 11,
            up: 9,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 11,
            invalid_level: 9,
        },
        support: LayoutSupport::Verified,
    },
    LayoutEntry {
        name: "GH4XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 27,
            index4: 22,
            level0: 15,
            level3: 15,
            numlock_index: 22,
            numlock_level: 12,
            left: 6,
            right: 5,
            up: 18,
            down: 8,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 4,
            invalid_level: 2,
        },
        support: LayoutSupport::Family,
    },
    LayoutEntry {
        name: "GHM",
        layout: Layout {
            columns: 5,
            index0: 20,
            index4: 20,
            level0: 13,
            level3: 13,
            numlock_index: 20,
            numlock_level: 13,
            left: 0,
            right: 0,
            up: 0,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "GVL",
        layout: Layout {
            columns: 5,
            index0: 13,
            index4: 13,
            level0: 17,
            level3: 16,
            numlock_index: 13,
            numlock_level: 17,
            left: 2,
            right: 2,
            up: 7,
            down: 6,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 2,
            invalid_level: 7,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "GVM",
        layout: GVM,
        support: LayoutSupport::Family,
    },
    LayoutEntry {
        name: "NH2XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 21,
            index4: 21,
            level0: 15,
            level3: 17,
            numlock_index: 10,
            numlock_level: 12,
            left: 10,
            right: 10,
            up: 9,
            down: 0,
            numadv_index: 10,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NH2XLNDZ9LED",
        layout: Layout {
            columns: 5,
            index0: 20,
            index4: 20,
            level0: 14,
            level3: 16,
            numlock_index: 8,
            numlock_level: 10,
            left: 7,
            right: 7,
            up: 10,
            down: 0,
            numadv_index: 8,
            numadv_level: 10,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NH2XLOL",
        layout: Layout {
            columns: 5,
            index0: 21,
            index4: 23,
            level0: 14,
            level3: 16,
            numlock_index: 10,
            numlock_level: 12,
            left: 7,
            right: 7,
            up: 10,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NH3XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 20,
            index4: 21,
            level0: 16,
            level3: 16,
            numlock_index: 10,
            numlock_level: 12,
            left: 9,
            right: 9,
            up: 10,
            down: 0,
            numadv_index: 10,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NH4XLNDZ",
        layout: Layout {
            columns: 5,
            index0: 24,
            index4: 24,
            level0: 18,
            level3: 21,
            numlock_index: 10,
            numlock_level: 12,
            left: 12,
            right: 12,
            up: 12,
            down: 0,
            numadv_index: 10,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHL",
        layout: Layout {
            columns: 5,
            index0: 21,
            index4: 21,
            level0: 20,
            level3: 16,
            numlock_index: 10,
            numlock_level: 8,
            left: 0,
            right: 0,
            up: 0,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHLNDZ",
        layout: Layout {
            columns: 5,
            index0: 17,
            index4: 17,
            level0: 13,
            level3: 17,
            numlock_index: 8,
            numlock_level: 12,
            left: 5,
            right: 5,
            up: 10,
            down: 0,
            numadv_index: 8,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHLOL",
        layout: Layout {
            columns: 5,
            index0: 18,
            index4: 18,
            level0: 14,
            level3: 16,
            numlock_index: 10,
            numlock_level: 12,
            left: 3,
            right: 3,
            up: 10,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHM",
        layout: Layout {
            columns: 5,
            index0: 21,
            index4: 21,
            level0: 13,
            level3: 13,
            numlock_index: 10,
            numlock_level: 7,
            left: 0,
            right: 0,
            up: 0,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHMNDZ",
        layout: Layout {
            columns: 5,
            index0: 17,
            index4: 19,
            level0: 11,
            level3: 13,
            numlock_index: 8,
            numlock_level: 12,
            left: 3,
            right: 3,
            up: 8,
            down: 0,
            numadv_index: 8,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHMOL",
        layout: Layout {
            columns: 5,
            index0: 18,
            index4: 18,
            level0: 12,
            level3: 13,
            numlock_index: 10,
            numlock_level: 12,
            left: 3,
            right: 3,
            up: 7,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHMOLFP",
        layout: Layout {
            columns: 5,
            index0: 18,
            index4: 18,
            level0: 12,
            level3: 10,
            numlock_index: 10,
            numlock_level: 13,
            left: 3,
            right: 3,
            up: 13,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHS",
        layout: Layout {
            columns: 5,
            index0: 20,
            index4: 21,
            level0: 11,
            level3: 15,
            numlock_index: 10,
            numlock_level: 7,
            left: 0,
            right: 0,
            up: 0,
            down: 0,
            numadv_index: 0,
            numadv_level: 0,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHXL",
        layout: Layout {
            columns: 5,
            index0: 21,
            index4: 21,
            level0: 13,
            level3: 16,
            numlock_index: 10,
            numlock_level: 12,
            left: 8,
            right: 8,
            up: 4,
            down: 0,
            numadv_index: 10,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    LayoutEntry {
        name: "NHXLNDZ",
        layout: Layout {
            columns: 5,
            index0: 22,
            index4: 22,
            level0: 13,
            level3: 14,
            numlock_index: 8,
            numlock_level: 12,
            left: 6,
            right: 6,
            up: 8,
            down: 0,
            numadv_index: 8,
            numadv_level: 12,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Unsupported,
    },
    // 官方 inf 对 `NUMPADLAYOUT_NVM_*` 只列到 `_12`（`_13` / `_14` 的 NumLock InValidEdge
    // 没有给出，按缺省 0 填）。列数同为 5，网格结构与 GH3XLNDZ 一致，所以归同族；
    // 灵耀双屏的触摸板竖排在键盘右侧、数字模式由键盘上的专用键切换，尚未上真机逐键核对，
    // 因此是 `Family` 而不是 `Verified`——要跑起来仍需显式放行。
    LayoutEntry {
        name: "NVM",
        layout: Layout {
            columns: 5,
            index0: 12,
            index4: 12,
            level0: 13,
            level3: 17,
            numlock_index: 7,
            numlock_level: 7,
            left: 3,
            right: 4,
            up: 6,
            down: 0,
            numadv_index: 7,
            numadv_level: 7,
            invalid_index: 0,
            invalid_level: 0,
        },
        support: LayoutSupport::Family,
    },
    // `GVM_A` 是 `GVM` 的变体名，官方 inf 里没有独立的 `NUMPADLAYOUT_GVM_A_*`：
    // `HID\ASUE1408&Col02` 注为 `GVM_A'`，而 `NUMPADLAYOUT_GVM_05` / `_06` 的注释直接写
    // 「0.1mm offset (Index/Level NumLockEdge) for GVMA」——即 GVMA 用的就是 GVM 那组参数，
    // 这两个参数本来就是给它加的。所以这里指向同一份参数，不再借 GH3XLNDZ 占位。
    LayoutEntry {
        name: "GVM_A",
        layout: GVM,
        support: LayoutSupport::Family,
    },
    LayoutEntry {
        name: "UNKNOWN",
        layout: GH3XLNDZ,
        support: LayoutSupport::Unsupported,
    },
];

/// 官方 INF 的 HID 硬件 ID → 布局名映射（`HID\\ASUE1416&Col02` 这类条目）。
///
/// 这是判断机型该用哪套布局的**唯一可靠依据**：布局由触摸板模组（ASUE/ASUF/ASCP/ASUP/ELAN
/// 前缀 + 4 位序号）决定，与整机型号无关，所以同模组的机型天然通用。
pub const HID_LAYOUTS: &[(&str, &str)] = &[
    ("ASCF1400", "GH4XLNDZ"),
    ("ASCP1400", "GH4XLNDZ"),
    ("ASUE1400", "NHL"),
    ("ASUE1401", "NHM"),
    ("ASUE1402", "NHS"),
    ("ASUE1403", "GH2XLNDZ"),
    ("ASUE1404", "GVM"),
    ("ASUE1405", "GHM"),
    ("ASUE1406", "NVM"),
    ("ASUE1407", "NHXL"),
    ("ASUE1408", "GVM_A"),
    ("ASUE1409", "NHMOL"),
    ("ASUE140A", "NHXLNDZ"),
    ("ASUE140B", "NHLNDZ"),
    ("ASUE140C", "NHMNDZ"),
    ("ASUE140D", "NH2XLNDZ"),
    ("ASUE140E", "NHXLNDZ"),
    ("ASUE140F", "GVL"),
    ("ASUE1410", "CHXLNDZ"),
    ("ASUE1411", "NHLOL"),
    ("ASUE1412", "NH3XLNDZ"),
    ("ASUE1413", "NH4XLNDZ"),
    ("ASUE1414", "NH2XLOL"),
    ("ASUE1415", "CH3XLNDZ"),
    ("ASUE1416", "GH3XLNDZ"),
    ("ASUE1417", "NH2XLNDZ9LED"),
    ("ASUE1418", "CH3XLNDZ"),
    ("ASUF1411", "NHLOL"),
    ("ASUF1415", "NH2XLNDZ9LED"),
    ("ASUF1416", "GH3XLNDZ"),
    ("ASUP1411", "NHLOL"),
    ("ASUP1412", "NHMOL"),
    ("ASUP1413", "NH4XLNDZ"),
    ("ASUP1414", "NH2XLOL"),
    ("ASUP1415", "NH2XLNDZ9LED"),
    ("ASUP1416", "NH2XLNDZ"),
    ("ELAN1400", "NHL"),
    ("ELAN1401", "NHM"),
    ("ELAN1402", "NHS"),
    // ELAN1403 与 ASUE1403 是同一款模组的两种命名（官方 SupportList 只列了 ASUE1403，
    // 其余 1400–1407 都成对出现 Elan / ASUE 两种写法，这里按同一模组处理），
    // 归入 GH2XLNDZ：结构同族，按 family 放行需人工逐键核对。
    ("ELAN1403", "GH2XLNDZ"),
    ("ELAN1404", "GVM"),
    ("ELAN1405", "GHM"),
    ("ELAN1406", "NVM"),
    ("ELAN1407", "NHXL"),
];

/// 默认布局名（键表基准 GH3XLNDZ）。
pub const DEFAULT_LAYOUT_NAME: &str = "GH3XLNDZ";

impl LayoutSupport {
    pub fn as_str(self) -> &'static str {
        match self {
            LayoutSupport::Verified => "verified",
            LayoutSupport::Family => "family",
            LayoutSupport::Unsupported => "unsupported",
        }
    }
    /// 只有 `Verified` 可以不经确认直接跑；其余必须在配置里显式放行。
    pub fn needs_confirm(self) -> bool {
        !matches!(self, LayoutSupport::Verified)
    }
}

/// 按布局名查表（大小写不敏感）：先查配置里的自定义布局，再查官方表。
pub fn by_name(name: &str) -> Option<&'static LayoutEntry> {
    let want = name.trim();
    if want.is_empty() {
        return None;
    }
    if let Some(entry) = crate::custom::entry_by_name(want) {
        return Some(entry);
    }
    builtin_by_name(want)
}

/// 只查官方布局表（注册自定义布局时用来判重名）。
pub fn builtin_by_name(name: &str) -> Option<&'static LayoutEntry> {
    let want = name.trim();
    if want.is_empty() {
        return None;
    }
    LAYOUTS.iter().find(|e| e.name.eq_ignore_ascii_case(want))
}

/// 反查布局名：优先命中表内完全相同的参数，否则返回默认名。
pub fn name_of(layout: &Layout) -> &'static str {
    LAYOUTS
        .iter()
        .find(|e| {
            e.layout.columns == layout.columns
                && e.layout.index0 == layout.index0
                && e.layout.index4 == layout.index4
                && e.layout.level0 == layout.level0
                && e.layout.level3 == layout.level3
                && e.layout.numlock_index == layout.numlock_index
                && e.layout.numlock_level == layout.numlock_level
                && e.layout.left == layout.left
                && e.layout.right == layout.right
                && e.layout.up == layout.up
                && e.layout.down == layout.down
                && e.layout.invalid_index == layout.invalid_index
                && e.layout.invalid_level == layout.invalid_level
        })
        .map(|e| e.name)
        .unwrap_or(DEFAULT_LAYOUT_NAME)
}

/// 从设备名里提取官方 HID 硬件 ID，例如
/// `ASUE1416:00 04F3:3275 Touchpad` → `ASUE1416`。
///
/// 布局由触摸板模组 ID 决定（`ASUE/ASUF/ASCP/ASUP/ELAN` + 4 位序号），
/// 与整机型号无关，这是跨机型通用的关键。
pub fn hid_id_of(device_name: &str) -> Option<String> {
    let up = device_name.to_uppercase();
    let bytes = up.as_bytes();
    for prefix in ["ASUE", "ASUF", "ASCP", "ASUP", "ASUX", "ELAN"] {
        let mut from = 0;
        while let Some(pos) = up[from..].find(prefix) {
            let start = from + pos;
            let end = start + prefix.len() + 4;
            if end <= bytes.len() {
                let digits = &up[start + prefix.len()..end];
                if digits.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(up[start..end].to_string());
                }
            }
            from = start + prefix.len();
        }
    }
    None
}

/// HID 硬件 ID → 布局记录：配置里显式绑定的自定义布局优先于官方映射。
pub fn for_hid_id(hid: &str) -> Option<&'static LayoutEntry> {
    let key = hid.trim().to_uppercase();
    if let Some(entry) = crate::custom::entry_by_hid(&key) {
        return Some(entry);
    }
    builtin_for_hid_id(&key)
}

/// 只查官方 `HID -> 布局` 映射表。
pub fn builtin_for_hid_id(hid: &str) -> Option<&'static LayoutEntry> {
    let key = hid.trim().to_uppercase();
    HID_LAYOUTS
        .iter()
        .find(|(k, _)| *k == key)
        .and_then(|(_, name)| builtin_by_name(name))
}

/// 设备名 → 布局记录（先取 HID 硬件 ID，再查表）。
pub fn for_device_name(device_name: &str) -> Option<&'static LayoutEntry> {
    let hid = hid_id_of(device_name)?;
    for_hid_id(&hid)
}

/// 与 [`for_device_name`] 同源，但只查官方表，不认配置里的自定义布局。
///
/// 用于「内置表应该是什么」这类断言和判重名——注册了自定义布局的进程里，
/// 这两件事都必须看官方原表，不能被自定义项顶掉。
pub fn builtin_for_device_name(device_name: &str) -> Option<&'static LayoutEntry> {
    let hid = hid_id_of(device_name)?;
    builtin_for_hid_id(&hid)
}

/// 全部受支持 HID 硬件 ID（`layout` 子命令展示对应机型布局用）。
pub fn hid_list() -> &'static [(&'static str, &'static str)] {
    HID_LAYOUTS
}

#[cfg(test)]
mod tests {
    use super::*;

    /// G634 家族的触摸板模组 ID 与其它机型必须各自解析到正确的布局。
    #[test]
    fn hid_ids_map_to_expected_layouts() {
        // 本机（SCAR 16 G634JY）：ASUE1416 -> GH3XLNDZ
        assert_eq!(
            hid_id_of("ASUE1416:00 04F3:3275 Touchpad").as_deref(),
            Some("ASUE1416")
        );
        assert_eq!(
            builtin_for_device_name("ASUE1416:00 04F3:3275 Touchpad").map(|e| e.name),
            Some("GH3XLNDZ")
        );
        // 同族不同机型
        assert_eq!(
            builtin_for_device_name("ASUE1403:00 04F3:3288 Touchpad").map(|e| e.name),
            Some("GH2XLNDZ")
        );
        assert_eq!(
            builtin_for_device_name("ASCP1400:00 28A9:0100 Touchpad").map(|e| e.name),
            Some("GH4XLNDZ")
        );
        // 老式 NH 系列
        assert_eq!(
            builtin_for_device_name("ASUE1400:00 04F3:3083 Touchpad").map(|e| e.name),
            Some("NHL")
        );
        // 无线接收器 / 键盘不应被当成触摸板，也解析不出 HID
        assert_eq!(hid_id_of("ASUS ROG WIRELESS RECEIVER 0B05:1ACE"), None);
        assert_eq!(hid_id_of("ASUSTeK Computer Inc. N-KEY Device"), None);
    }

    /// 只有 GH3XLNDZ 是已验证；同族 GH2/GH4 需确认；其余不可套用。
    #[test]
    fn support_levels_are_conservative() {
        assert_eq!(
            builtin_by_name("GH3XLNDZ").unwrap().support,
            LayoutSupport::Verified
        );
        assert_eq!(
            builtin_by_name("GH2XLNDZ").unwrap().support,
            LayoutSupport::Family
        );
        assert_eq!(
            builtin_by_name("GH4XLNDZ").unwrap().support,
            LayoutSupport::Family
        );
        assert_eq!(
            builtin_by_name("NHL").unwrap().support,
            LayoutSupport::Unsupported
        );
        // 双屏机的三张键表：官方参数已照录、列数同为 5，归同族；但没上过真机，仍需显式放行
        for name in ["GVM", "GVM_A", "NVM"] {
            let e = builtin_by_name(name).expect("双屏键表必须在表内");
            assert_eq!(e.support, LayoutSupport::Family, "{name} 应为同族");
            assert_eq!(e.layout.columns, 5, "{name} 应为 5 列");
            assert!(e.support.needs_confirm(), "{name} 不能免确认直接跑");
        }
        // GVM_A 用 GVM 那组参数（NUMPADLAYOUT_GVM_05/_06 注为 for GVMA），不再借 GH3XLNDZ 占位
        assert_eq!(builtin_by_name("GVM_A").unwrap().layout, GVM);
        assert_ne!(builtin_by_name("GVM_A").unwrap().layout, GH3XLNDZ);
        assert_eq!(
            builtin_for_hid_id("ASUE1408").map(|e| e.name),
            Some("GVM_A")
        );
        assert_eq!(builtin_for_hid_id("ASUE1404").map(|e| e.name), Some("GVM"));
        assert_eq!(builtin_for_hid_id("ASUE1406").map(|e| e.name), Some("NVM"));
        // G634（SCAR 16/18 2023）用的两个模组 ID 都必须解析到已验证的 GH3XLNDZ
        for hid in ["ASUE1416", "ASUF1416"] {
            let e = builtin_for_hid_id(hid).expect("G634 模组 ID 必须在表内");
            assert_eq!(e.name, "GH3XLNDZ", "{hid} 应解析为 GH3XLNDZ");
            assert_eq!(e.support, LayoutSupport::Verified);
        }
        // 非 G634 的模组（CH3XLNDZ）保持不可用，不能被误当成 G634
        for hid in ["ASUE1415", "ASUE1418"] {
            assert_eq!(builtin_for_hid_id(hid).map(|e| e.name), Some("CH3XLNDZ"));
            assert_eq!(
                builtin_for_hid_id(hid).unwrap().support,
                LayoutSupport::Unsupported
            );
        }
    }

    /// 每个 HID 映射都必须能查到布局记录，不允许出现空指针式回落。
    #[test]
    fn every_hid_entry_resolves() {
        for (hid, name) in HID_LAYOUTS {
            assert!(
                builtin_by_name(name).is_some(),
                "{hid} -> {name} 未在 LAYOUTS 中定义"
            );
        }
    }

    /// 每套布局都要能换算出一张站得住的网格：列宽行高为正、不越出有效区、
    /// NumLock 区落在网格内、格子在物理上不至于小到点不中。
    ///
    /// 机型参数按官方 INF 逐机型照录，改一个数就可能让网格翻出触摸板；
    /// 这条断言把「照录」的结果关住，避免某套布局悄悄变成空白键位。
    #[test]
    fn every_layout_keeps_a_sane_grid() {
        // 典型轴参数（ASUE1416：4752 x 3008 units，约 45 / 42 units per mm）
        let ux = 45.0 / 1000.0;
        let uy = 42.0 / 1000.0;
        for entry in LAYOUTS {
            let g =
                Geometry::new(0, 4752, 0, 3008, ux, uy, entry.layout).with_layout_name(entry.name);
            let name = entry.name;
            assert_eq!(g.layout_name, name, "布局名未写入几何");
            assert_eq!(
                entry.layout.columns as usize, COLS,
                "{name} 列数与键表常量 COLS 不一致"
            );
            assert!(g.col_w > 0.0, "{name} 列宽非正：{}", g.col_w);
            assert!(g.row_h > 0.0, "{name} 行高非正：{}", g.row_h);
            assert!(
                g.grid_x0 >= g.usable_x0 && g.grid_x1 <= g.usable_x1,
                "{name} 网格横向越出有效区"
            );
            assert!(
                g.grid_y0 >= g.usable_y0 && g.grid_y1 <= g.usable_y1,
                "{name} 网格纵向越出有效区"
            );
            assert!(
                g.nl_x0 >= g.grid_x0 && g.nl_x1 <= g.grid_x1,
                "{name} NumLock 区不在网格内"
            );
            // units → µm → mm
            let col_mm = g.col_w / ux / 1000.0;
            let row_mm = g.row_h / uy / 1000.0;
            assert!(
                (5.0..=30.0).contains(&col_mm),
                "{name} 列宽 {col_mm:.1} mm 不像一格按键"
            );
            assert!(
                (5.0..=30.0).contains(&row_mm),
                "{name} 行高 {row_mm:.1} mm 不像一格按键"
            );
        }
    }

    /// 键表固定 5 列：列数更多的布局在多出来的一列上不出键，也不越界。
    #[test]
    fn wider_layout_than_key_grid_yields_no_key() {
        let ux = 45.0 / 1000.0;
        let uy = 42.0 / 1000.0;
        let mut l = builtin_by_name("GH3XLNDZ").unwrap().layout;
        l.columns = COLS as i32 + 1;
        let g = Geometry::new(0, 4752, 0, 3008, ux, uy, l);
        // 第 2 行最后一列（键表里不存在）
        let (row, col) = (2usize, COLS);
        let (cx, cy) = g.cell_center(row, col);
        assert_eq!(
            g.key_at(cx as i32, cy as i32, KeyMode::Keypad, Tol::exact()),
            None
        );
    }

    /// 15 参数的 GH2/GH3/GH4 三套布局必须结构一致（都是 5 列 5x4 键表）。
    #[test]
    fn gh_family_shares_structure() {
        let g3 = builtin_by_name("GH3XLNDZ").unwrap().layout;
        for name in ["GH2XLNDZ", "GH4XLNDZ"] {
            let l = builtin_by_name(name).unwrap().layout;
            assert_eq!(l.columns, g3.columns, "{name} 列数不同");
            // 同族布局的键表几何必须落在同一量级（0.1mm 单位，误差不超过 1 格）
            assert!((l.index0 - g3.index0).abs() <= 10, "{name} index0 差异过大");
            assert!((l.index4 - g3.index4).abs() <= 10, "{name} index4 差异过大");
        }
    }

    /// 官方驱动 SupportList 里的 44 个模组名必须全部收录。
    ///
    /// 证据：AsusNUMPADFilter.inf（17.0.0.27 / 29，X 系与 J 系共三个 exe，
    /// 解包后 487 条注册表参数逐项一致）中 `Parameters\SupportList` 的 PTP_0..45。
    /// 少一个就意味着某款 ASUS 机型装上来会落到「未知模组」分支。
    #[test]
    fn covers_official_supportlist() {
        let mut official: Vec<String> = (0x00..=0x18).map(|i| format!("ASUE14{i:02X}")).collect();
        official.extend(["ASUF1411", "ASUF1415", "ASUF1416"].map(String::from));
        official.extend(
            [
                "ASUP1411", "ASUP1412", "ASUP1413", "ASUP1414", "ASUP1415", "ASUP1416",
            ]
            .map(String::from),
        );
        official.extend(["ASCF1400", "ASCP1400"].map(String::from));
        official.extend((0x00..=0x07).map(|i| format!("ELAN14{i:02X}")));

        let missing: Vec<&String> = official
            .iter()
            .filter(|hid| builtin_for_hid_id(hid).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "以下官方 SupportList 模组未收录：{missing:?}"
        );
        assert_eq!(official.len(), 44, "官方 SupportList 模组数应为 44");
    }
}
