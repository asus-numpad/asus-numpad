//! 配置片段：把四项扩展打包成一段 JSON，导出 / 复制 / 存盘 / 应用。

use numdeck::custom::{CustomLayoutSpec, HotkeySpec, LedProtoSpec, PressureSwitch};

/// 四项扩展打包成一段 JSON 文本（同机型对拷用）。
pub(crate) fn fragment_of(cfg: &numdeck::config::Config) -> String {
    let value = serde_json::json!({
        "custom_layouts": cfg.custom_layouts,
        "led_proto": cfg.led_proto,
        "pressure_switch": cfg.pressure_switch,
        "hotkeys": cfg.hotkeys,
    });
    serde_json::to_string_pretty(&value).unwrap_or_default()
}

/// 片段写回配置：缺的项保持原样，坏的那段直接报出是哪一项。
///
/// 返回值第二项为 `true` 表示出错。写进来的内容不做裁剪——真正能不能装，
/// 由保存时的 `sanitize` 统一判定，界面不替它猜。
pub(crate) fn apply_fragment(cfg: &mut numdeck::config::Config, text: &str) -> (String, bool) {
    let text = text.trim();
    if text.is_empty() {
        return (
            "片段是空的：先点「导出」，或粘贴别人的片段".to_string(),
            true,
        );
    }
    let value: serde_json::Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(err) => return (format!("不是合法 JSON：{err}"), true),
    };
    let mut touched: Vec<String> = Vec::new();
    // 先全部解析到局部，四项都过了再一次性写回：一段坏片段不该留下改了一半的配置。
    let mut layouts: Option<Vec<CustomLayoutSpec>> = None;
    let mut proto: Option<LedProtoSpec> = None;
    let mut pressure: Option<PressureSwitch> = None;
    let mut hotkeys: Option<Vec<HotkeySpec>> = None;

    if let Some(raw) = value.get("custom_layouts") {
        match serde_json::from_value::<Vec<CustomLayoutSpec>>(raw.clone()) {
            Ok(items) => {
                touched.push(format!("自定义布局 {} 条", items.len()));
                layouts = Some(items);
            }
            Err(err) => return (format!("custom_layouts 解析失败：{err}"), true),
        }
    }
    if let Some(raw) = value.get("led_proto") {
        match serde_json::from_value::<LedProtoSpec>(raw.clone()) {
            Ok(spec) => match spec.validate() {
                Ok(()) => {
                    touched.push("点亮报文".to_string());
                    proto = Some(spec);
                }
                Err(err) => return (format!("led_proto 模板不合法：{err}"), true),
            },
            Err(err) => return (format!("led_proto 解析失败：{err}"), true),
        }
    }
    if let Some(raw) = value.get("pressure_switch") {
        match serde_json::from_value::<PressureSwitch>(raw.clone()) {
            Ok(spec) => {
                touched.push("压感切换".to_string());
                pressure = Some(spec);
            }
            Err(err) => return (format!("pressure_switch 解析失败：{err}"), true),
        }
    }
    if let Some(raw) = value.get("hotkeys") {
        match serde_json::from_value::<Vec<HotkeySpec>>(raw.clone()) {
            Ok(items) => {
                touched.push(format!("热键 {} 条", items.len()));
                hotkeys = Some(items);
            }
            Err(err) => return (format!("hotkeys 解析失败：{err}"), true),
        }
    }

    if touched.is_empty() {
        return (
            "片段里没有这四项里的任何一项：需要 custom_layouts / led_proto / pressure_switch / hotkeys".to_string(),
            true,
        );
    }

    if let Some(items) = layouts {
        cfg.custom_layouts = items;
    }
    if let Some(spec) = proto {
        cfg.led_proto = spec;
    }
    if let Some(spec) = pressure {
        cfg.pressure_switch = spec;
    }
    if let Some(items) = hotkeys {
        cfg.hotkeys = items;
    }
    (
        format!("已应用 {}；记得点「保存」落盘", touched.join("、")),
        false,
    )
}

/// 在给定目录里挑一个没被占用的文件名写下去。
pub(crate) fn save_snippet_in(
    dir: &std::path::Path,
    text: &str,
) -> Result<std::path::PathBuf, String> {
    if text.trim().is_empty() {
        return Err("片段是空的，没什么可写".to_string());
    }
    for round in 1..100 {
        let name = if round == 1 {
            "numdeck-extend.json".to_string()
        } else {
            format!("numdeck-extend-{round}.json")
        };
        let path = dir.join(name);
        if path.exists() {
            continue;
        }
        std::fs::write(&path, text).map_err(|err| format!("写不进去：{err}"))?;
        return Ok(path);
    }
    Err("同名文件太多了，先去下载目录清一下".to_string())
}
