//! 直接基于 libc 的 evdev 访问（不依赖 evdev crate）。
//!
//! 只用到内核稳定 ABI：`ioctl` 的 `EVIOC*`、`read()` 的 `struct input_event`、
//! uinput 的 `UI_*`。少一个第三方依赖，打包时少一处版本漂移风险。

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::io::RawFd;

use crate::engine::Event;
use crate::keycodes as kc;

/// 一次 `read()` 最多取多少个事件：24 字节 × 64 = 1536 字节，够装一次手势里
/// 所有手指的 MT 报文，又不至于让栈缓冲过大。
pub(crate) const MAX_EVENTS_PER_READ: usize = 64;

/// 一次 `read()` 的目标缓冲字节数。
const EVENT_BUF_BYTES: usize = std::mem::size_of::<RawInputEvent>() * MAX_EVENTS_PER_READ;

/// 单次 `read_events_into` 最多连续读几块事件，避免持续有数据时主循环饿死。
const MAX_BURST_READS: usize = 16;

/// `struct input_event`（64 位 Linux：timeval 16 字节 + 8 字节载荷 = 24）。
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RawInputEvent {
    pub sec: i64,
    pub usec: i64,
    pub etype: u16,
    pub code: u16,
    pub value: i32,
}

impl RawInputEvent {
    pub fn to_event(self) -> Event {
        Event {
            etype: self.etype,
            code: self.code,
            value: self.value,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct DeviceId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UInputSetup {
    pub id: DeviceId,
    pub name: [u8; 80],
    pub ff_effects_max: u32,
}

// ---- ioctl 号（asm-generic，x86_64 / aarch64 一致） ----
const IOC_NRBITS: u64 = 8;
const IOC_TYPEBITS: u64 = 8;
const IOC_SIZEBITS: u64 = 14;
const IOC_NRSHIFT: u64 = 0;
const IOC_TYPESHIFT: u64 = IOC_NRSHIFT + IOC_NRBITS;
const IOC_SIZESHIFT: u64 = IOC_TYPESHIFT + IOC_TYPEBITS;
const IOC_DIRSHIFT: u64 = IOC_SIZESHIFT + IOC_SIZEBITS;
const IOC_NONE: u64 = 0;
const IOC_WRITE: u64 = 1;
const IOC_READ: u64 = 2;

const fn ioc(dir: u64, ty: u64, nr: u64, size: u64) -> u64 {
    (dir << IOC_DIRSHIFT) | (ty << IOC_TYPESHIFT) | (nr << IOC_NRSHIFT) | (size << IOC_SIZESHIFT)
}

const fn iow(ty: u64, nr: u64, size: u64) -> u64 {
    ioc(IOC_WRITE, ty, nr, size)
}
const fn ior(ty: u64, nr: u64, size: u64) -> u64 {
    ioc(IOC_READ, ty, nr, size)
}
const fn io(ty: u64, nr: u64) -> u64 {
    ioc(IOC_NONE, ty, nr, 0)
}

pub const EVIOCGID: u64 = ior(b'E' as u64, 0x02, std::mem::size_of::<DeviceId>() as u64);
pub const EVIOCGRAB: u64 = iow(b'E' as u64, 0x90, 4);
pub const EVIOCREVOKE: u64 = iow(b'E' as u64, 0x91, 4);

pub fn eviocgled(len: usize) -> u64 {
    ior(b'E' as u64, 0x19, len as u64)
}

pub fn eviocgname(len: usize) -> u64 {
    ior(b'E' as u64, 0x06, len as u64)
}
pub fn eviocgbit(ev: u16, len: usize) -> u64 {
    ior(b'E' as u64, 0x20 + ev as u64, len as u64)
}
pub fn eviocgabs(code: u16) -> u64 {
    ior(
        b'E' as u64,
        0x40 + code as u64,
        std::mem::size_of::<AbsInfo>() as u64,
    )
}

pub const UI_SET_EVBIT: u64 = iow(b'U' as u64, 100, 4);
pub const UI_SET_KEYBIT: u64 = iow(b'U' as u64, 101, 4);
pub const UI_SET_ABSBIT: u64 = iow(b'U' as u64, 103, 4);
pub const UI_DEV_SETUP: u64 = iow(b'U' as u64, 3, std::mem::size_of::<UInputSetup>() as u64);
pub const UI_DEV_CREATE: u64 = io(b'U' as u64, 1);
pub const UI_DEV_DESTROY: u64 = io(b'U' as u64, 2);

/// 直接走 `syscall(SYS_ioctl)`。
///
/// musl 把 `ioctl()` 的请求码声明成 `int`，高位请求码（`0xC0...` 这类）
/// 经它传递时有截断歧义；`syscall` 的参数统一是 `u64`，glibc / musl 行为一致，
/// 也让本文件可以用 musl 目标交叉编译（无需 zig 提供 glibc sysroot）。
fn do_ioctl(fd: RawFd, req: u64, arg: *mut libc::c_void) -> libc::c_int {
    unsafe { libc::syscall(libc::SYS_ioctl, fd, req, arg) as libc::c_int }
}

fn ioctl_ptr(fd: RawFd, req: u64, arg: *mut u8) -> io::Result<()> {
    // SAFETY: 所有调用点都传入了与 req 编码一致的合法指针（见各处注释）
    let r = do_ioctl(fd, req, arg as *mut libc::c_void);
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn ioctl_val(fd: RawFd, req: u64, val: i32) -> io::Result<()> {
    // SAFETY: EVIOCGRAB / UI_SET_*BIT 的 ioctl 参数按值传递 int
    let r = do_ioctl(fd, req, val as *mut libc::c_void);
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

// --------------------------------------------------------------------------- //
// 设备
// --------------------------------------------------------------------------- //

pub struct InputDevice {
    pub path: String,
    pub name: String,
    pub fd: RawFd,
    pub id: DeviceId,
}

impl InputDevice {
    pub fn open(path: &str) -> io::Result<Self> {
        let c = CString::new(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad path"))?;
        // O_NONBLOCK：主循环用 poll 阻塞，读时不能挂着
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut dev = Self {
            path: path.to_string(),
            name: String::new(),
            fd,
            id: DeviceId::default(),
        };
        dev.name = dev.query_name().unwrap_or_default();
        dev.id = dev.query_id().unwrap_or_default();
        Ok(dev)
    }

    fn query_name(&self) -> io::Result<String> {
        let mut buf = [0u8; 256];
        ioctl_ptr(self.fd, eviocgname(buf.len()), buf.as_mut_ptr())?;
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        Ok(String::from_utf8_lossy(&buf[..end]).to_string())
    }

    fn query_id(&self) -> io::Result<DeviceId> {
        let mut id = DeviceId::default();
        ioctl_ptr(self.fd, EVIOCGID, &mut id as *mut DeviceId as *mut u8)?;
        Ok(id)
    }

    /// 支持的事件类型集合。
    pub fn event_types(&self) -> io::Result<Vec<u16>> {
        let mut buf = [0u8; 8]; // 足够容纳 EV_MAX(0x1f) 位
        ioctl_ptr(self.fd, eviocgbit(0, buf.len()), buf.as_mut_ptr())?;
        Ok(bits(&buf)
            .into_iter()
            .filter(|v| *v <= 0x1f)
            .map(|v| v as u16)
            .collect())
    }

    /// 某个事件类型支持的码集合。
    pub fn codes(&self, etype: u16) -> io::Result<Vec<u16>> {
        let mut buf = [0u8; 96]; // KEY_MAX(0x2ff) = 768 bit
        ioctl_ptr(self.fd, eviocgbit(etype, buf.len()), buf.as_mut_ptr())?;
        Ok(bits(&buf).into_iter().map(|v| v as u16).collect())
    }

    pub fn supports(&self, etype: u16, code: u16) -> bool {
        self.codes(etype)
            .map(|c| c.contains(&code))
            .unwrap_or(false)
    }

    pub fn abs_info(&self, code: u16) -> io::Result<AbsInfo> {
        let mut info = AbsInfo::default();
        ioctl_ptr(
            self.fd,
            eviocgabs(code),
            &mut info as *mut AbsInfo as *mut u8,
        )?;
        Ok(info)
    }

    pub fn grab(&self) -> io::Result<()> {
        ioctl_val(self.fd, EVIOCGRAB, 1)
    }

    pub fn ungrab(&self) -> io::Result<()> {
        ioctl_val(self.fd, EVIOCGRAB, 0)
    }

    pub fn has_event_type(&self, etype: u16) -> bool {
        self.event_types()
            .map(|t| t.contains(&etype))
            .unwrap_or(false)
    }

    /// 读 LED 位图，返回 NumLock 灯是否亮。
    ///
    /// 官方驱动靠 NumLock 指示灯做双向联动，用户态用 `EVIOCGLED` 读同一份状态，
    /// 比解析 /sys/class/leds 更省事且不依赖节点命名。
    pub fn read_led_numl(&self) -> io::Result<bool> {
        let mut buf = [0u8; 8];
        ioctl_ptr(self.fd, eviocgled(buf.len()), buf.as_mut_ptr())?;
        let byte = buf[0];
        Ok(byte & 1 != 0) // LED_NUML == bit 0
    }

    /// 非阻塞读取一批事件，写入调用方给的缓冲（先清空）。
    ///
    /// 主循环每秒要读上百次，这里不留任何堆分配：缓冲在栈上，事件直接按位拷进
    /// `out`。`out` 的容量由调用方决定，跨帧复用。
    pub fn read_events_into(&self, out: &mut Vec<RawInputEvent>) -> io::Result<()> {
        out.clear();
        let sizes = std::mem::size_of::<RawInputEvent>();
        // 缓冲马上被 read() 整块覆盖，不需要先清零
        let mut buf = std::mem::MaybeUninit::<[u8; EVENT_BUF_BYTES]>::uninit();
        let buf = buf.as_mut_ptr() as *mut u8;
        // 一次唤醒内排着的不止一块事件：多点触摸加快速滑动时，一帧 64 条很容易读满。
        // 只读一块会把剩下的事件压到下一帧（8 ms 之后），坐标与压力被拆到两帧，
        // 判键、判手势都会错位。这里持续读到 EAGAIN 为止；上限防止某设备持续灌
        // 数据把主循环饿死（剩下的下一帧继续读）。
        for _ in 0..MAX_BURST_READS {
            let n = unsafe { libc::read(self.fd, buf as *mut libc::c_void, EVENT_BUF_BYTES) };
            if n < 0 {
                let err = io::Error::last_os_error();
                match err.kind() {
                    io::ErrorKind::WouldBlock => return Ok(()),
                    io::ErrorKind::Interrupted => continue,
                    _ => return Err(err),
                }
            }
            if n == 0 {
                return Ok(());
            }
            // read() 最多回填 EVENT_BUF_BYTES，天然不会多于 MAX_EVENTS_PER_READ，
            // 这里只需按整块向下取整，丢掉尾部不足一个事件的残片。
            let count = n as usize / sizes;
            out.reserve(count);
            for i in 0..count {
                // SAFETY: RawInputEvent 是 repr(C) 的 POD（全整数），缓冲按同样大小切块，
                // 尾部可能有不足一个事件的残片，count 已按整块向下取整。
                let ev =
                    unsafe { std::ptr::read_unaligned(buf.add(i * sizes) as *const RawInputEvent) };
                out.push(ev);
            }
            if count < MAX_EVENTS_PER_READ {
                // 没读满说明内核缓冲已空，不必再猜着多读一次
                return Ok(());
            }
        }
        Ok(())
    }

    /// 非阻塞读取一批事件（自建缓冲的便捷版，热路径请用 [`Self::read_events_into`]）。
    pub fn read_events(&self) -> io::Result<Vec<RawInputEvent>> {
        let mut out = Vec::new();
        self.read_events_into(&mut out)?;
        Ok(out)
    }

    /// poll 等待可读，超时毫秒。
    pub fn wait_readable(&self, timeout_ms: i32) -> io::Result<bool> {
        let mut pfd = libc::pollfd {
            fd: self.fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let r = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if r < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(false);
            }
            return Err(err);
        }
        if r == 0 {
            return Ok(false);
        }
        // 设备被拔掉 / 驱动卸载时 poll 也会返回 >0，但置的是 ERR/HUP 而不是 POLLIN。
        // 把它当成「有事件可读」会让主循环空转，所以这里转成错误交给调用方处理。
        if pfd.revents & libc::POLLIN != 0 {
            return Ok(true);
        }
        if pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(io::Error::from_raw_os_error(libc::ENODEV));
        }
        Ok(false)
    }
}

impl Drop for InputDevice {
    fn drop(&mut self) {
        // 进程退出必须松开 grab，否则触摸板会被卡在「被占用」直到重登
        let _ = self.ungrab();
        if self.fd >= 0 {
            unsafe { libc::close(self.fd) };
        }
    }
}

fn bits(buf: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, byte) in buf.iter().enumerate() {
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                out.push(i * 8 + bit);
            }
        }
    }
    out
}

// --------------------------------------------------------------------------- //
// uinput
// --------------------------------------------------------------------------- //

/// 虚拟键盘 / 虚拟触摸板。
pub struct VirtualDevice {
    fd: RawFd,
    pub name: String,
}

impl VirtualDevice {
    pub fn create_keyboard(name: &str) -> io::Result<Self> {
        let dev = Self::open(name)?;
        dev.enable_event_type(kc::EV_KEY)?;
        dev.enable_event_type(kc::EV_SYN)?;
        for &code in kc::ALL_KEYS.iter() {
            dev.enable_key(code)?;
        }
        dev.finish()?;
        Ok(dev)
    }

    /// 虚拟触摸板：把未吞掉的触点原样还给桌面。
    ///
    /// 官方 `dwSupport_MixedMode = 1` 的语义是「数字键与光标共存」，
    /// 靠一层 uinput 转发就能做到，不需要改内核。
    pub fn create_touchpad(
        name: &str,
        abs_ranges: &[(u16, AbsInfo)],
        key_codes: &[u16],
    ) -> io::Result<Self> {
        let dev = Self::open(name)?;
        dev.enable_event_type(kc::EV_ABS)?;
        dev.enable_event_type(kc::EV_KEY)?;
        dev.enable_event_type(kc::EV_SYN)?;
        for (code, info) in abs_ranges {
            dev.setup_abs(*code, info)?;
        }
        for code in key_codes {
            dev.enable_key(*code)?;
        }
        dev.finish()?;
        Ok(dev)
    }

    fn open(name: &str) -> io::Result<Self> {
        let path = CString::new("/dev/uinput").unwrap();
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            fd,
            name: name.to_string(),
        })
    }

    fn enable_event_type(&self, etype: u16) -> io::Result<()> {
        ioctl_val(self.fd, UI_SET_EVBIT, etype as i32)
    }

    fn enable_key(&self, code: u16) -> io::Result<()> {
        ioctl_val(self.fd, UI_SET_KEYBIT, code as i32)
    }

    fn setup_abs(&self, code: u16, info: &AbsInfo) -> io::Result<()> {
        ioctl_val(self.fd, UI_SET_ABSBIT, code as i32)?;
        // 老内核没有 UI_ABS_SETUP 时才需要，此处仅设置 absbit，范围由 UI_DEV_SETUP 之外的
        // 默认值决定；为了让桌面正确缩放，直接写 UI_ABS_SETUP 更可靠
        ui_abs_setup(self.fd, code, info)
    }

    fn finish(&self) -> io::Result<()> {
        let mut setup = UInputSetup {
            id: DeviceId {
                bustype: 0x18,
                vendor: 0x0b05,
                product: 0x0000,
                version: 1,
            },
            name: [0u8; 80],
            ff_effects_max: 0,
        };
        let bytes = self.name.as_bytes();
        let n = bytes.len().min(79);
        setup.name[..n].copy_from_slice(&bytes[..n]);
        ioctl_ptr(
            self.fd,
            UI_DEV_SETUP,
            &mut setup as *mut UInputSetup as *mut u8,
        )?;
        ioctl_ptr(self.fd, UI_DEV_CREATE, std::ptr::null_mut())
    }

    pub fn write_event(&self, ev: &Event) -> io::Result<()> {
        let raw = RawInputEvent {
            sec: 0,
            usec: 0,
            etype: ev.etype,
            code: ev.code,
            value: ev.value,
        };
        let bytes = std::mem::size_of::<RawInputEvent>();
        // SAFETY: raw 是 repr(C) POD，长度与内核期望的一致
        let n = unsafe {
            libc::write(
                self.fd,
                &raw as *const RawInputEvent as *const libc::c_void,
                bytes,
            )
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn syn(&self) -> io::Result<()> {
        self.write_event(&Event {
            etype: kc::EV_SYN,
            code: 0,
            value: 0,
        })
    }
}

impl Drop for VirtualDevice {
    fn drop(&mut self) {
        if self.fd >= 0 {
            let _ = ioctl_ptr(self.fd, UI_DEV_DESTROY, std::ptr::null_mut());
            unsafe { libc::close(self.fd) };
        }
    }
}

#[repr(C)]
struct UInputAbsSetup {
    code: u16,
    pad: u16,
    absinfo: AbsInfo,
}

const UI_ABS_SETUP: u64 = iow(b'U' as u64, 4, std::mem::size_of::<UInputAbsSetup>() as u64);

fn ui_abs_setup(fd: RawFd, code: u16, info: &AbsInfo) -> io::Result<()> {
    let mut arg = UInputAbsSetup {
        code,
        pad: 0,
        absinfo: *info,
    };
    ioctl_ptr(fd, UI_ABS_SETUP, &mut arg as *mut UInputAbsSetup as *mut u8)
}

// --------------------------------------------------------------------------- //
// 探测
// --------------------------------------------------------------------------- //

/// 枚举 /dev/input/event*。
pub fn list_event_devices() -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir("/dev/input") else {
        return out;
    };
    for entry in rd.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("event") {
            out.push(format!("/dev/input/{name}"));
        }
    }
    out.sort_by_key(|p| {
        p.rsplit(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0)
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ioctl_numbers_match_linux_headers() {
        // 与 <linux/input.h> 中 EVIOCGRAB / EVIOCGID 的实际取值对齐（x86_64）
        assert_eq!(EVIOCGID, 0x80084502);
        assert_eq!(EVIOCGRAB, 0x40044590);
        assert_eq!(UI_DEV_CREATE, 0x5501);
        assert_eq!(UI_DEV_DESTROY, 0x5502);
        assert_eq!(UI_SET_EVBIT, 0x40045564);
        assert_eq!(UI_SET_KEYBIT, 0x40045565);
        assert_eq!(UI_SET_ABSBIT, 0x40045567);
    }

    #[test]
    fn raw_input_event_is_24_bytes() {
        assert_eq!(std::mem::size_of::<RawInputEvent>(), 24);
    }
}
