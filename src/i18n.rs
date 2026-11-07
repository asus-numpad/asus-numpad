//! 多语言：内置 简体中文 / 繁體中文 / English 三语，可被外部 `.lang` 文件覆盖。
//!
//! 设计要点：
//!
//! * **内置即兜底**：三份语言文件用 `include_str!` 编进二进制，即使 `/usr/share` 下的
//!   语言文件被删光也能正常输出，不会出现空白提示。
//! * **可翻译**：安装到 `/usr/share/asus-numpad/lang/*.lang`，用户可整份替换或只放
//!   自己关心的键；`/etc/asus-numpad/lang/` 与 `$ASUS_NUMPAD_LANG_DIR` 优先级更高，
//!   便于本地化翻译不碰包里文件。
//! * **键值格式**：`key = 文案`，`#` 开头为注释，值里支持 `\n` 换行转义。
//!   缺键时按「当前语言文件 → 该语言内置 → 英文内置 → 键名」逐级回落。
//! * **切换**：`asus-numpad lang <code|auto>` 写入配置；`--lang` 与
//!   `$ASUS_NUMPAD_LANG` 可临时覆盖，不改配置。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 内置语言文件（与安装到 `/usr/share/asus-numpad/lang/` 的是同一份）
pub const EMBED_ZH_CN: &str = include_str!("../lang/zh_CN.lang");
pub const EMBED_ZH_TW: &str = include_str!("../lang/zh_TW.lang");
pub const EMBED_EN: &str = include_str!("../lang/en.lang");

/// 临时覆盖语言的环变量（优先级最高，仅影响本次进程）
pub const LANG_ENV: &str = "ASUS_NUMPAD_LANG";
/// 追加语言文件搜索目录
pub const LANG_DIR_ENV: &str = "ASUS_NUMPAD_LANG_DIR";

/// 语言文件的系统搜索目录（前者优先，便于本机覆盖）
///
/// 用户级目录 `~/.config/asus-numpad/lang` 由 [`user_lang_dir`] 动态拼出，
/// 排在系统目录之前——普通用户不写 `/etc` 也能覆盖文案。
pub const LANG_DIRS: [&str; 2] = ["/etc/asus-numpad/lang", "/usr/share/asus-numpad/lang"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 简体中文
    ZhCn,
    /// 繁體中文
    ZhTw,
    /// English
    En,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::ZhCn, Lang::ZhTw, Lang::En];

    /// 配置 / 命令行里使用的语言代码
    pub fn code(self) -> &'static str {
        match self {
            Lang::ZhCn => "zh-CN",
            Lang::ZhTw => "zh-TW",
            Lang::En => "en",
        }
    }

    /// 语言文件名
    pub fn file_name(self) -> &'static str {
        match self {
            Lang::ZhCn => "zh_CN.lang",
            Lang::ZhTw => "zh_TW.lang",
            Lang::En => "en.lang",
        }
    }

    /// 该语言自己的写法（列可用语言时展示）
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::ZhCn => "简体中文",
            Lang::ZhTw => "繁體中文",
            Lang::En => "English",
        }
    }

    pub fn embedded(self) -> &'static str {
        match self {
            Lang::ZhCn => EMBED_ZH_CN,
            Lang::ZhTw => EMBED_ZH_TW,
            Lang::En => EMBED_EN,
        }
    }

    /// 宽松解析用户输入：`zh-CN` / `zh_CN` / `zhcn` / `zh-Hans` / `cn` / `chinese` 等。
    pub fn parse(text: &str) -> Option<Lang> {
        let t = text.trim().to_ascii_lowercase().replace('_', "-");
        match t.as_str() {
            "" | "auto" | "system" | "default" => None,
            "zh-cn" | "zhcn" | "zh-hans" | "cn" | "chs" | "chinese" | "simplified" | "zh"
            | "zh-sg" => Some(Lang::ZhCn),
            "zh-tw" | "zhtw" | "zh-hant" | "tw" | "cht" | "hk" | "zh-hk" | "zh-mo"
            | "traditional" => Some(Lang::ZhTw),
            "en" | "en-us" | "en-gb" | "english" | "eng" => Some(Lang::En),
            _ => None,
        }
    }

    /// 由 locale 推断：`zh_TW.UTF-8` → 繁體，`zh_CN.UTF-8` → 简体，`C`/`POSIX`/其它 → English。
    pub fn from_locale(locale: &str) -> Lang {
        let l = locale.trim().to_ascii_lowercase();
        if l.is_empty() {
            return Lang::En;
        }
        let l = l.split('.').next().unwrap_or("").replace('_', "-");
        if l.starts_with("zh") {
            if l.contains("tw") || l.contains("hk") || l.contains("mo") || l.contains("hant") {
                Lang::ZhTw
            } else {
                Lang::ZhCn
            }
        } else {
            Lang::En
        }
    }
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// 系统 locale 推断的语言（`LC_ALL` > `LC_MESSAGES` > `LANG` > `LANGUAGE`）。
pub fn detect_system() -> Lang {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(v) = env_nonempty(key) {
            return Lang::from_locale(&v);
        }
    }
    if let Some(v) = env_nonempty("LANGUAGE") {
        // LANGUAGE 可能是 "zh_TW:zh_CN"，取第一个
        return Lang::from_locale(v.split(':').next().unwrap_or(""));
    }
    Lang::En
}

/// 环境变量 `ASUS_NUMPAD_LANG` 指定的语言（无则 None）。
pub fn detect_env_override() -> Option<Lang> {
    env_nonempty(LANG_ENV).and_then(|v| Lang::parse(&v))
}

/// 语言文件搜索目录，按优先级从高到低。
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = env_nonempty(LANG_DIR_ENV) {
        dirs.push(PathBuf::from(d));
    }
    // 用户级覆盖（免 root）：~/.config/asus-numpad/lang，写自己的家目录就能改文案
    if let Some(d) = user_lang_dir() {
        dirs.push(d);
    }
    for d in LANG_DIRS {
        dirs.push(PathBuf::from(d));
    }
    dirs
}

/// 用户级语言目录：`$XDG_CONFIG_HOME/asus-numpad/lang` 或 `~/.config/asus-numpad/lang`。
pub fn user_lang_dir() -> Option<PathBuf> {
    let base = match env_nonempty("XDG_CONFIG_HOME") {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(env_nonempty("HOME")?).join(".config"),
    };
    Some(base.join("asus-numpad").join("lang"))
}

/// 解析 `.lang` 文本：`key = value`，`#` 注释，空行忽略，值支持 `\n` / `\t` 转义。
pub fn parse_lang_text(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        if key.is_empty() {
            continue;
        }
        let value = unescape(v.trim());
        map.insert(key.to_string(), value);
    }
    map
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            // `, \s` 这类需要保留尾随空格的文案（值会先被 trim）
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 一套生效文案：外部文件覆盖内置，缺键回落到英文内置。
#[derive(Debug, Clone)]
pub struct Catalog {
    lang: Lang,
    map: HashMap<String, String>,
    en: HashMap<String, String>,
    source: Option<PathBuf>,
}

impl Catalog {
    /// 加载某个语言：内置为基础，再按 `search_dirs()` 顺序覆盖（高优先级目录先覆盖）。
    pub fn load(lang: Lang) -> Catalog {
        let mut map = parse_lang_text(lang.embedded());
        let mut source = None;
        // 低优先级在前、高优先级在后覆盖，所以倒序合并
        for dir in search_dirs().into_iter().rev() {
            let path = dir.join(lang.file_name());
            if let Ok(text) = std::fs::read_to_string(&path) {
                for (k, v) in parse_lang_text(&text) {
                    map.insert(k, v);
                }
                source = Some(path);
            }
        }
        Catalog {
            lang,
            map,
            en: parse_lang_text(EMBED_EN),
            source,
        }
    }

    /// 纯离线构造（测试用，不读文件系统）。
    pub fn from_text(lang: Lang, text: &str) -> Catalog {
        Catalog {
            lang,
            map: parse_lang_text(text),
            en: parse_lang_text(EMBED_EN),
            source: None,
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    /// 取一条文案：当前语言 → 英文内置 → 键名本身。
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        if let Some(v) = self.map.get(key) {
            return v;
        }
        if let Some(v) = self.en.get(key) {
            return v;
        }
        key
    }

    /// 已加载的键数量（自检用）
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

static GLOBAL: OnceLock<Catalog> = OnceLock::new();

/// 设定生效语言（进程内只生效一次，重复调用被忽略）。
pub fn init(lang: Lang) {
    let _ = GLOBAL.set(Catalog::load(lang));
}

/// 当前语言；未初始化时按系统 locale 现算（保证 `t()` 随时可用）。
pub fn current() -> Lang {
    match GLOBAL.get() {
        Some(c) => c.lang(),
        None => detect_system(),
    }
}

/// 当前语言文件来自哪个路径（None = 用内置文案）。
pub fn current_source() -> Option<PathBuf> {
    GLOBAL
        .get()
        .and_then(|c| c.source().map(|p| p.to_path_buf()))
}

fn catalog() -> &'static Catalog {
    GLOBAL.get_or_init(|| Catalog::load(detect_system()))
}

/// 取文案（缺失回落英文，再回落键名）。
pub fn t(key: &str) -> String {
    catalog().get(key).to_string()
}

/// 取文案，键不存在时用调用方给的原文兜底。
///
/// 用于「文案本体由代码里的数据表带着、只有译文才放进语言文件」的场合：
/// 例如机型展示名与判定依据，中文原文写在 `sku.rs` 的表里，
/// `lang/*.lang` 只补译文；某份语言文件没翻译时应当显示原文，而不是裸键名。
pub fn t_or(key: &str, fallback: &str) -> String {
    let v = catalog().get(key);
    if v == key {
        fallback.to_string()
    } else {
        v.to_string()
    }
}

/// 取文案并替换 `{name}` 占位符。
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut text = t(key);
    for (name, value) in args {
        text = text.replace(&format!("{{{name}}}"), value);
    }
    text
}

/// 语言来源，用于 `lang` 子命令交代清楚「为什么现在是这个语言」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangSource {
    Env,
    Cli,
    Config,
    Locale,
    Default,
}

impl LangSource {
    pub fn key(self) -> &'static str {
        match self {
            LangSource::Env => "lang.source.env",
            LangSource::Cli => "lang.source.cli",
            LangSource::Config => "lang.source.config",
            LangSource::Locale => "lang.source.locale",
            LangSource::Default => "lang.source.default",
        }
    }
}

/// 按优先级决定语言：环境变量 > 命令行 > 配置 > 系统 locale > 英文。
///
/// `cfg_lang` 为空或 `auto` 时视为未指定，`cli_lang` 同理。
pub fn resolve(cfg_lang: &str, cli_lang: Option<&str>) -> (Lang, LangSource) {
    if let Some(lang) = detect_env_override() {
        return (lang, LangSource::Env);
    }
    if let Some(raw) = cli_lang {
        if let Some(lang) = Lang::parse(raw) {
            return (lang, LangSource::Cli);
        }
    }
    let cfg = cfg_lang.trim();
    if !cfg.is_empty() && !cfg.eq_ignore_ascii_case("auto") {
        if let Some(lang) = Lang::parse(cfg) {
            return (lang, LangSource::Config);
        }
    }
    if env_nonempty("LC_ALL").is_some()
        || env_nonempty("LC_MESSAGES").is_some()
        || env_nonempty("LANG").is_some()
        || env_nonempty("LANGUAGE").is_some()
    {
        return (detect_system(), LangSource::Locale);
    }
    (Lang::En, LangSource::Default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_common_spellings() {
        assert_eq!(Lang::parse("zh-CN"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh_CN.UTF-8"), None); // 带后缀的请走 from_locale
        assert_eq!(Lang::parse("zhcn"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh-Hans"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh-TW"), Some(Lang::ZhTw));
        assert_eq!(Lang::parse("zh_Hant"), Some(Lang::ZhTw));
        assert_eq!(Lang::parse("english"), Some(Lang::En));
        assert_eq!(Lang::parse("auto"), None);
        assert_eq!(Lang::parse("klingon"), None);
    }

    #[test]
    fn from_locale_maps_chinese_variants() {
        assert_eq!(Lang::from_locale("zh_CN.UTF-8"), Lang::ZhCn);
        assert_eq!(Lang::from_locale("zh_SG.utf8"), Lang::ZhCn);
        assert_eq!(Lang::from_locale("zh_TW.UTF-8"), Lang::ZhTw);
        assert_eq!(Lang::from_locale("zh_HK"), Lang::ZhTw);
        assert_eq!(Lang::from_locale("zh_Hant"), Lang::ZhTw);
        assert_eq!(Lang::from_locale("C"), Lang::En);
        assert_eq!(Lang::from_locale("POSIX"), Lang::En);
        assert_eq!(Lang::from_locale("en_US.UTF-8"), Lang::En);
        assert_eq!(Lang::from_locale(""), Lang::En);
    }

    #[test]
    fn lang_text_parses_comments_escapes_and_equals_in_value() {
        let m = parse_lang_text("# 注释\n\na = 一\nb= 二=三\nc = 上\\n下\n");
        assert_eq!(m.get("a").map(String::as_str), Some("一"));
        assert_eq!(m.get("b").map(String::as_str), Some("二=三"));
        assert_eq!(m.get("c").map(String::as_str), Some("上\n下"));
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn missing_key_falls_back_to_english_then_key() {
        let c = Catalog::from_text(Lang::ZhTw, "only.here = 只有這一條\n");
        assert_eq!(c.get("only.here"), "只有這一條");
        // 英文内置里有 bright.follow_on，繁体缺这条时应回落到英文原文（不是键名）
        let en = Catalog::from_text(Lang::En, "");
        assert_eq!(c.get("bright.follow_on"), en.get("bright.follow_on"));
        assert_ne!(c.get("bright.follow_on"), "bright.follow_on");
        assert_eq!(c.get("no.such.key.at.all"), "no.such.key.at.all");
    }

    #[test]
    fn three_languages_share_the_same_key_set() {
        // 三语键集必须一致：漏翻会在这里直接失败，而不是等用户跑到那一步才看见英文/键名
        let cn = parse_lang_text(EMBED_ZH_CN);
        let tw = parse_lang_text(EMBED_ZH_TW);
        let en = parse_lang_text(EMBED_EN);
        let mut missing_in_tw: Vec<&String> = cn.keys().filter(|k| !tw.contains_key(*k)).collect();
        let mut missing_in_cn: Vec<&String> = tw.keys().filter(|k| !cn.contains_key(*k)).collect();
        let mut missing_in_en: Vec<&String> = cn.keys().filter(|k| !en.contains_key(*k)).collect();
        missing_in_tw.sort();
        missing_in_cn.sort();
        missing_in_en.sort();
        assert!(missing_in_tw.is_empty(), "繁体缺键: {missing_in_tw:?}");
        assert!(missing_in_cn.is_empty(), "简体缺键: {missing_in_cn:?}");
        assert!(missing_in_en.is_empty(), "英文缺键: {missing_in_en:?}");
    }

    #[test]
    fn no_empty_values_in_bundled_catalogs() {
        for lang in Lang::ALL {
            for (k, v) in parse_lang_text(lang.embedded()) {
                assert!(!v.trim().is_empty(), "{} 的 {k} 是空文案", lang.code());
            }
        }
    }

    #[test]
    fn placeholders_are_consistent_across_languages() {
        // 占位符集合必须一致，否则切换语言会出现 {path} 之类没被替换的残渣
        fn holes(text: &str) -> Vec<String> {
            let mut out = Vec::new();
            let mut rest = text;
            while let Some(i) = rest.find('{') {
                let tail = &rest[i + 1..];
                match tail.find('}') {
                    Some(j) => {
                        out.push(tail[..j].to_string());
                        rest = &tail[j + 1..];
                    }
                    None => break,
                }
            }
            out.sort();
            out
        }
        let cn = parse_lang_text(EMBED_ZH_CN);
        let tw = parse_lang_text(EMBED_ZH_TW);
        let en = parse_lang_text(EMBED_EN);
        for (k, v) in &cn {
            let h = holes(v);
            if h.is_empty() {
                continue;
            }
            assert_eq!(holes(&tw[k]), h, "{k} 繁体占位符不一致");
            assert_eq!(holes(&en[k]), h, "{k} 英文占位符不一致");
        }
    }

    #[test]
    fn tf_replaces_named_placeholders() {
        let text = tf("bright.status_dir", &[("dir", "/sys/x"), ("ms", "500")]);
        assert!(text.contains("/sys/x"), "{text}");
        assert!(text.contains("500"), "{text}");
        assert!(!text.contains("{dir}"), "{text}");
    }
}
