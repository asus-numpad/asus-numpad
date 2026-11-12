//! 点亮报文模板的解析（十六进制字节串）。

/// 解析十六进制字节串：接受空格 / 逗号 / `0x` 前缀与连写（`0d1403`）。
pub(crate) fn parse_hex(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text
        .replace("0x", " ")
        .replace("0X", " ")
        .replace([',', ':'], " ");
    let mut out = Vec::new();
    for token in cleaned.split_whitespace() {
        if token.len() % 2 != 0 {
            return Err(format!("`{token}` 是奇数个十六进制字符"));
        }
        for pair in token.as_bytes().chunks(2) {
            let pair = std::str::from_utf8(pair).map_err(|_| "非 ASCII".to_string())?;
            let byte =
                u8::from_str_radix(pair, 16).map_err(|_| format!("`{pair}` 不是十六进制"))?;
            out.push(byte);
        }
    }
    if out.is_empty() {
        return Err("模板不能为空".to_string());
    }
    Ok(out)
}
