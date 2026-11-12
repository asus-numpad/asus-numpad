//! 自定义布局：几何增量 + 键表 + 可选的 HID 模组绑定，装进进程级注册表。

use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::layout::{self, Layout, LayoutEntry, LayoutSupport, ROWS};

use super::*;

/// 15 个官方布局参数的可选覆盖（未写的项继承 `base`）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutPatch {
    pub columns: Option<i32>,
    pub index0: Option<i32>,
    pub index4: Option<i32>,
    pub level0: Option<i32>,
    pub level3: Option<i32>,
    pub numlock_index: Option<i32>,
    pub numlock_level: Option<i32>,
    pub left: Option<i32>,
    pub right: Option<i32>,
    pub up: Option<i32>,
    pub down: Option<i32>,
    pub numadv_index: Option<i32>,
    pub numadv_level: Option<i32>,
    pub invalid_index: Option<i32>,
    pub invalid_level: Option<i32>,
}

impl LayoutPatch {
    /// 把已写的项覆盖到 `base` 上。
    pub fn apply(&self, base: Layout) -> Layout {
        let mut l = base;
        macro_rules! take {
            ($($f:ident),*) => {$( if let Some(v) = self.$f { l.$f = v; } )*};
        }
        take!(
            columns,
            index0,
            index4,
            level0,
            level3,
            numlock_index,
            numlock_level,
            left,
            right,
            up,
            down,
            numadv_index,
            numadv_level,
            invalid_index,
            invalid_level
        );
        l
    }
}

/// 配置里的一份自定义布局。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct CustomLayoutSpec {
    /// 布局名（写进 `layout_name` 就能选中它）。
    pub name: String,
    /// 参数基准：内置布局名（如 `GH3XLNDZ`）或上一条自定义布局。
    pub base: String,
    /// 绑定的触摸板 HID 模组 ID（如 `ASUE1416`）；`auto` 时命中即自动选用。
    pub hid_ids: Vec<String>,
    /// 4 行键表；留空 = 沿用基准布局的键表。
    pub keys: Vec<String>,
    #[serde(flatten)]
    pub patch: LayoutPatch,
}

impl Default for CustomLayoutSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            base: layout::DEFAULT_LAYOUT_NAME.to_string(),
            hid_ids: Vec::new(),
            keys: Vec::new(),
            patch: LayoutPatch::default(),
        }
    }
}

/// 键表是否「留空 = 沿用基准布局的键表」。
///
/// 配置里 `keys = []`（或全是空白行）时不报错：贴纸/镭雕只改了行列几何、键位照抄
/// 基准机型的场景很常见，逼用户把整张表抄一遍既啰嗦又容易抄错。
pub fn keys_inherit_base(keys: &[String]) -> bool {
    keys.iter().all(|line| line.trim().is_empty())
}

/// 已注册的自定义布局：Leaked 成 `'static`，好让 `Geometry` / `LayoutEntry` 的
/// 现有 `&'static str` 接口原样复用。
pub struct CustomLayout {
    pub entry: &'static LayoutEntry,
    pub table: KeyTable,
    pub hid_ids: Vec<String>,
}

fn registry() -> &'static RwLock<Vec<CustomLayout>> {
    static REG: OnceLock<RwLock<Vec<CustomLayout>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(Vec::new()))
}

/// 注册全部自定义布局（进程启动时调一次）；返回给用户看的提示。
///
/// 两阶段：先在**不持锁**的状态下把每个 spec 解析成 [`CustomLayout`]（`base` 既可
/// 指向内置布局，也可指向同一份配置里排在前面的另一条自定义布局），最后一次性
/// 提交进全局表。先持锁再解析会踩到「写锁持有期间调用 `layout::by_name` 又要读锁」
/// 的重入死锁——`std::sync::RwLock` 不可重入。
pub fn install_layouts(specs: &[CustomLayoutSpec]) -> Vec<String> {
    // 先清空：每次安装都是「配置文件里写了什么就是什么」
    {
        let mut reg = match registry().write() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        reg.clear();
    }
    let mut notes = Vec::new();
    if specs.is_empty() {
        return notes;
    }
    let mut pending: Vec<CustomLayout> = Vec::new();
    for spec in specs {
        let name = spec.name.trim();
        if name.is_empty() {
            notes.push(crate::i18n::t("note.custom_layout_no_name"));
            continue;
        }
        if layout::builtin_by_name(name).is_some() {
            notes.push(crate::i18n::tf(
                "note.custom_layout_builtin_clash",
                &[("name", name)],
            ));
            continue;
        }
        let base = layout::builtin_by_name(&spec.base).or_else(|| {
            pending
                .iter()
                .find(|c| c.entry.name.eq_ignore_ascii_case(spec.base.trim()))
                .map(|c| c.entry)
        });
        let Some(base) = base else {
            notes.push(crate::i18n::tf(
                "note.custom_layout_base_missing",
                &[("name", name), ("base", spec.base.trim())],
            ));
            continue;
        };
        let mut patch = spec.patch;
        let inherited = keys_inherit_base(&spec.keys);
        let table = if inherited {
            // 沿用它继承的那张表：先看前面已解析的自定义布局，再看内置表
            match pending
                .iter()
                .find(|c| c.entry.name.eq_ignore_ascii_case(spec.base.trim()))
            {
                Some(base) => base.table.clone(),
                None => builtin_table().clone(),
            }
        } else {
            match KeyTable::parse(&spec.keys) {
                Ok(t) => {
                    // 列数以键表为准：贴纸方案天然由贴纸的列数决定
                    patch.columns = Some(t.cols() as i32);
                    t
                }
                Err(err) => {
                    notes.push(crate::i18n::tf(
                        "note.custom_layout_bad_keys",
                        &[("name", name), ("err", &err)],
                    ));
                    continue;
                }
            }
        };
        let layout = patch.apply(base.layout);
        if layout.columns < 1 || layout.columns > 8 {
            notes.push(crate::i18n::tf(
                "note.custom_layout_bad_columns",
                &[("name", name), ("n", &layout.columns.to_string())],
            ));
            continue;
        }
        let leaked_name: &'static str = Box::leak(name.to_string().into_boxed_str());
        let entry: &'static LayoutEntry = Box::leak(Box::new(LayoutEntry {
            name: leaked_name,
            layout,
            support: LayoutSupport::Verified,
        }));
        let hid_ids: Vec<String> = spec
            .hid_ids
            .iter()
            .map(|s| s.trim().to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        for hid in &hid_ids {
            if let Some(builtin) = layout::builtin_for_hid_id(hid) {
                notes.push(crate::i18n::tf(
                    "note.custom_hid_override",
                    &[("hid", hid), ("name", name), ("builtin", builtin.name)],
                ));
            }
        }
        pending.push(CustomLayout {
            entry,
            table,
            hid_ids,
        });
    }
    let mut reg = match registry().write() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    *reg = pending;
    notes
}

fn read_registry<T>(f: impl FnOnce(&[CustomLayout]) -> T) -> Option<T> {
    let reg = registry().read().ok()?;
    if reg.is_empty() {
        return None;
    }
    Some(f(&reg))
}

/// 按名字查自定义布局。
pub fn entry_by_name(name: &str) -> Option<&'static LayoutEntry> {
    read_registry(|reg| {
        reg.iter()
            .find(|c| c.entry.name.eq_ignore_ascii_case(name))
            .map(|c| c.entry)
    })?
}

/// 按触摸板 HID 模组 ID 查自定义布局。
pub fn entry_by_hid(hid: &str) -> Option<&'static LayoutEntry> {
    let wanted = hid.trim().to_ascii_uppercase();
    read_registry(|reg| {
        reg.iter()
            .find(|c| c.hid_ids.contains(&wanted))
            .map(|c| c.entry)
    })?
}

/// 当前配置下应该用哪张键表：命中自定义布局用它的，否则用内置表。
pub fn table_for(layout_name: &str) -> KeyTable {
    let found: Option<KeyTable> = read_registry(|reg| {
        reg.iter()
            .find(|c| c.entry.name.eq_ignore_ascii_case(layout_name))
            .map(|c| c.table.clone())
    })
    .flatten();
    if let Some(table) = found {
        return table.flatten();
    }
    builtin_table().clone()
}

impl KeyTable {
    /// 补齐为固定 4 行（`Geometry` 始终按 4 行切分网格）。
    fn flatten(mut self) -> Self {
        while self.rows.len() < ROWS {
            self.rows.push(vec![KeySlot::None; self.cols]);
        }
        self
    }
}

/// 已注册的自定义布局一览（CLI / GUI 展示用）：`(名字, 绑定 HID, 网格文本)`。
pub fn list_custom() -> Vec<(String, Vec<String>, Vec<String>)> {
    read_registry(|reg| {
        reg.iter()
            .map(|c| {
                (
                    c.entry.name.to_string(),
                    c.hid_ids.clone(),
                    c.table.describe(),
                )
            })
            .collect()
    })
    .unwrap_or_default()
}
