//! `run`：拉起驱动主循环。

use anyhow::Result;
use numdeck::banner;
use numdeck::config::Config;
use numdeck::i18n::{self};

// 闸门与亮度摘要都在别的子模块里，`super::*` 不会把它们带进来；
// 显式导入并加 cfg，非 Linux 上不产生未使用告警。
#[cfg(target_os = "linux")]
use super::bright::brightness_report;
#[cfg(target_os = "linux")]
use super::sku::{numpad_gate, sku_gate};

pub(crate) fn cmd_run(cfg: Config, verbose: bool) -> Result<i32> {
    // 两道闸门只对 Linux 有意义：非 Linux 上读不到 DMI 与 hidraw，跑闸门只会
    // 得到「不在支持列表」，把「这个平台跑不了」误报成「这台机器不支持」。
    #[cfg(target_os = "linux")]
    {
        if let Some(code) = sku_gate(&cfg) {
            return Ok(code);
        }
        if let Some(code) = numpad_gate() {
            return Ok(code);
        }
    }
    #[cfg(target_os = "linux")]
    {
        println!("{}", banner());
        println!("{}", cfg.summary());
        println!(
            "{}",
            i18n::tf("run.brightness", &[("text", &brightness_report(&cfg))])
        );
        match numdeck::linux::run_service(cfg, verbose)? {
            // 75 = EX_TEMPFAIL：这次是「有意重来」，配合 Restart=on-failure 重新拉起，
            // 新配置随之生效。返回 0 会让 systemd 认为已正常结束，服务就停在那儿了。
            numdeck::linux::ServiceExit::Reload => {
                println!("{}", i18n::t("run.reloading"));
                Ok(75)
            }
            // 锁没拿到：多半是上一个进程还在收尾。75 让 systemd 隔几秒再试一次，
            // 直接返回 0 会让单元停在 inactive（用户看到「服务没起来」）。
            numdeck::linux::ServiceExit::Busy => Ok(75),
            numdeck::linux::ServiceExit::Stopped => Ok(0),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (cfg, verbose);
        println!("{}", banner());
        println!("{}", i18n::t("run.nonlinux"));
        println!("{}", i18n::t("run.nonlinux_hint"));
        Ok(1)
    }
}
