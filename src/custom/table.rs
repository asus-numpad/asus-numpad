//! 键表：一格放内置符号、固定键码还是空着。

use std::sync::OnceLock;

use crate::layout::{self, Cell, KeyMode, ROWS};

use super::*;

/// 一格里放什么：无键 / 内置符号（跟随 `key_mode` 切换主键区或小键盘）/ 固定键码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySlot {
    None,
    Builtin(Cell),
    Code(u16),
}

impl KeySlot {
    pub fn key(self, mode: KeyMode) -> Option<u16> {
        match self {
            KeySlot::None => None,
            KeySlot::Builtin(cell) => layout::key_for_cell(cell, mode),
            KeySlot::Code(code) => Some(code),
        }
    }

    /// 这一格的短名，供 `describe` 把键表打印成又一种可以再喂回 `parse` 的文本。
    ///
    /// 空位、小数点、减号在键盘上都只是一个符号（`.` `-`），表里却要区分三样东西：
    /// 空位不出键、小数点是 `period`、减号是 `minus`。所以这三格印英文名，其余照键盘
    /// 上印的写——`describe` 的结果因此总能原样解析回去，GUI 里改一格不会顺带改掉别人。
    pub fn label(self) -> String {
        match self {
            KeySlot::None => "none".to_string(),
            KeySlot::Builtin(Cell::Period) => "period".to_string(),
            KeySlot::Builtin(Cell::Minus) => "minus".to_string(),
            KeySlot::Builtin(cell) => layout::cell_label(cell).to_string(),
            KeySlot::Code(code) => key_label(code),
        }
    }
}

/// 4 行键表。列数由表长决定（1..=8），行数固定 [`ROWS`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTable {
    pub(crate) rows: Vec<Vec<KeySlot>>,
    pub(crate) cols: usize,
}

/// 内置键表（`KEY_GRID` 的运行时形态），进程内只构造一次。
pub fn builtin_table() -> &'static KeyTable {
    static BUILTIN: OnceLock<KeyTable> = OnceLock::new();
    BUILTIN.get_or_init(|| {
        let rows = layout::KEY_GRID
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| match cell {
                        Some(c) => KeySlot::Builtin(*c),
                        None => KeySlot::None,
                    })
                    .collect()
            })
            .collect();
        KeyTable {
            rows,
            cols: layout::COLS,
        }
    })
}

impl KeyTable {
    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> &[Vec<KeySlot>] {
        &self.rows
    }

    pub fn get(&self, row: usize, col: usize) -> Option<KeySlot> {
        self.rows.get(row)?.get(col).copied()
    }

    pub fn key_at(&self, row: usize, col: usize, mode: KeyMode) -> Option<u16> {
        self.get(row, col)?.key(mode)
    }

    /// 解析配置里的 4 行文本；`Err` 里带出行号，方便定位写错的那一行。
    ///
    /// 每格的写法（大小写不敏感）：
    /// * `.` / `-` / `none`：空位，不出键；
    /// * `7` `dot` `slash` `asterisk` `minus` `plus` `backspace` `enter`：
    ///   内置符号，跟随 `key_mode`（keypad / digits）自动切换；
    ///
    /// 符号键也可以直接照键盘上印的样子写：`+` `*` `/` 与 `plus` `asterisk` `slash`
    /// 等价。只有 `-` 例外——它早就被当成空位（与 [`Self::describe`] 打印空位的写法
    /// 对齐），减号得写 `minus`；小数点写 `period`（`dot` 是主键盘上那个固定键码）。
    /// * `kp7` `F13` `playpause` `volup` 等：固定键码，见 [`key_code`]；
    /// * `0x64`：直接写 Linux 键码（十进制或十六进制）。
    pub fn parse(rows: &[String]) -> Result<Self, String> {
        if rows.is_empty() || rows.len() > ROWS {
            return Err(format!("rows: need 1..={ROWS} lines, got {}", rows.len()));
        }
        let mut table: Vec<Vec<KeySlot>> = Vec::with_capacity(rows.len());
        let mut cols = 0usize;
        for (r, line) in rows.iter().enumerate() {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.is_empty() {
                return Err(format!("row {}: empty", r + 1));
            }
            if r == 0 {
                cols = tokens.len();
                if !(1..=8).contains(&cols) {
                    return Err(format!("row 1: {} columns, need 1..=8", cols));
                }
            } else if tokens.len() != cols {
                return Err(format!(
                    "row {}: {} columns, row 1 has {}",
                    r + 1,
                    tokens.len(),
                    cols
                ));
            }
            let mut row = Vec::with_capacity(cols);
            for (c, tok) in tokens.iter().enumerate() {
                row.push(
                    parse_slot(tok).map_err(|e| format!("row {}, col {}: {e}", r + 1, c + 1))?,
                );
            }
            table.push(row);
        }
        // 不足 4 行时补空行：Geometry 固定按 4 行切分网格
        while table.len() < ROWS {
            table.push(vec![KeySlot::None; cols]);
        }
        Ok(KeyTable { rows: table, cols })
    }

    /// 供 CLI / GUI 打印的网格文本（每格一个短名）。
    pub fn describe(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|row| row.iter().map(|s| s.label()).collect::<Vec<_>>().join(" "))
            .collect()
    }
}

fn parse_slot(tok: &str) -> Result<KeySlot, String> {
    let t = tok.trim();
    if t.is_empty() || t == "." || t == "-" || t.eq_ignore_ascii_case("none") {
        return Ok(KeySlot::None);
    }
    // 内置符号：与 key_mode 联动
    // 符号键两种写法都收：写 `plus` 或直接写键盘上那个 `+`，用户不该为了对齐去查文档。
    // 只把 `-` 留给空位（`describe` 打印空位也用 `.`），减号因此仍要写 `minus`。
    let builtin = match t.to_ascii_lowercase().as_str() {
        "period" => Some(Cell::Period),
        "backspace" | "bs" => Some(Cell::Backspace),
        "return" | "ret" | "enter" => Some(Cell::Return),
        "slash" | "div" | "/" => Some(Cell::Slash),
        "asterisk" | "star" | "mul" | "*" => Some(Cell::Asterisk),
        "minus" | "sub" => Some(Cell::Minus),
        "plus" | "add" | "+" => Some(Cell::Plus),
        _ => None,
    };
    if let Some(cell) = builtin {
        return Ok(KeySlot::Builtin(cell));
    }
    if let Some(d) = t.parse::<u8>().ok().filter(|d| *d <= 9) {
        return Ok(KeySlot::Builtin(Cell::Digit(d)));
    }
    if let Some(code) = key_code(t) {
        return Ok(KeySlot::Code(code));
    }
    Err(format!("unknown key `{t}`"))
}
