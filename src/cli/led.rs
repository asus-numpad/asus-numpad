//! `led`：直发背光命令字，排障专用。

use anyhow::Result;
use numdeck::config::Config;
use numdeck::i18n::{self};

use super::*;

/// `led`：绕过所有状态机，直接下发一个背光命令字。
///
/// 报「完全不亮」时必须先分清两件事：
/// * 驱动**没发**（服务没起来 / 通道不通 / 被跟随逻辑覆盖）——下发会报错，或根本没人发；
/// * 驱动发了但**硬件不认这个值**——`led on` 亮、`led 0x41` 不亮，就是档位字节的问题。
///
/// 所以留一条能直发 `0x01 / 0x00 / 0x41..0x48` 的通道，让现场一条命令就能定位：
/// `numdeck led on` → 亮，说明通道好的；`numdeck led 0x41` → 不亮，
/// 说明这台机器的 EC 只吃两态、不吃亮度档，那跟随亮度就得整体关掉。
pub(crate) fn cmd_led(cfg: &Config, value: Option<&str>) -> Result<i32> {
    let raw = value.map(|v| v.trim().to_ascii_lowercase());
    let level = match raw.as_deref() {
        None | Some("") | Some("status") | Some("help") => {
            println!("{}", i18n::t("led.cmd.usage"));
            return Ok(2);
        }
        Some("on") => 0x01,
        Some("off") => 0x00,
        Some(v) => {
            let parsed = match v.strip_prefix("0x") {
                Some(hex) => u8::from_str_radix(hex, 16).ok(),
                None => v.parse::<u8>().ok(),
            };
            match parsed {
                Some(b) => b,
                None => {
                    println!("{}", i18n::tf("led.cmd.bad_value", &[("v", v)]));
                    return Ok(2);
                }
            }
        }
    };
    let ok = apply_led_level(cfg, level);
    Ok(if ok { 0 } else { 1 })
}
