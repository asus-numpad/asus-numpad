//! 高级参数：只在需要时追加的冷门开关。
/// 全局开关与高级选项。字段都是界面原样输入，拼装时才判空与校验。
#[derive(Default)]
pub struct Advanced {
    pub config: String,
    pub device: String,
    pub keyboard: String,
    pub led: String,
    pub hidraw: String,
    pub i2c_bus: String,
    pub i2c_addr: String,
    pub i2c_transfer: String,
    pub key_mode: String,
    pub layout: String,
    pub sku: String,
    pub numlock: String,
    pub mixed_mode: String,
    pub no_follow_brightness: bool,
    pub grab: bool,
    pub no_grab: bool,
    pub allow_unverified: bool,
    pub dump_config: bool,
    pub verbose: bool,
}

impl Advanced {
    pub(super) fn append(&self, argv: &mut Vec<String>) -> Result<(), String> {
        let mut push = |flag: &str, value: &str| {
            let value = value.trim();
            if !value.is_empty() {
                argv.push(flag.to_string());
                argv.push(value.to_string());
            }
        };
        push("--config", &self.config);
        push("--device", &self.device);
        push("--keyboard", &self.keyboard);
        push("--led", &self.led);
        push("--hidraw", &self.hidraw);
        push("--i2c-addr", &self.i2c_addr);
        push("--i2c-transfer", &self.i2c_transfer);
        push("--key-mode", &self.key_mode);
        push("--layout", &self.layout);
        push("--sku", &self.sku);
        push("--numlock", &self.numlock);
        push("--mixed-mode", &self.mixed_mode);

        if !self.i2c_bus.trim().is_empty() {
            let bus: i32 = self
                .i2c_bus
                .trim()
                .parse()
                .map_err(|_| format!("--i2c-bus 需要整数：{}", self.i2c_bus.trim()))?;
            argv.push("--i2c-bus".to_string());
            argv.push(bus.to_string());
        }

        for (on, flag) in [
            (self.no_follow_brightness, "--no-follow-brightness"),
            (self.allow_unverified, "--allow-unverified"),
            (self.dump_config, "--dump-config"),
            (self.verbose, "--verbose"),
        ] {
            if on {
                argv.push(flag.to_string());
            }
        }

        if self.grab && self.no_grab {
            return Err("--grab 与 --no-grab 互斥，只能勾一个".to_string());
        }
        if self.grab {
            argv.push("--grab".to_string());
        }
        if self.no_grab {
            argv.push("--no-grab".to_string());
        }
        Ok(())
    }
}
