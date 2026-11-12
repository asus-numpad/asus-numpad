//! 子命令实现。`main.rs` 只留参数解析与分派，每个命令具体干什么在这里。

mod util;
pub(crate) use util::*;
mod args;
pub(crate) use args::*;
mod check;
pub(crate) use check::*;
mod levels;
pub(crate) use levels::*;
mod bright;
pub(crate) use bright::*;
mod led;
pub(crate) use led::*;
mod lang;
pub(crate) use lang::*;
mod layout;
pub(crate) use layout::*;
mod sku;
pub(crate) use sku::*;
mod run;
pub(crate) use run::*;
mod calibrate;
pub(crate) use calibrate::*;
mod dump;
pub(crate) use dump::*;
