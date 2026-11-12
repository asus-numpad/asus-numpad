//! 自定义点亮报文模板。

use serde::{Deserialize, Serialize};

/// 报文模板里的「亮度字节」占位符。
pub const LEVEL_MARK: u8 = 0xff;

/// 点亮报文模板：`0xff` 换成实际亮度字节，其余字节原样下发。
///
/// 默认就是 NumberPad 的 FEATURE 报告 `0d 14 03 ff ad`。换成别的机型/别的
/// 触摸板时，照着它的 HID 报告描述符把固定部分抄进 `template`，把亮度所在
/// 位置写成 `255` 即可。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LedProtoSpec {
    pub template: Vec<u8>,
}

impl Default for LedProtoSpec {
    fn default() -> Self {
        Self {
            template: crate::led_proto::encode(0)
                .map(|b| {
                    let mut v = b.to_vec();
                    v[3] = LEVEL_MARK;
                    v
                })
                .unwrap_or_default(),
        }
    }
}

impl LedProtoSpec {
    /// 是否就是内置报文（是的话走原来的快路径，不多一次拼接）。
    pub fn is_builtin(&self) -> bool {
        self.template.len() == crate::led_proto::LED_FEATURE_LEN
            && self.template[..3]
                == [
                    crate::led_proto::LED_FEATURE_REPORT_ID,
                    crate::led_proto::LED_FEATURE_BODY[0],
                    crate::led_proto::LED_FEATURE_BODY[1],
                ]
            && self.template[3] == LEVEL_MARK
            && self.template[4] == crate::led_proto::LED_FEATURE_TAIL
    }

    /// 结构校验：长度、占位符个数、首字节（report id）。
    pub fn validate(&self) -> Result<(), String> {
        if self.template.is_empty() {
            return Err("template is empty".to_string());
        }
        if self.template.len() > 64 {
            return Err(format!(
                "template has {} bytes, max 64",
                self.template.len()
            ));
        }
        let marks = self.template.iter().filter(|b| **b == LEVEL_MARK).count();
        if marks != 1 {
            return Err(format!(
                "template needs exactly one {LEVEL_MARK} (level placeholder), found {marks}"
            ));
        }
        if self.template[0] == LEVEL_MARK {
            return Err("template byte 1 is the report id, cannot be the level".to_string());
        }
        Ok(())
    }

    /// 生成待写入报文；`0x60`（i2c 解锁字）在 FEATURE 通道无意义。
    pub fn encode(&self, value: u8) -> Option<Vec<u8>> {
        let level = crate::led_proto::level_byte(value)?;
        let mut out = self.template.clone();
        for b in out.iter_mut() {
            if *b == LEVEL_MARK {
                *b = level;
            }
        }
        Some(out)
    }
}
