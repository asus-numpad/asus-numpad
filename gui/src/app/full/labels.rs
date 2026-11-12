//! 枚举到界面文案的对照（只读，不参与布局）。

use super::*;

pub(super) fn support_label(support: LayoutSupport) -> &'static str {
    match support {
        LayoutSupport::Verified => "真机核对",
        LayoutSupport::Family => "同族未核对",
        LayoutSupport::Unsupported => "未核对、默认不放行",
    }
}

pub(super) fn source_label(source: sku::SkuSource) -> &'static str {
    match source {
        sku::SkuSource::Manual => "手动指定",
        sku::SkuSource::Dmi => "DMI 机型",
        sku::SkuSource::Hardware => "触摸板模组",
        // ARM64 构建不带机型表，机型恒为自定义布局
        sku::SkuSource::Custom => "自定义布局（无机型表）",
    }
}

pub(super) fn numlock_label(mode: NumLockMode) -> &'static str {
    match mode {
        NumLockMode::Sync => "sync：关闭时补一次按键再点灯",
        NumLockMode::Toggle => "toggle：无条件切换 NumLock",
        NumLockMode::Off => "off：完全不碰 NumLock",
    }
}

pub(super) fn mixed_label(mode: MixedMode) -> &'static str {
    match mode {
        MixedMode::Auto => "auto：有虚拟触摸板就开（官方默认）",
        MixedMode::On => "on：强制开启",
        MixedMode::Off => "off：纯数字键盘",
    }
}

pub(super) fn lang_label(code: &str) -> &'static str {
    match code {
        "auto" => "auto：跟随系统 locale",
        "zh-CN" => "zh-CN 简体中文",
        "zh-TW" => "zh-TW 繁體中文",
        "en" => "en English",
        _ => "auto：跟随系统 locale",
    }
}
