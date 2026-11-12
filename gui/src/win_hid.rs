//! Windows HID 后端：只用系统自带的 `setupapi` + `hid.dll`。
//!
//! 三件事：
//!
//! 1. 用 `SetupDiGetClassDevsW(HID)` 枚举全部 HID 设备接口；
//! 2. 对每个接口打开句柄，读 `HIDD_ATTRIBUTES`（VID/PID）、产品字符串、
//!    `HIDP_CAPS`，再用 `HidP_GetValueCaps` 确认它是否有 FEATURE 报告 `0x0d`；
//! 3. 写灯光时重新 `CreateFileW` → `HidD_SetFeature` → `CloseHandle`。
//!
//! 不装内核驱动、不注册设备接口、不读输入报告：数字键与手势始终由 ASUS 官方
//! 驱动处理，本程序只往那条 FEATURE 通道写 5 个字节。
//!
//! 句柄每次用完即关，理由与 Linux 侧一致：某些机型的 EC 通路在同一时刻只允许
//! 一个持有者，长期占用会让 MyASUS / 官方驱动自己的调光失效。

use anyhow::{anyhow, Context, Result};
use std::mem::size_of;
use std::ptr;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW, DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO,
    SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetPreparsedData, HidD_GetProductString,
    HidD_SetFeature, HidP_Feature, HidP_GetCaps, HidP_GetValueCaps, GUID_DEVINTERFACE_HID,
    HIDD_ATTRIBUTES, HIDP_CAPS, HIDP_STATUS_SUCCESS, HIDP_VALUE_CAPS, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, HWND};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_NUMLOCK};

use numdeck::led_proto::LED_FEATURE_REPORT_ID;

use crate::backend::{mark_best, Backend, DeviceInfo};

const FEATURE_REPORT_ID: u8 = LED_FEATURE_REPORT_ID;

/// 句柄守卫：任何路径返回都会关掉，避免泄漏。
struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 打开设备接口路径；读写都要，`HidD_SetFeature` 本身只需要写，
/// 但读属性/描述符要读权限，合在一次打开里省一次句柄。
fn open_path(path: &str) -> Result<OwnedHandle> {
    let name = wide(path);
    let handle = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            (GENERIC_READ | GENERIC_WRITE).0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            HANDLE::default(),
        )
    }
    .with_context(|| format!("打开设备接口失败：{path}"))?;
    Ok(OwnedHandle(handle))
}

/// 取第 `index` 个 HID 接口的路径；枚举结束时返回 `Ok(None)`。
fn interface_path(hdev: HDEVINFO, index: u32) -> Result<Option<String>> {
    let mut did = SP_DEVICE_INTERFACE_DATA {
        cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
        ..Default::default()
    };
    let guid: GUID = GUID_DEVINTERFACE_HID;
    if unsafe { SetupDiEnumDeviceInterfaces(hdev, None, &guid, index, &mut did) }.is_err() {
        // ERROR_NO_MORE_ITEMS 也走这里：枚举到头了
        return Ok(None);
    }

    let mut required = 0u32;
    let _ =
        unsafe { SetupDiGetDeviceInterfaceDetailW(hdev, &did, None, 0, Some(&mut required), None) };
    if required == 0 {
        return Ok(None);
    }

    let mut buf = vec![0u8; required as usize];
    let detail = buf.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    unsafe {
        (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
    }
    unsafe {
        SetupDiGetDeviceInterfaceDetailW(hdev, &did, Some(detail), required, None, None)
            .context("读取设备接口路径失败")?;
    }

    let path_offset = std::mem::offset_of!(SP_DEVICE_INTERFACE_DETAIL_DATA_W, DevicePath);
    let chars = (required as usize - path_offset) / size_of::<u16>();
    let path_ptr = unsafe { ptr::addr_of!((*detail).DevicePath) as *const u16 };
    let path_slice = unsafe { std::slice::from_raw_parts(path_ptr, chars) };
    Ok(Some(from_wide(path_slice)))
}

/// 读一个接口的「身份 + 能力」。
fn probe(handle: HANDLE) -> Option<(HIDD_ATTRIBUTES, String, bool, u16)> {
    let mut attrs = HIDD_ATTRIBUTES {
        Size: size_of::<HIDD_ATTRIBUTES>() as u32,
        ..Default::default()
    };
    if unsafe { HidD_GetAttributes(handle, &mut attrs) } == false {
        return None;
    }

    let mut product = [0u16; 128];
    let product = if unsafe {
        HidD_GetProductString(
            handle,
            product.as_mut_ptr() as *mut _,
            (product.len() * 2) as u32,
        )
    } == false
    {
        String::new()
    } else {
        from_wide(&product)
    };

    let (has_feature, feature_len) = feature_caps(handle).unwrap_or((false, 0));
    Some((attrs, product, has_feature, feature_len))
}

/// 读 `HIDP_CAPS` 并确认存在 FEATURE 报告 `0x0d`。
fn feature_caps(handle: HANDLE) -> Option<(bool, u16)> {
    let mut preparsed = PHIDP_PREPARSED_DATA::default();
    if unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } == false {
        return None;
    }
    let _guard = OwnedPreparsed(preparsed);

    let mut caps = HIDP_CAPS::default();
    if unsafe { HidP_GetCaps(preparsed, &mut caps) } != HIDP_STATUS_SUCCESS {
        return None;
    }

    let mut value_caps: [HIDP_VALUE_CAPS; 128] = unsafe { std::mem::zeroed() };
    let mut caps_len = value_caps.len() as u16;
    let status = unsafe {
        HidP_GetValueCaps(
            HidP_Feature,
            value_caps.as_mut_ptr(),
            &mut caps_len,
            preparsed,
        )
    };
    if status != HIDP_STATUS_SUCCESS {
        // 拿不到 value caps 时退一步：只要 FEATURE 报告足够长就认为可写，
        // 真正的可用性由写报文时的返回值决定。
        return Some((
            caps.FeatureReportByteLength as usize >= numdeck::led_proto::LED_FEATURE_LEN,
            caps.FeatureReportByteLength,
        ));
    }

    let has = value_caps[..caps_len as usize]
        .iter()
        .any(|c| c.ReportID == FEATURE_REPORT_ID);
    Some((has, caps.FeatureReportByteLength))
}

struct OwnedPreparsed(PHIDP_PREPARSED_DATA);

impl Drop for OwnedPreparsed {
    fn drop(&mut self) {
        unsafe {
            let _ = HidD_FreePreparsedData(self.0);
        }
    }
}

/// 基于系统 HID 接口的后端。
pub struct WindowsHid {
    /// 上一次扫描留下的接口数，仅用于界面展示
    pub last_count: usize,
}

impl Default for WindowsHid {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsHid {
    pub fn new() -> Self {
        Self { last_count: 0 }
    }
}

impl Backend for WindowsHid {
    fn name(&self) -> &'static str {
        "Windows HID（setupapi + hid.dll）"
    }

    fn is_real(&self) -> bool {
        true
    }

    fn scan(&mut self) -> Result<Vec<DeviceInfo>> {
        let guid: GUID = GUID_DEVINTERFACE_HID;
        let hdev = unsafe {
            SetupDiGetClassDevsW(
                Some(&guid),
                PCWSTR::null(),
                HWND::default(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            )
        }
        .context("枚举 HID 设备接口失败")?;
        let _guard = DevInfoSet(hdev);

        let mut devices = Vec::new();
        let mut index = 0u32;
        while let Some(path) = interface_path(hdev, index)? {
            index += 1;
            let handle = match open_path(&path) {
                Ok(h) => h,
                // 单个接口打不开（被独占 / 权限）不影响整体枚举
                Err(_) => continue,
            };
            let Some((attrs, product, has_feature, feature_len)) = probe(handle.0) else {
                continue;
            };
            devices.push(DeviceInfo {
                path,
                vid: attrs.VendorID,
                pid: attrs.ProductID,
                product,
                has_feature,
                feature_len,
                rank: None,
                selected: false,
            });
        }

        mark_best(&mut devices);
        self.last_count = devices.len();
        Ok(devices)
    }

    fn write(
        &mut self,
        device: &DeviceInfo,
        frame: [u8; numdeck::led_proto::LED_FEATURE_LEN],
    ) -> Result<()> {
        // 每次重开句柄：不让本程序长期占住 EC 通路
        let handle = open_path(&device.path)?;
        // FEATURE 报文首字节必须是 report id，这里 frame[0] 就是 0x0d
        let ok = unsafe {
            HidD_SetFeature(
                handle.0,
                frame.as_ptr() as *const core::ffi::c_void,
                frame.len() as u32,
            )
        };
        if ok == false {
            return Err(anyhow!(
                "HidD_SetFeature 被拒绝（接口 {}）",
                device.vid_pid()
            ));
        }
        Ok(())
    }

    fn numlock(&mut self) -> Option<bool> {
        let state = unsafe { GetKeyState(VK_NUMLOCK.0 as i32) };
        Some(state & 1 != 0)
    }
}

struct DevInfoSet(HDEVINFO);

impl Drop for DevInfoSet {
    fn drop(&mut self) {
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_roundtrip() {
        let w = wide(r"\\?\hid#vid_04f3&pid_3275#col01");
        assert_eq!(w.last(), Some(&0));
        assert!(from_wide(&w).starts_with(r"\\?\hid#"));
    }

    #[test]
    fn feature_report_id_is_the_shared_one() {
        // 编译期就保证 Windows 侧用的是 led_proto 的同一个 ID
        assert_eq!(FEATURE_REPORT_ID, 0x0d);
    }
}
