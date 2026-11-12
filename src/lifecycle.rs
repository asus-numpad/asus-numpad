//! 运行期自愈策略：设备节点什么时候算「没了」、什么时候该重新枚举。
//!
//! 这部分决定的是**无人值守时能不能自己爬起来**：数字键盘是常驻服务，
//! 用户在合盖唤醒、USB 重枚举、外接键盘插拔之后不会去 `systemctl restart`。
//! 真机不可能随时在旁边，所以判定条件必须是纯函数、能在任意平台按样本验证。

use std::io::ErrorKind;

/// Linux 通用 errno：设备不存在（`ENODEV`）。取值在 x86 / arm64 上一致。
const ENODEV_LINUX: i32 = 19;
/// Linux 通用 errno：设备不存在或地址不可用（`ENXIO`）。
const ENXIO_LINUX: i32 = 6;

/// 连续读失败达到这个次数就认定设备真没了（偶发 `EINTR`/`EIO` 不该退出）。
pub const READ_FAILURE_LIMIT: u32 = 8;

/// 键盘节点连续读失败达到这个次数就重新枚举（键盘被换掉后节点会失效）。
pub const KEYBOARD_REOPEN_FAILURES: u32 = 8;

/// 启动时没找到键盘，隔多久再扫一次 `/dev/input`（扫描要遍历目录，不能每帧来一次）。
pub const KEYBOARD_RESCAN_INTERVAL_MS: f64 = 2000.0;

/// 这个错误是否说明设备节点已经不可用。
///
/// 判错方向的代价不对称：把偶发错误当成「没了」会让服务反复重启、数字键盘
/// 每次抽一下；把真没了当成偶发则会让服务挂在一个死 fd 上一动不动。
/// 因此明确的设备级 errno 立即判定，其余错误靠连续失败次数兜底。
pub fn device_lost(kind: Option<ErrorKind>, raw_errno: Option<i32>, failures: u32) -> bool {
    if failures >= READ_FAILURE_LIMIT {
        return true;
    }
    if matches!(
        kind,
        Some(ErrorKind::NotFound) | Some(ErrorKind::BrokenPipe) | Some(ErrorKind::UnexpectedEof)
    ) {
        return true;
    }
    matches!(raw_errno, Some(ENODEV_LINUX) | Some(ENXIO_LINUX))
}

/// 键盘（NumLock 灯源）连续失败到这个次数，就该重新找一次节点。
///
/// 键盘只用来读 NumLock 状态，读不到不影响数字区本身；但一直抱着一个失效的
/// 节点会让「NumLock 亮时点灯」这条联动永久失灵，直到用户手动重启服务。
pub fn should_reopen_keyboard(consecutive_failures: u32) -> bool {
    consecutive_failures >= KEYBOARD_REOPEN_FAILURES
}

/// 距上次扫描已过 `elapsed_ms`，该不该再扫一次键盘节点。
///
/// 传负数表示「从未扫过」。时钟倒退（挂起恢复后某些时钟源会重置）同样按
/// 「该扫」处理：宁可多扫一次，也不要让服务在唤醒后一直等一个走不到的时长。
pub fn should_rescan(elapsed_ms: f64) -> bool {
    elapsed_ms < 0.0 || elapsed_ms >= KEYBOARD_RESCAN_INTERVAL_MS
}

/// 相邻两帧的时间差超过这个值，就认为进程刚刚经历过挂起 / 长时间阻塞
/// （时间源用含休眠计时的 `CLOCK_BOOTTIME`，普通的 `CLOCK_MONOTONIC` 在挂起期间不走）。
pub const RESUME_GAP_MS: f64 = 5000.0;

/// 这一帧的时间差是否说明刚发生过挂起恢复。
///
/// 阈值取 5 秒：正常帧间隔是 8 ms，偶发的卡顿（重负载、GC 式的内存回收）不会到秒级；
/// 而任何一次真实的挂起都远超这个数量级。判早了会白白重刷灯，判晚了灯就一直黑着。
pub fn resume_detected(gap_ms: f64) -> bool {
    gap_ms >= RESUME_GAP_MS
}

/// grab 重试间隔（毫秒）。
///
/// 独占抓取失败时事件会同时进桌面：用户按数字键既出数字又移动光标。
/// 失败原因往往是暂时的（别的进程短暂占着、权限刚被放开），所以运行期要重试；
/// 但间隔不能短——一直失败的话会刷满日志，也白白耗电。
pub const GRAB_RETRY_MS: f64 = 4000.0;

/// 到点该不该重试 grab。
pub fn should_retry_grab(want: bool, grabbed: bool, lit: bool, since_ms: f64) -> bool {
    want && !grabbed && lit && since_ms >= GRAB_RETRY_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::ErrorKind;

    #[test]
    fn device_lost_on_explicit_device_errnos() {
        // 明确是「设备没了」的 errno 立即判定，不用等连续失败
        assert!(device_lost(Some(ErrorKind::NotFound), None, 0));
        assert!(device_lost(Some(ErrorKind::BrokenPipe), None, 0));
        assert!(device_lost(Some(ErrorKind::UnexpectedEof), None, 0));
        assert!(device_lost(None, Some(ENODEV_LINUX), 0));
        assert!(device_lost(None, Some(ENXIO_LINUX), 0));
    }

    #[test]
    fn transient_errors_do_not_kill_service() {
        // 偶发 EINTR / EIO：判错方向的代价不对称，不能当成「设备没了」
        assert!(!device_lost(Some(ErrorKind::Interrupted), None, 0));
        assert!(!device_lost(Some(ErrorKind::Other), Some(5), 0));
        // 但连续失败到上限就必须兜底退出
        assert!(device_lost(
            Some(ErrorKind::Other),
            Some(5),
            READ_FAILURE_LIMIT
        ));
    }

    #[test]
    fn keyboard_reopen_needs_failure_budget() {
        assert!(!should_reopen_keyboard(KEYBOARD_REOPEN_FAILURES - 1));
        assert!(should_reopen_keyboard(KEYBOARD_REOPEN_FAILURES));
    }

    #[test]
    fn rescan_throttled() {
        assert!(!should_rescan(KEYBOARD_RESCAN_INTERVAL_MS - 1.0));
        assert!(should_rescan(KEYBOARD_RESCAN_INTERVAL_MS));
    }

    #[test]
    fn resume_detected_on_suspend_gap() {
        assert!(!resume_detected(RESUME_GAP_MS - 1.0));
        assert!(resume_detected(RESUME_GAP_MS));
        // 启动第一帧的巨大 gap 也不误判：阈值以上一律视为睡过，由调用方
        // 保证「首个采样点」只作基线
        assert!(resume_detected(60_000.0));
    }

    #[test]
    fn grab_retry_only_when_needed() {
        assert!(!should_retry_grab(false, false, true, 99_999.0));
        assert!(!should_retry_grab(true, true, true, 99_999.0));
        assert!(!should_retry_grab(true, false, false, 99_999.0));
        assert!(!should_retry_grab(true, false, true, GRAB_RETRY_MS - 1.0));
        assert!(should_retry_grab(true, false, true, GRAB_RETRY_MS));
    }
}
