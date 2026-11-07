//! 机型配置：一个包，运行期自动选择本机对应的机型参数。
//!
//! ## 为什么不再按机型分包
//!
//! 早期做法是按机型出多个包（`asus-numpad-g614` 之类），每个包在
//! 编译期把机型写死，装错包就退出码 3 拒绝启动。这带来三个问题：
//!
//! * 用户必须先查清自己是哪台机器才能装——查 DMI、对表、挑包，全是多余步骤；
//! * 同一台机器换主板 / 刷 BIOS / 装到虚拟机后包名立刻变成「错的」；
//! * 机型表每加一行就要多发一轮包，九个包维护成本远高于收益。
//!
//! 现在只有一个包，机型表 [`SKUS`] 编译进二进制，启动时按**本机实际硬件**选配置：
//!
//! 1. 配置文件 / 命令行显式指定（`"sku": "g614"` 或 `--sku g614`）——最高优先，
//!    给「DMI 读不出型号」的机器留的备用通道（[`SkuSource::Manual`]）；
//! 2. DMI 型号字段命中（[`SkuSource::Dmi`]）；
//! 3. 触摸板模组名命中（[`SkuSource::Hardware`]）——DMI 写成占位串时的第二判据；
//! 4. 都没命中 → 拒绝启动（退出码 `3`）：本包只跑机型表里的机器。
//!
//! 命中机型只是第一道闸门。表内也有不带这块硬件的配置（同型号的 4 区 RGB 版共用
//! 同一块触摸板模组），所以启动前还要读触摸板 HID 描述符里的 FEATURE 报告 `0x0d`，
//! 没有就同样退出码 `3`。读取失败属于权限问题，按「未知」放行而不是当成没有硬件。
//!
//! ## 认不出机型就拒绝启动
//!
//! 早期版本在「认不出」时落一套通用配置照常运行。那套兜底看起来体贴，实际是拿别人
//! 的参数往未知硬件上套——跑起来像成功，出问题时无从判断该不该支持。现在不再猜：
//! DMI 型号与触摸板模组都对不上，启动闸门直接退出码 `3`，并打印支持的机型列表。
//!
//! 两条提示分开给：厂商字段明确不是 ASUS 时报「不是 ASUS 机器」，其余报「本机不在
//! 支持列表」。读不到 DMI 的机器（虚拟机、白牌机箱、BIOS 占位串）靠手动指定
//! `--sku` / `asus-numpad sku <id>` 放行，那是唯一的逃生通道。
//!
//! ## 机型表不收的机器：幻16 系列
//!
//! 幻16（华硕中国名，对应 Zephyrus M16 / G16 / Flow X16，`GU603`–`GV601`）逐代只有
//! `Touchpad`，没有 NumberPad，故不占表行——留行等于让闸门放行没有图标区的机器。
//! 带这块硬件的是双屏 `GX650`，已由 `gx550` 覆盖；核验表见 `docs/machines.md`。
//!
//! ## 不做的事
//!
//! 这里不做布局/键表判定，那仍然由 [`crate::layout`] 负责。本模块只回答一个问题：
//! **这台机器该用哪套机型参数。**

use std::path::Path;

/// 一个机型配置条目。
#[derive(Debug)]
pub struct Sku {
    /// 配置 id（也是 `--sku` / 配置文件的取值），如 `g614`
    pub id: &'static str,
    /// 给用户看的机型说明
    pub display: &'static str,
    /// DMI `product_name` 里出现的机型前缀；为空表示不做机型校验
    pub models: &'static [&'static str],
    /// 与 [`Sku::models`] 共用同一代号前缀、但**没有**该硬件的代际代号（如 `G513Q`
    /// 之于 `G513`）。命中即判定这不是本配置的机器，不参与 DMI 识别，改为继续走
    /// 触摸板模组判据。DMI 里没有年份字段，代际只能靠代号第三位字母区分。
    pub excludes: &'static [&'static str],
    /// 该机型号对应的触摸板模组名（`ASUE1416` / `ASCP1400` …），与
    /// [`crate::layout::HID_LAYOUTS`] 的键同源。硬件层面的第二判据，见 [`HardwareProbe`]。
    pub modules: &'static [&'static str],
    /// 该机型官方规格里的 NumberPad 结论，用于输出自证
    pub note: &'static str,
}

/// 机型配置表。新增机型只加一行——这张表是唯一源，出包脚本不再复制一份。
///
/// ## 分组规则
///
/// 一个配置服务一个**触摸板模组族**，而不是一个整机 SKU。同族的依据是
/// `layout.rs` 的 [`crate::layout::HID_LAYOUTS`]（官方 INF 的 HID 名 → 键表）：
/// 模组名相同（如 `ASUE1416` / `ASUF1416`）就一定是同一份键表、同一套几何。
///
/// * `g614` 吸收 `g634`：同一块 ELAN 触摸板（04F3:3275 / ASUE1416），键表 GH3XLNDZ；
/// * `g533` 吸收 `g513`：魔霸6 15 英寸与枪神6 同代同平台，模组 ASUE1403 / ELAN1403
///   → GH2XLNDZ 同族。2021 代枪神5 / 魔霸5 与 2022 代共用 `G533` / `G513` 前缀，
///   靠 `excludes`（`G533Q*` / `G513Q*` / `G513I*`）把它们挡在识别之外；
/// * `g615` 吸收 `g635`：PixArt ASCP1400 / ASCF1400 → GH4XLNDZ 同族；
/// * `gx550` 吸收 `gx650`：两代冰刃双屏同属 gx551 键位族。
///
/// 不并入任何配置的机型：17 英寸 `G713`（机身自带物理数字键）与 ProArt P16
/// `H7606`（触控板是 DialPad 智能旋钮）——这两类机器触控板里没有虚拟数字键盘，
/// 因此不占表行，启动时按「不在支持列表」拒绝启动，不会套用别的机型的参数。
///
/// 分组只影响「自动识别」的 DMI 白名单，运行期选哪套键表仍由实际 HID 模组名决定，
/// 所以分组不改变保守性：模组不在 `HID_LAYOUTS` 里，照样拒绝启动。
///
/// 历史机型包名（`asus-numpad-g614` 等）由统一包 Obsoletes 掉，老用户
/// `dnf upgrade` 会自动接管，不需要先手工卸载旧机型包。
pub const SKUS: &[Sku] = &[
    Sku {
        id: "g533",
        display: "ASUS 枪神6 / 魔霸6，15 英寸，2022（G533Z* / G513R*）",
        models: &["G533", "G513"],
        // 2021 代枪神5 / 魔霸5：代号第三位是 Q / I，官方规格页没有 NumberPad，
        // 靠模组判据之外再按代际挡一道，免得识别成 g533 给出「这台有小键盘」的错觉。
        excludes: &["G533Q", "G513Q", "G513I"],
        modules: &["ASUE1403", "ELAN1403"],
        note: "模组 ASUE1403 / ELAN1403 → 键表 GH2XLNDZ（同族）。2022 代（枪神6 G533Z*、魔霸6 15 英寸 G513R*）才有 NumberPad；2021 代（G533Q* / G513Q* / G513I*）官方规格无此硬件，由 excludes 挡在识别之外，不并入本配置。注意 2022 代也非全配：魔霸6 官方 5 个配置里 3 个是 4 区 RGB 版，无此硬件",
    },
    Sku {
        id: "g614",
        display: "ASUS 枪神7 / 枪神8 及超竞版，16 英寸，2023–2024（G614 / G634）",
        models: &["G614", "G634"],
        excludes: &[],
        modules: &["ASUE1416", "ASUF1416"],
        note: "G614 与 G634 同一块触摸板（ASUE1416 / ASUF1416 → GH3XLNDZ）；型号前缀不保证有硬件：2023 代官方 10 个配置里 2 个是 4 区 RGB 版（尾缀 EACX）无 NumberPad，国行「魔霸新锐」全系即此类，启动时探测不到 FEATURE 0x0d 即按不兼容拒绝启动",
    },
    Sku {
        id: "g615",
        display: "ASUS 枪神9 / 枪神10 及超竞版，16 英寸，2025–2026（G615 / G635）",
        models: &["G615", "G635"],
        excludes: &[],
        modules: &["ASCP1400", "ASCF1400"],
        note: "PixArt ASCP1400 / ASCF1400 → GH4XLNDZ（同族）。G615 为标准版、G635 为超竞版；2026 年的枪神10 沿用同一对代号，模组取值来自 2025 代。2025 代官方 25 个配置里有 1 个（90NR0LJ1-M005P0）是不带 NumberPad 的 4 区 RGB 版，国行「魔霸新锐 2025」（G614PM / G614PR 8940HX 与酷睿版）同属此类。同代的 ProArt P16（H7606）触控板是 DialPad 智能旋钮，不属于 NumberPad，不并入本配置",
    },
    Sku {
        id: "gx550",
        display: "ASUS 冰刃双屏，15 / 16 英寸，侧置触摸板（GX550 / GX551 / GX650）",
        models: &["GX550", "GX551", "GX650"],
        excludes: &[],
        modules: &["ASUE1408", "ASUE1404"],
        note: "GX550 / GX551 / GX650 同属 gx551 键位族；模组 ASUE1408 → GVM_A、ASUE1404 → GVM。GVM_A 与 GVM 共用同一份参数（官方 NUMPADLAYOUT_GVM_* 只列到 _06，其余项按缺省 0），键表状态是同族、未上真机。触摸板竖排在键盘右侧，数字模式由键盘上的专用键切换，不是长按图标区（见 README「双屏机型：侧置触摸板」）",
    },
    Sku {
        id: "ux582",
        display: "ASUS 灵耀双屏，侧置触摸板（UX582 / UX482）",
        models: &["UX582", "UX482"],
        excludes: &[],
        modules: &["ASUE1406", "ELAN1406"],
        note: "模组 ASUE1406 / ELAN1406 → 键表 NVM（官方 NUMPADLAYOUT_NVM_* 只列到 _12，其余项按缺省 0），键表状态是同族、未上真机。触摸板在键盘右侧，数字模式由键盘上的专用键切换，不是长按图标区",
    },
];

/// 机型配置是怎么选出来的——输出里要说清「凭什么判定是这台」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkuSource {
    /// 配置文件 `sku` 字段 / 命令行 `--sku` 显式指定
    Manual,
    /// DMI 型号字段命中
    Dmi,
    /// 触摸板模组名命中（DMI 读不出型号时的第二判据）
    Hardware,
}

impl Sku {
    /// 展示名：优先用 `sku.<id>.display` 的译文，该语言没翻译时回落到表里的中文原文。
    ///
    /// 机型说明是数据表里的常量，译文放在语言文件里；两边分开维护，避免为了
    /// 加一种语言去改识别逻辑。
    pub fn localized_display(&self) -> String {
        crate::i18n::t_or(&format!("sku.{}.display", self.id), self.display)
    }

    /// 判定依据说明，回落规则同 [`Sku::localized_display`]。
    pub fn localized_note(&self) -> String {
        crate::i18n::t_or(&format!("sku.{}.note", self.id), self.note)
    }
}

impl SkuSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkuSource::Manual => "manual",
            SkuSource::Dmi => "dmi",
            SkuSource::Hardware => "hardware",
        }
    }

    /// 对应 `sku.source.*` 这组 i18n 键。
    pub fn i18n_key(&self) -> &'static str {
        match self {
            SkuSource::Manual => "sku.source.manual",
            SkuSource::Dmi => "sku.source.dmi",
            SkuSource::Hardware => "sku.source.hardware",
        }
    }
}

/// 自动识别的结果：用哪套配置 + 凭什么。
#[derive(Debug, Clone)]
pub struct Resolved {
    pub sku: &'static Sku,
    pub source: SkuSource,
    /// 选中依据的细节：命中的机型代号 / 触摸板模组 / 手动指定的 id
    pub detail: Option<String>,
}

impl Resolved {
    pub fn id(&self) -> &'static str {
        self.sku.id
    }
}

/// 手动指定的机型 id 不在表里。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSku(pub String);

impl std::fmt::Display for UnknownSku {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown model id: {}", self.0)
    }
}

impl std::error::Error for UnknownSku {}

/// 本机不在支持列表里——启动闸门据此退出码 `3`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    /// 厂商字段明确不是 ASUS 时带上是哪家；`None` 表示只是认不出机型
    pub non_asus: Option<String>,
}

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.non_asus {
            Some(vendor) => write!(f, "not an ASUS machine ({vendor})"),
            None => f.write_str("no supported model matched"),
        }
    }
}

impl std::error::Error for Unsupported {}

/// 识别失败：手动 id 无效，或本机不在支持列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    UnknownId(UnknownSku),
    Unsupported(Unsupported),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::UnknownId(e) => e.fmt(f),
            ResolveError::Unsupported(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ResolveError {}

/// 机型表里所有可选 id。
pub fn known_ids() -> Vec<&'static str> {
    SKUS.iter().map(|s| s.id).collect()
}

/// 纯逻辑：DMI 身份 × 硬件探测 × 手动指定 → 该用哪套机型配置。
///
/// 判定顺序（越靠前越优先）：
///
/// 1. 手动指定（配置文件 `sku` / `--sku`）→ `Manual`，认了这个 id 就不再质疑；
/// 2. DMI 型号字段命中某条机的 `models` → `Dmi`（该条机的 `excludes` 命中的代际除外）；
/// 3. 触摸板模组命中某条机的 `modules` → `Hardware`；
/// 4. 都没命中 → `Unsupported`，调用方（启动闸门）退出码 `3`。
///
/// 第 4 步刻意不留兜底：认不出机型就不跑，避免用别人的参数在未知硬件上「看起来跑起来了」。
/// 读不到 DMI 的机器靠手动指定放行，不必依赖兜底。
pub fn resolve(
    identity: &DmiIdentity,
    hw: &HardwareProbe,
    manual: Option<&str>,
) -> Result<Resolved, ResolveError> {
    if let Some(id) = manual {
        if !id.trim().is_empty() {
            let sku = by_id(id)
                .ok_or_else(|| ResolveError::UnknownId(UnknownSku(id.trim().to_string())))?;
            // 手动指定等于「我确认这台就是它」，不再让 DMI 反过来否掉用户的选择
            return Ok(Resolved {
                sku,
                source: SkuSource::Manual,
                detail: Some(sku.id.to_string()),
            });
        }
    }

    if let Some((sku, model)) = dmi_hit(identity) {
        return Ok(Resolved {
            sku,
            source: SkuSource::Dmi,
            detail: Some(model.to_string()),
        });
    }

    for sku in SKUS.iter().filter(|s| !s.modules.is_empty()) {
        if let Some(module) = hw.first_hit(sku) {
            return Ok(Resolved {
                sku,
                source: SkuSource::Hardware,
                detail: Some(module.to_string()),
            });
        }
    }

    Err(ResolveError::Unsupported(Unsupported {
        non_asus: non_asus_vendor(identity),
    }))
}

/// 厂商字段明确表明不是 ASUS 时返回厂商名，否则 `None`。
fn non_asus_vendor(identity: &DmiIdentity) -> Option<String> {
    if identity.is_asus() != Some(false) {
        return None;
    }
    Some(
        identity
            .sys_vendor
            .as_deref()
            .or(identity.board_vendor.as_deref())
            .unwrap_or("unknown")
            .trim()
            .to_string(),
    )
}

/// 本机该用哪套机型配置（读真实 DMI 与触摸板）；`manual` 来自配置 / 命令行。
pub fn current(manual: Option<&str>) -> Result<Resolved, ResolveError> {
    resolve(&dmi_identity(), &probe_hardware(), manual)
}

/// 纯逻辑：DMI 型号 → 命中的配置条目与命中的那个代号。
///
/// 多条同时命中时取 `model` 最长的那个：`G614` 与 `G614JY` 都在表里时，
/// 更长的代号更精确。
fn dmi_hit(identity: &DmiIdentity) -> Option<(&'static Sku, &'static str)> {
    let pools = identity.haystacks();
    if pools.is_empty() {
        return None;
    }
    // 各 DMI 字段的型号 token 只抽一次：之前抽在 matches_model 里，机型表每条 model
    // 都要把所有字段重新切一遍，机型表长大后全是重复分配。
    let tokens: Vec<Vec<String>> = pools.iter().map(|p| model_tokens(p)).collect();
    let mut best: Option<(&'static Sku, &'static str)> = None;
    for sku in SKUS.iter().filter(|s| !s.models.is_empty()) {
        // 代际排除优先于型号命中：同前缀的老代际机器没有这块硬件，不参与识别，
        // 继续往下走触摸板模组判据。
        if sku
            .excludes
            .iter()
            .any(|ex| matches_model(ex, &pools, &tokens))
        {
            continue;
        }
        for model in sku.models {
            if !matches_model(model, &pools, &tokens) {
                continue;
            }
            let better = match best {
                Some((_, cur)) => model.len() > cur.len(),
                None => true,
            };
            if better {
                best = Some((sku, model));
            }
        }
    }
    best
}

/// 按 id 查机型（大小写不敏感）。
pub fn by_id(id: &str) -> Option<&'static Sku> {
    SKUS.iter().find(|s| s.id.eq_ignore_ascii_case(id.trim()))
}

/// DMI 身份快照：厂商 + 产品名 + 主板名 + 产品家族。
///
/// 单看 `product_name` 不够用，实际机器上这几种情况都真实存在：
///
/// * 部分机型 `product_name` 只有营销名（`ROG Strix SCAR 16`），机型编号 `G614` 只落在
///   `board_name` 或 `product_family` 里；
/// * 部分机型 `product_name` 是 `G614JY_G614JY`，编号重复两遍（老前缀匹配能吃下，
///   但 `G614JY` 里的 `G614` 也可能被 `G614X` 的规则误吃，这里用整串多字段比对更稳）；
/// * 白牌 / 虚拟机会写 `System Product Name`、`To Be Filled By O.E.M.` 这类占位串，
///   当机型处理会误判成「不是 ASUS 机器」而拦下正常安装。
///
/// 所以判定改成「多字段取并集 + 占位串归一 + 厂商字段定性质」。
#[derive(Debug, Clone, Default)]
pub struct DmiIdentity {
    /// `/sys/class/dmi/id/sys_vendor`，正常为 `ASUSTeK COMPUTER INC.`
    pub sys_vendor: Option<String>,
    /// `/sys/class/dmi/id/board_vendor`
    pub board_vendor: Option<String>,
    /// `/sys/class/dmi/id/product_name`，如 `ROG Strix SCAR 16 G614JY_G614JY`
    pub product_name: Option<String>,
    /// `/sys/class/dmi/id/board_name`，部分机型只有这里有编号
    pub board_name: Option<String>,
    /// `/sys/class/dmi/id/product_family`
    pub product_family: Option<String>,
}

/// DMI 里常见的占位串（BIOS 没填真值时就写这些），一律当「读不到」处理。
const DMI_PLACEHOLDERS: &[&str] = &[
    "SYSTEM PRODUCT NAME",
    "SYSTEM MANUFACTURER",
    "SYSTEM VERSION",
    "TO BE FILLED BY O.E.M.",
    "TO BE FILLED BY OEM",
    "DEFAULT STRING",
    "NOT SPECIFIED",
    "NOT APPLICABLE",
    "INVALID",
    "UNKNOWN",
    "OEM",
    "O.E.M.",
];

impl DmiIdentity {
    /// 有效字段：去掉空值与占位串，统一大写，去重。
    ///
    /// 返回顺序固定为「产品名 → 主板名 → 产品家族」，便于输出与测试稳定。
    pub fn haystacks(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for field in [&self.product_name, &self.board_name, &self.product_family] {
            let Some(raw) = field.as_deref() else {
                continue;
            };
            let up = raw.trim().to_ascii_uppercase();
            if up.is_empty() || DMI_PLACEHOLDERS.contains(&up.as_str()) {
                continue;
            }
            if !out.contains(&up) {
                out.push(up);
            }
        }
        out
    }

    /// 厂商字段的结论：`Some(true)` 明确是 ASUS、`Some(false)` 明确不是、`None` 判断不了。
    ///
    /// 只在两个厂商字段都读到实值、且都不含 ASUS 时才敢说「不是」；
    /// 任一字段缺失（容器、非 Linux）都返回 `None`，交给后续按「未知」放行。
    pub fn is_asus(&self) -> Option<bool> {
        let mut seen = false;
        for field in [&self.sys_vendor, &self.board_vendor] {
            let Some(raw) = field.as_deref() else {
                continue;
            };
            let up = raw.trim().to_ascii_uppercase();
            if up.is_empty() || DMI_PLACEHOLDERS.contains(&up.as_str()) {
                continue;
            }
            seen = true;
            if up.contains("ASUS") {
                return Some(true);
            }
        }
        if seen {
            Some(false)
        } else {
            None
        }
    }

    /// 供日志输出的一行身份摘要（只列有效字段）。
    pub fn label(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = &self.sys_vendor {
            if !v.trim().is_empty() {
                parts.push(format!("vendor={}", v.trim()));
            }
        }
        if let Some(v) = &self.product_name {
            if !v.trim().is_empty() {
                parts.push(format!("product={}", v.trim()));
            }
        }
        if let Some(v) = &self.board_name {
            let t = v.trim();
            if !t.is_empty() && self.product_name.as_deref().map(str::trim) != Some(t) {
                parts.push(format!("board={t}"));
            }
        }
        if let Some(v) = &self.product_family {
            if !v.trim().is_empty() {
                parts.push(format!("family={}", v.trim()));
            }
        }
        if parts.is_empty() {
            "unknown".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// 硬件探测结果：这台机器上真实存在的**触摸板模组名**。
///
/// DMI 是 BIOS 写的字符串，会写错、会写成占位串、会因为主板换装而对不上；
/// 触摸板模组名（`ASUE1416:00` 这种）来自 i2c-hid / HID 子系统，直接反映硬件本身，
/// 因此作为 DMI 之外的第二判据——DMI 说不上来时靠它放行，DMI 与实物冲突时也以它为准。
#[derive(Debug, Clone, Default)]
pub struct HardwareProbe {
    /// 归一化后的模组名（大写、去掉 `:00` 之类的实例后缀），去重
    pub modules: Vec<String>,
}

impl HardwareProbe {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// 命中的第一个模组名（用于日志输出「凭什么判定是它」）。
    pub fn first_hit(&self, sku: &Sku) -> Option<&str> {
        let hit = self.modules.iter().find(|m| {
            sku.modules.iter().any(|want| {
                let want = want.to_ascii_uppercase();
                // 完全相等，或本机串是「模组名 + 尾巴」（`ASUE1416` vs `ASUE1416-PIXA`）
                m.as_str() == want.as_str() || m.starts_with(&want)
            })
        })?;
        Some(hit.as_str())
    }

    /// 供日志输出的一行硬件摘要。
    pub fn label(&self) -> String {
        if self.modules.is_empty() {
            "unknown".to_string()
        } else {
            self.modules.join(", ")
        }
    }
}

/// 已知触摸板模组的厂前缀。
///
/// 归一化时只认这些前缀，避免把 i2c/HID 总线上顺手读到的 `PNP0C50`（ACPI ID）、
/// `HID-SENSOR` 之类当成模组名——宁可少数认不出，也不让无关串把机型判定带偏。
const MODULE_PREFIXES: &[&str] = &[
    "ASUE", "ASUF", "ASUP", "ASCP", "ASCF", "ELAN", "ATML", "SYNA", "FTS", "GTCH",
];

/// 归一化模组名：大写、去空白、砍掉 `:00` / `.0001` 这类实例后缀。
fn normalize_module(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let cut = trimmed
        .find(|c: char| c == ':' || c == '.' || c.is_whitespace())
        .unwrap_or(trimmed.len());
    let head = &trimmed[..cut];
    if head.len() < 4 {
        return None;
    }
    // 只留「字母开头 + 含数字」的标识（ASUE1416 / ELAN1403 / ASCP1400），
    // 滤掉 i2c 总线上那些 `SynPS/2`、`PNP0C50` 之类的无关名字
    let bytes = head.as_bytes();
    if !bytes[0].is_ascii_alphabetic() {
        return None;
    }
    if !head.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if !head.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let up = head.to_ascii_uppercase();
    if !MODULE_PREFIXES.iter().any(|p| up.starts_with(p)) {
        return None;
    }
    Some(up)
}

/// 从给定 sysfs 根与 input 设备表读硬件标识（便于测试）。
///
/// 三个来源互补：i2c-hid 触摸板在 `bus/i2c/devices/*/name`，HID 层在
/// `bus/hid/devices/*/HID_NAME`，老内核/部分模组只在 `/proc/bus/input/devices` 里露面。
pub fn probe_hardware_at(sys_root: &Path, input_devices: &Path) -> HardwareProbe {
    let mut modules: Vec<String> = Vec::new();
    let mut push = |raw: &str| {
        if let Some(m) = normalize_module(raw) {
            if !modules.contains(&m) {
                modules.push(m);
            }
        }
    };

    for bus in ["bus/i2c/devices", "bus/hid/devices"] {
        let dir = sys_root.join(bus);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            for name in ["name", "HID_NAME"] {
                if let Ok(text) = std::fs::read_to_string(entry.path().join(name)) {
                    push(&text);
                }
            }
        }
    }

    if let Ok(text) = std::fs::read_to_string(input_devices) {
        for line in text.lines() {
            // `N: Name="ASUE1416:00 04F3:3275 Touchpad"` 与 `I: ...` 两类行都扫一遍
            for field in ["Name=\"", "Name: "] {
                if let Some(pos) = line.find(field) {
                    let rest = &line[pos + field.len()..];
                    let end = rest.find('"').unwrap_or(rest.len());
                    for token in rest[..end].split_whitespace() {
                        push(token);
                    }
                }
            }
        }
    }

    HardwareProbe { modules }
}

/// 读本机硬件标识（非 Linux 环境返回空，判定退回 DMI 路径）。
pub fn probe_hardware() -> HardwareProbe {
    probe_hardware_at(Path::new("/sys"), Path::new("/proc/bus/input/devices"))
}

/// 从任意 DMI 字符串里抽出「型号代号」token（`G614` / `GX650` / `H7606` / `G614JY`）。
///
/// 判定规则：按非字母数字切分，段长 ≥ 4、以 ASCII 字母开头、且至少含 2 位数字。
///
/// 这样串里的营销词（`ROG` / `STRIX` / `SCAR`）和纯数字（`16`）会被自然滤掉，
/// 剩下的才是可用于比对的代号；像 `G614JY_G614JY` 这种重复写法会被切成两个相同的
/// `G614JY`，不影响判定。
pub fn model_tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for seg in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if seg.len() < 4 {
            continue;
        }
        let bytes = seg.as_bytes();
        if !bytes[0].is_ascii_alphabetic() {
            continue;
        }
        if seg.chars().filter(|c| c.is_ascii_digit()).count() < 2 {
            continue;
        }
        let up = seg.to_ascii_uppercase();
        if !out.contains(&up) {
            out.push(up);
        }
    }
    out
}

/// 机型表里的 `model` 是否能对上这批 DMI 字段。
///
/// 优先按**型号代号 token** 比对，消除「子串匹配吃进邻近型号」的老问题：
///
/// * `model == token` → 命中；
/// * `token` 以 `model` 开头、且 `model` 之后的第一位**不是数字** → 命中
///   （`G614JY` / `G614JVR` 归 `G614`；`G6140` 不会被 `G614` 吃）；
/// * 某个字段整串抽不出 token（纯营销名，如 `ROG STRIX SCAR 16`）→ 退回旧的
///   整体子串匹配，宁可宽松也不误判这种读不出代号的机器。
fn matches_model(model: &str, pools: &[String], pool_tokens: &[Vec<String>]) -> bool {
    let up = model.to_ascii_uppercase();
    if !up.is_ascii() || up.is_empty() {
        return pools.iter().any(|p| p.to_ascii_uppercase().contains(&up));
    }
    for (pool, tokens) in pools.iter().zip(pool_tokens.iter()) {
        if tokens.is_empty() {
            if pool.to_ascii_uppercase().contains(&up) {
                return true;
            }
            continue;
        }
        for tok in tokens {
            if tok == &up {
                return true;
            }
            if tok.len() > up.len() && tok.starts_with(&up) {
                // up 全是 ASCII，按字节取界安全
                if !tok.as_bytes()[up.len()].is_ascii_digit() {
                    return true;
                }
            }
        }
    }
    false
}

/// 读单个 DMI 字段，空串按「读不到」处理。
fn read_dmi_field(root: &Path, name: &str) -> Option<String> {
    let text = std::fs::read_to_string(root.join(name)).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn identity_at(root: &Path) -> DmiIdentity {
    let id_dir = root.join("class/dmi/id");
    DmiIdentity {
        sys_vendor: read_dmi_field(&id_dir, "sys_vendor"),
        board_vendor: read_dmi_field(&id_dir, "board_vendor"),
        product_name: read_dmi_field(&id_dir, "product_name"),
        board_name: read_dmi_field(&id_dir, "board_name"),
        product_family: read_dmi_field(&id_dir, "product_family"),
    }
}

/// 读本机 DMI 身份（Linux `/sys/class/dmi/id/`，其它平台全部字段为 `None`）。
///
/// 任一字段读不到都不影响其余字段——权限受限的容器里常见只有 `product_name` 可读。
/// 本机机型能确定的唯一键表名。
///
/// 设备名里读不到 HID 模组 ID 时（`hid-generic` 命名、个别内核不上报前缀），键表就失去
/// 了唯一依据。此时若 DMI 已把机型定下来，用该机型的模组表反查：**所有模组都指向同一套
/// 键表**才返回它，指向多套说明这套参数不足以定键表，返回 `None` 让调用方回落默认。
pub fn layout_name_for_local_machine(forced: Option<&str>) -> Option<&'static str> {
    let resolved = resolve(&dmi_identity(), &probe_hardware(), forced).ok()?;
    layout_name_for(resolved.sku)
}

/// 单个机型配置能否唯一确定键表（见 [`layout_name_for_local_machine`]）。
pub fn layout_name_for(sku: &Sku) -> Option<&'static str> {
    let mut names = sku
        .modules
        .iter()
        .filter_map(|m| crate::layout::for_hid_id(m))
        .map(|e| e.name);
    let first = names.next()?;
    if names.all(|n| n == first) {
        Some(first)
    } else {
        None
    }
}

pub fn dmi_identity() -> DmiIdentity {
    identity_at(Path::new("/sys"))
}

/// 从指定 sysfs 根目录读 DMI 身份（便于测试与容器内排查）。
pub fn dmi_identity_at(root: &Path) -> DmiIdentity {
    identity_at(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(product: Option<&str>, board: Option<&str>, family: Option<&str>) -> DmiIdentity {
        DmiIdentity {
            sys_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            board_vendor: Some("ASUSTeK COMPUTER INC.".to_string()),
            product_name: product.map(str::to_string),
            board_name: board.map(str::to_string),
            product_family: family.map(str::to_string),
        }
    }

    /// 只有 product_name 有值的身份——绝大多数真机就是这种形态。
    fn dmi(product: &str) -> DmiIdentity {
        ident(Some(product), None, None)
    }

    fn hw(modules: &[&str]) -> HardwareProbe {
        HardwareProbe {
            modules: modules.iter().map(|m| m.to_ascii_uppercase()).collect(),
        }
    }

    fn no_hw() -> HardwareProbe {
        HardwareProbe::default()
    }

    #[test]
    fn sku_ids_are_unique_and_complete() {
        let mut ids: Vec<&str> = SKUS.iter().map(|s| s.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "SKU id 必须唯一");
        assert_eq!(SKUS.len(), 5, "机型表只保留五个模组族");
        assert!(
            by_id("universal").is_none(),
            "不再有通用配置：认不出机型就拒绝启动"
        );
        assert!(by_id("G614").is_some(), "机型查询应忽略大小写");
        // g513 并入 g533；g635 并入 g615；gx650 并入 gx550；ux482 并入 ux582。
        // g713 / h7606 不并入任何配置：触控板里没有虚拟数字键盘
        for merged in ["g634", "g635", "gx650", "g513", "g713", "h7606"] {
            assert!(
                by_id(merged).is_none(),
                "{merged} 已并入同族配置，不应再单独存在"
            );
        }
        for s in SKUS {
            assert!(!s.display.is_empty(), "{} 缺少展示名", s.id);
            assert!(!s.note.is_empty(), "{} 缺少说明", s.id);
            // 没有 DMI 白名单的条目永远只能靠硬件判据命中，等于退化成兜底，不允许
            assert!(!s.models.is_empty(), "{} 必须有自己的 DMI 型号", s.id);
            assert!(!s.modules.is_empty(), "{} 必须绑定触摸板模组", s.id);
            // 模组名与 layout::HID_LAYOUTS 的键同源，写错一个字母就等于该机型永远
            // 走不到硬件判据
            for m in s.modules {
                assert!(
                    crate::layout::for_hid_id(m).is_some(),
                    "{} 的模组 {m} 不在 HID 布局表里",
                    s.id
                );
            }
        }
        let known = known_ids();
        for id in ["g533", "g614", "g615", "gx550", "ux582"] {
            assert!(known.contains(&id), "{id} 应在机型表里");
        }
    }

    /// 设备名读不到模组 ID 时的键表反查：只有模组全指向同一套键表才给结果。
    #[test]
    fn sku_pins_layout_only_when_modules_agree() {
        for (id, want) in [
            ("g614", Some("GH3XLNDZ")),
            ("g615", Some("GH4XLNDZ")),
            ("g533", Some("GH2XLNDZ")),
            ("ux582", Some("NVM")),
            // 两代冰刃双屏模组分别指向 GVM_A / GVM，反查没有唯一答案，必须回落默认
            ("gx550", None),
        ] {
            let sku = by_id(id).expect("机型必须在表内");
            assert_eq!(layout_name_for(sku), want, "{id} 的键表反查结果不对");
        }
    }

    #[test]
    fn dmi_auto_selects_the_matching_config() {
        let cases = [
            ("ROG Strix SCAR 16 G634JY_G634JY", "g614"),
            ("ROG Strix SCAR 16 G614JZ_G614JZ", "g614"),
            ("ROG Strix SCAR 16 G615LR_G615LR", "g615"),
            ("ROG Strix SCAR 16 G635LX_G635LX", "g615"),
            ("ROG Strix SCAR 15 G533ZW_G533ZW", "g533"),
            ("ROG Strix G15 G513RM_G513RM", "g533"),
            ("ROG Zephyrus Duo 16 GX650PZ_GX650PZ", "gx550"),
            ("ROG Zephyrus Duo 15 GX550LXS_GX550LXS", "gx550"),
            ("ROG Zephyrus Duo 15 SE GX551QS_GX551QS", "gx550"),
            ("ASUS Zenbook Duo UX582LR_UX582LR", "ux582"),
            ("ASUS Zenbook Duo UX482EG_UX482EG", "ux582"),
        ];
        for (product, want) in cases {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert_eq!(r.id(), want, "{product}");
            assert_eq!(r.source, SkuSource::Dmi, "{product}");
            assert!(r.detail.is_some(), "{product} 应说明命中的代号");
        }
    }

    #[test]
    fn one_binary_serves_every_model() {
        // 同一份二进制在两种机器上得到两套配置：这正是取消分包后要保证的行为
        let a = resolve(&dmi("ROG Strix SCAR 16 G634JY_G634JY"), &no_hw(), None).unwrap();
        let b = resolve(&dmi("ROG Strix SCAR 16 G615LR_G615LR"), &no_hw(), None).unwrap();
        assert_eq!(a.id(), "g614");
        assert_eq!(b.id(), "g615");
        assert_ne!(a.sku.models, b.sku.models);
    }

    #[test]
    fn model_id_found_in_board_or_family_field() {
        // 产品名只有营销名，编号在主板名里
        let r = resolve(
            &ident(Some("ROG Strix SCAR 16"), Some("G614JY"), None),
            &no_hw(),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "g614");
        assert_eq!(r.source, SkuSource::Dmi);
        // 编号只在产品家族里
        let r = resolve(
            &ident(Some("ROG Strix SCAR 16"), None, Some("G614JZ")),
            &no_hw(),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "g614");
        // 只有营销名、没有编号：认不出机型就报不支持，不再落通用配置
        let err = resolve(
            &ident(Some("ROG Strix SCAR 16"), None, None),
            &no_hw(),
            None,
        )
        .unwrap_err();
        assert!(
            matches!(err, ResolveError::Unsupported(ref u) if u.non_asus.is_none()),
            "{err}"
        );
    }

    #[test]
    fn hardware_module_is_the_second_judgement() {
        // DMI 里没有机型编号，但触摸板模组对得上 → 按硬件选配置
        let r = resolve(&dmi("ROG Strix SCAR 16"), &hw(&["ASUE1416:00"]), None).unwrap();
        assert_eq!(r.id(), "g614");
        assert_eq!(r.source, SkuSource::Hardware);
        assert!(r.detail.as_deref().unwrap_or("").starts_with("ASUE1416"));
        // 模组不在任何机型名单里 → 不支持
        let err = resolve(
            &dmi("ROG Strix SCAR 16"),
            &hw(&["PNP0C50", "HID-SENSOR-2000"]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ResolveError::Unsupported(_)), "{err}");
        // DMI 写错、模组属于别的机型族时，硬件判据把它拉回正确配置
        let r = resolve(
            &dmi("ROG Strix G15 G513RM_G513RM"),
            &hw(&["ASUE1403:00"]),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "g533");
        // 既不在机型表、模组也不认识的机器：拒绝，不再兜底
        let err = resolve(
            &dmi("ROG Strix SCAR 18 G834JY_G834JY"),
            &hw(&["PNP0C50"]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, ResolveError::Unsupported(_)), "{err}");
    }

    #[test]
    fn manual_selection_overrides_detection() {
        // 手动指定优先于 DMI：读错硬件信息时这是唯一的逃生通道
        let r = resolve(
            &dmi("ROG Strix SCAR 16 G634JY_G634JY"),
            &no_hw(),
            Some("gx550"),
        )
        .unwrap();
        assert_eq!(r.id(), "gx550");
        assert_eq!(r.source, SkuSource::Manual);
        assert_eq!(r.detail.as_deref(), Some("gx550"));
        // 大小写与空白不敏感
        let r = resolve(&no_identity(), &no_hw(), Some("  GX550 ")).unwrap();
        assert_eq!(r.id(), "gx550");
        // 通用配置已经取消：手动指定 universal 也该报 id 不存在
        let err = resolve(&no_identity(), &no_hw(), Some("universal")).unwrap_err();
        assert_eq!(
            err,
            ResolveError::UnknownId(UnknownSku("universal".to_string()))
        );
        // 空字符串等同没指定
        let r = resolve(&dmi("ROG Strix SCAR 16 G634JY"), &no_hw(), Some("   ")).unwrap();
        assert_eq!(r.source, SkuSource::Dmi);
    }

    #[test]
    fn manual_unknown_id_is_an_error() {
        let err = resolve(&no_identity(), &no_hw(), Some("g999")).unwrap_err();
        assert_eq!(err, ResolveError::UnknownId(UnknownSku("g999".to_string())));
    }

    fn no_identity() -> DmiIdentity {
        DmiIdentity::default()
    }

    #[test]
    fn previous_generation_sharing_a_prefix_is_excluded() {
        // 2021 代枪神5（G533Q*）/ 魔霸5（G513Q*、G513I*）与 2022 代共用 G533 / G513 前缀，
        // 但官方规格页没有 NumberPad：不能被识别成 g533，也不该被当成支持机型。
        for product in [
            "ROG Strix SCAR 15 G533QM_G533QM",
            "ROG Strix SCAR 15 G533QR_G533QR",
            "ROG Strix SCAR 15 G533QS_G533QS",
            "ROG Strix G15 G513QM_G513QM",
            "ROG Strix G15 G513QR_G513QR",
            "ROG Strix G15 G513IC_G513IC",
            "ROG Strix G15 G513IH_G513IH",
        ] {
            let err = resolve(&dmi(product), &no_hw(), None).unwrap_err();
            assert!(
                matches!(err, ResolveError::Unsupported(_)),
                "{product}: {err}"
            );
        }
        // 逃生通道照样在：手动指定 g533 立即生效
        let r = resolve(&dmi("ROG Strix G15 G513QM_G513QM"), &no_hw(), Some("g533")).unwrap();
        assert_eq!(r.id(), "g533");
        assert_eq!(r.source, SkuSource::Manual);
        // 主板名写错、但确实插着 2022 代的模组：硬件判据仍能把它拉回 g533
        let r = resolve(
            &dmi("ROG Strix G15 G513QM_G513QM"),
            &hw(&["ASUE1403:00"]),
            None,
        )
        .unwrap();
        assert_eq!(r.id(), "g533");
        assert_eq!(r.source, SkuSource::Hardware);
        // 排除项不会误伤 2022 代：R / Z 开头的代号照旧命中
        for (product, want) in [
            ("ROG Strix SCAR 15 G533ZW_G533ZW", "g533"),
            ("ROG Strix G15 G513RM_G513RM", "g533"),
            ("ROG Strix G15 G513RC_G513RC", "g533"),
        ] {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert_eq!(r.id(), want, "{product}");
            assert_eq!(r.source, SkuSource::Dmi, "{product}");
        }
    }

    #[test]
    fn models_outside_the_table_are_rejected() {
        // ProArt P16（H7606，DialPad 旋钮）、18 英寸、17 英寸 G17（物理数字键）
        // 都不在机型表里：直接拒绝，不再拿一套通用参数跑上去。
        for product in [
            "ASUS ProArt P16 H7606WI_H7606WI",
            "ROG Strix SCAR 18 G834JY_G834JY",
            "ROG Strix G18 G814JZ_G814JZ",
            "ROG Strix G17 G713RC_G713RC",
            "ROG Strix G17 G713QR_G713QR",
        ] {
            let err = resolve(&dmi(product), &no_hw(), None).unwrap_err();
            assert!(
                matches!(err, ResolveError::Unsupported(_)),
                "{product}: {err}"
            );
        }
    }

    #[test]
    fn real_gaming_models_are_recognised() {
        for product in [
            "ROG Strix SCAR 16 G615LR_G615LR",
            "ROG Strix SCAR 16 G635LX_G635LX",
            "ROG Strix SCAR 16 G634JY_G634JY",
        ] {
            let r = resolve(&dmi(product), &no_hw(), None).unwrap();
            assert_ne!(r.id(), "universal", "{product} 应认出具体配置");
        }
    }

    #[test]
    fn non_asus_vendor_is_reported_as_non_asus() {
        let id = DmiIdentity {
            sys_vendor: Some("LENOVO".to_string()),
            board_vendor: Some("LENOVO".to_string()),
            product_name: Some("Legion 7 16IRX8H".to_string()),
            board_name: Some("LNVNB161216".to_string()),
            product_family: None,
        };
        let err = resolve(&id, &no_hw(), None).unwrap_err();
        match &err {
            ResolveError::Unsupported(u) => {
                assert!(u.non_asus.as_deref().unwrap_or("").contains("LENOVO"));
            }
            other => panic!("应报 unsupported：{other}"),
        }
        // 没认出机型（厂商仍是 ASUS）时不能误报「不是 ASUS」
        let err = resolve(&dmi("ROG Strix SCAR 16"), &no_hw(), None).unwrap_err();
        match &err {
            ResolveError::Unsupported(u) => assert!(u.non_asus.is_none()),
            other => panic!("应报 unsupported：{other}"),
        }
    }

    #[test]
    fn unreadable_dmi_is_rejected() {
        // 读不到 DMI 不再兜底放行：要么手动指定，要么拒绝启动
        let err = resolve(&no_identity(), &no_hw(), None).unwrap_err();
        assert!(
            matches!(err, ResolveError::Unsupported(ref u) if u.non_asus.is_none()),
            "{err}"
        );
        // 型号在表外的新机型同样拒绝
        let err = resolve(&dmi("ROG Strix SCAR 16 G999ZZ"), &no_hw(), None).unwrap_err();
        assert!(matches!(err, ResolveError::Unsupported(_)), "{err}");
    }

    #[test]
    fn placeholders_count_as_unreadable() {
        // BIOS 没填真值：占位串不能被当成「别的机型」，但也不能当成支持机型
        let id = ident(
            Some("System Product Name"),
            Some("To Be Filled By O.E.M."),
            Some("Default string"),
        );
        assert!(id.haystacks().is_empty());
        let err = resolve(&id, &no_hw(), None).unwrap_err();
        assert!(
            matches!(err, ResolveError::Unsupported(ref u) if u.non_asus.is_none()),
            "{err}"
        );
        assert!(DmiIdentity::default().haystacks().is_empty());
    }

    #[test]
    fn dmi_file_read_and_auto_selection() {
        let root = std::env::temp_dir().join("asus-numpad-dmi-selftest");
        let dir = root.join("class/dmi/id");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("product_name"),
            "ROG Strix SCAR 16 G634JY_G634JY\n",
        )
        .unwrap();

        // 读到并去掉换行
        assert_eq!(
            dmi_identity_at(&root).product_name.as_deref(),
            Some("ROG Strix SCAR 16 G634JY_G634JY")
        );
        // 只有 product_name 时也能自动选中对应配置
        let r = resolve(&dmi("ROG Strix SCAR 16 G634JY_G634JY"), &no_hw(), None).unwrap();
        assert_eq!(r.id(), "g614");

        // 空文件视为未知：拒绝启动
        std::fs::write(dir.join("product_name"), "  \n").unwrap();
        assert_eq!(dmi_identity_at(&root).product_name, None);
        assert!(resolve(&no_identity(), &no_hw(), None).is_err());

        let _ = std::fs::remove_dir_all(&root);
    }
    /// 机型表与布局表必须对得上：每个 `modules` 都要在官方 HID 名表里，
    /// 且映射到的键表真实存在。两张表分别维护，任何一边改名而另一边忘了跟，
    /// 表现就是「机型识别过了，布局却落到 fallback」。
    #[test]
    fn sku_modules_all_known_to_layout_table() {
        for sku in SKUS {
            for module in sku.modules {
                let layout = crate::layout::HID_LAYOUTS
                    .iter()
                    .find(|(hid, _)| hid == module)
                    .map(|(_, name)| *name)
                    .unwrap_or_else(|| panic!("{}: 模组 {} 不在 HID_LAYOUTS 里", sku.id, module));
                assert!(
                    crate::layout::LAYOUTS.iter().any(|e| e.name == layout),
                    "{}: 模组 {} 映射到 {}，但 LAYOUTS 里没有这套键表",
                    sku.id,
                    module,
                    layout
                );
            }
        }
    }

    /// 机型表的 id 唯一、识别依据不为空：id 是全表主键（配置文件与 `--sku` 都按它取值）。
    #[test]
    fn sku_ids_unique_and_identifiable() {
        let mut seen = std::collections::HashSet::new();
        for sku in SKUS {
            assert!(!sku.id.is_empty(), "机型 id 不能为空");
            assert!(seen.insert(sku.id), "机型 id 重复：{}", sku.id);
            assert!(
                !sku.models.is_empty() || !sku.modules.is_empty(),
                "{}: 型号与模组都为空，这条配置永远不会被识别到",
                sku.id
            );
            assert!(!sku.display.is_empty(), "{}: 缺少展示名", sku.id);
            assert!(!sku.note.is_empty(), "{}: 缺少判定依据说明", sku.id);
        }
    }
    /// 三语里的机型文案必须覆盖每个机型：漏一条，那个机型在对应语言下就只剩原文。
    #[test]
    fn sku_texts_present_in_all_languages() {
        for (lang, embed) in [
            ("zh_CN", crate::i18n::EMBED_ZH_CN),
            ("zh_TW", crate::i18n::EMBED_ZH_TW),
            ("en", crate::i18n::EMBED_EN),
        ] {
            let map = crate::i18n::parse_lang_text(embed);
            for sku in SKUS {
                for field in ["display", "note"] {
                    let key = format!("sku.{}.{}", sku.id, field);
                    let value = map.get(&key).unwrap_or_else(|| panic!("{lang} 缺少 {key}"));
                    assert!(!value.trim().is_empty(), "{lang} 的 {key} 是空文案");
                }
            }
        }
    }

    /// 简体中文那份必须与表里的原文一字不差：改识别结论时若只改一边，
    /// 中文用户看到的还是旧说法，英文用户反而先更新。
    #[test]
    fn zh_cn_texts_match_table_verbatim() {
        let map = crate::i18n::parse_lang_text(crate::i18n::EMBED_ZH_CN);
        for sku in SKUS {
            assert_eq!(
                map.get(&format!("sku.{}.display", sku.id))
                    .map(|s| s.as_str()),
                Some(sku.display),
                "{}: zh_CN 展示名与表内原文不一致",
                sku.id
            );
            assert_eq!(
                map.get(&format!("sku.{}.note", sku.id)).map(|s| s.as_str()),
                Some(sku.note),
                "{}: zh_CN 判定依据与表内原文不一致",
                sku.id
            );
        }
    }

    /// 英文那份不能还是中文：漏翻一行，英文用户就会看到中文机型说明。
    #[test]
    fn english_texts_are_actually_translated() {
        let map = crate::i18n::parse_lang_text(crate::i18n::EMBED_EN);
        for sku in SKUS {
            for field in ["display", "note"] {
                let key = format!("sku.{}.{}", sku.id, field);
                let value = map.get(&key).expect("英文缺键");
                let han = value
                    .chars()
                    .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
                    .count();
                // 机型的中文产品名（枪神/魔霸新锐等）可以保留，整句都中文就不行
                assert!(
                    han * 10 < value.chars().count(),
                    "{key} 中文占比过高，像是没翻译：{value}"
                );
            }
        }
    }
}
