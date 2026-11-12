//! 各页渲染。按左侧导航一页一个模块，页面之间不互相调用，公共部分走
//! `crate::app::widgets` 与 `crate::theme`。

pub(super) mod command;
pub(super) mod device;
pub(super) mod light;
pub(super) mod settings;
pub(super) mod system;
