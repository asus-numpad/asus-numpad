//! 开机自启（Windows）。
//!
//! 用 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 下一个字符串值，
//! 不需要管理员权限，也不需要服务/计划任务。值名固定为 ASCII，便于脚本检查。
//!
//! Linux 不走这里：那边数字键与手势本来就有 systemd 用户单元，界面的「开机自启」
//! 直接 `systemctl --user enable numdeck`（见 [`crate::sysinfo`]）。

use anyhow::Result;

/// 注册表值名（ASCII，避免某些系统工具读不了中文值名）。
#[allow(dead_code)] // 非 Windows 分支不读它，但 README 与卸载脚本按这个名字说明
pub const VALUE_NAME: &str = "AsusNumpadGui";

/// 当前平台是否支持。
pub fn supported() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
mod imp {
    use super::VALUE_NAME;
    use anyhow::{anyhow, Result};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn open_run_key(write: bool) -> Result<Key> {
        let access = if write { KEY_WRITE } else { KEY_READ };
        let mut hkey = HKEY::default();
        let rc = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN_KEY,
                0,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                access,
                None,
                &mut hkey,
                None,
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(anyhow!("打开 Run 注册表项失败：{}", rc.0));
        }
        Ok(Key(hkey))
    }

    fn check(rc: WIN32_ERROR, what: &str) -> Result<()> {
        if rc == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(anyhow!("{what} 失败：{}", rc.0))
        }
    }

    /// 当前是否已登记自启。
    pub fn enabled() -> bool {
        let Ok(key) = open_run_key(false) else {
            return false;
        };
        let name: Vec<u16> = VALUE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let rc = unsafe { RegQueryValueExW(key.0, PCWSTR(name.as_ptr()), None, None, None, None) };
        rc == ERROR_SUCCESS
    }

    /// 写入或删除自启项；`cmd` 为要执行的命令行（通常是当前 exe 的绝对路径）。
    pub fn set(on: bool, cmd: &str) -> Result<()> {
        let key = open_run_key(true)?;
        let name: Vec<u16> = VALUE_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let name = PCWSTR(name.as_ptr());

        if on {
            // REG_SZ 数据要带结尾 NUL，按字节传给注册表
            let data: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes: Vec<u8> = data.iter().flat_map(|u| u.to_le_bytes()).collect();
            let rc = unsafe { RegSetValueExW(key.0, name, 0, REG_SZ, Some(&bytes)) };
            check(rc, "写入自启项")
        } else {
            let rc = unsafe { RegDeleteValueW(key.0, name) };
            if rc == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            check(rc, "删除自启项")
        }
    }
}

/// 当前是否已登记自启；非 Windows 恒为 false。
pub fn enabled() -> bool {
    #[cfg(windows)]
    {
        imp::enabled()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 设置自启；非 Windows 直接返回 `Ok(())`，由界面自己隐藏该选项。
pub fn set(on: bool, cmd: &str) -> Result<()> {
    #[cfg(windows)]
    {
        imp::set(on, cmd)
    }
    #[cfg(not(windows))]
    {
        let _ = (on, cmd);
        Ok(())
    }
}

/// 自启时应执行的命令行：`"<exe>" --apply-startup`
pub fn command_line() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "numdeck-gui".to_string());
    format!("\"{exe}\" --apply-startup")
}
