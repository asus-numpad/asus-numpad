//! 背光节点选择策略（跨平台纯函数，便于在非 Linux 主机上做单元测试）。
//!
//! NumberPad 触摸板的 `vid:pid` 随机型而变，唯一稳定的判据是
//! **HID 报告描述符里是否存在 FEATURE 报告 `0x0d`**。本模块只负责
//! “候选节点打分”，不碰 sysfs / ioctl，Linux 侧实现见 [`crate::linux::led`]。

/// ELAN 触摸板 vendor（NumberPad 触摸板模组几乎都是它）。
pub const ELAN_VID: u32 = 0x04f3;
/// PixArt 触摸板 vendor：`ASCP1400` / `ASCF1400` 模组上报为 `093A:2808`，
/// 此前因不在白名单内导致点灯链路断开。
pub const PIXART_VID: u32 = 0x093a;
/// ASUS vendor ID：部分机型的 N-KEY / 触摸板接口挂在它下面。
pub const ASUS_VID: u32 = 0x0b05;
/// OMNI RECEIVER 无线接收器 PID：绝不能往里写背光报文。
pub const OMNI_PID: u32 = 0x1ace;

/// 已知带 NumberPad 背光的 ELAN 触摸板 PID（`04F3:3275`）。
///
/// **只用于排序与日志，不用于排除**：其它机型 PID 各不相同
/// （甚至同机型不同批次也会变），排除逻辑一律走报告描述符判定。
pub const KNOWN_NUMPAD_PIDS: &[u32] = &[0x3275];

/// 给一个 hidraw 候选打分，用于跨机型选择背光 FEATURE 节点。
///
/// 返回 `Some(rank)`，rank 越小越优先；`None` 表示不是候选。
///   - rank 0：ELAN / PixArt 触摸板 + 描述符含 FEATURE `0x0d`（最可靠）
///   - rank 1：ASUS 节点 + 描述符含 FEATURE `0x0d`
///   - rank 2：已知 PID 的 ELAN 触摸板（无描述符证据，交给 ioctl 判定）
///   - rank 3：其它 ELAN / PixArt 触摸板（无描述符证据）
///   - rank 4：其它 ASUS 节点（最后回落）
pub fn candidate_rank(vid: u32, pid: u32, has_feature: bool) -> Option<u8> {
    // ROG 无线接收器，绝对不能写
    if vid == ASUS_VID && pid == OMNI_PID {
        return None;
    }
    // 触摸板本体厂商：ELAN 与 PixArt（ASCP1400 模组）。两者同等看待，
    // 真正决定优先级的是报告描述符里有没有 FEATURE `0x0d`。
    let is_touchpad = vid == ELAN_VID || vid == PIXART_VID;
    let is_asus = vid == ASUS_VID;
    if !(is_touchpad || is_asus) {
        return None;
    }
    let known = is_touchpad && KNOWN_NUMPAD_PIDS.contains(&pid);
    Some(match (is_touchpad, has_feature, known) {
        (true, true, _) => 0,
        (false, true, _) => 1,
        (true, false, true) => 2,
        (true, false, false) => 3,
        (false, false, _) => 4,
    })
}

/// 在候选列表里选出最终要写的节点下标（`(vid, pid, has_feature)` 三元组）。
///
/// 与 [`candidate_rank`] 同序，rank 相同时保留先出现的那个（sysfs 遍历顺序）。
pub fn pick_best(candidates: &[(u32, u32, bool)]) -> Option<usize> {
    let mut best: Option<(u8, usize)> = None;
    for (idx, &(vid, pid, has_feature)) in candidates.iter().enumerate() {
        let Some(rank) = candidate_rank(vid, pid, has_feature) else {
            continue;
        };
        let better = match best {
            None => true,
            Some((r, _)) => rank < r,
        };
        if better {
            best = Some((rank, idx));
        }
    }
    best.map(|(_, i)| i)
}

/// 需要在日志里提示“没找到 FEATURE 证据”的候选 rank 阈值。
pub const RANK_WARN_NO_FEATURE: u8 = 2;
/// 需要在日志里提示“回落到 ASUS 通用节点”的候选 rank 阈值。
pub const RANK_WARN_ASUS_FALLBACK: u8 = 4;

/// 单个节点「有没有 NumberPad 特征」的探测结论（跨平台口径，`linux::led` 负责填）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumpadProbe {
    /// 描述符里确有 FEATURE 报告 `0x0d`
    Has,
    /// 描述符读到了，确实没有该 FEATURE
    Missing,
    /// 没权限读描述符，判不了
    NoAccess,
    /// 节点异常 / 司机刚消失
    Unknown,
}

/// 本机到底有没有 NumberPad 硬件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumpadPresence {
    /// 至少一个触摸板节点带 FEATURE `0x0d`
    Present,
    /// 节点都在、描述符也读到了，但没有一个带该 FEATURE
    Absent,
    /// 节点在，没权限读描述符，判不了
    NoAccess,
    /// 没扫到候选节点（非 ELAN / PixArt / ASUS 触摸板，或 hidraw 未加载）
    Unknown,
}

/// 背光通道失效后，从「当前实际存在」的节点表里把同一块硬件捞回来。
///
/// 挂起恢复、USB 重枚举、驱动重载都会让 `/dev/hidrawN` 的编号整体漂移：
/// 昨天是 `hidraw3`，醒来变成 `hidraw7`，旧的路径永远打不开。内置链路这时
/// 只会每次点灯刷一条 warning，灯一路黑到用户手动重启服务——所以要按
/// `(vid:pid)` 认设备，而不是按节点编号认设备。
///
/// `present` 是调用方扫出来的 `(节点路径, vid, pid)` 列表；`None` 表示没找到同型号。
/// 优先选**同名节点**（编号被同一个设备抢回来，通常就是它）。
pub fn reattach_channel(
    dev: &str,
    vid: u32,
    pid: u32,
    present: &[(String, u32, u32)],
) -> Option<String> {
    let same_model: Vec<&(String, u32, u32)> = present
        .iter()
        .filter(|(_, v, p)| *v == vid && *p == pid)
        .collect();
    if let Some(hit) = same_model.iter().find(|(d, _, _)| d == dev) {
        return Some(hit.0.clone());
    }
    // 同型号可能不止一个接口，也可能换成别的编号：取第一个能用的。
    // 不能跨型号兜底——写错通道会往别的设备发 FEATURE 报文。
    same_model.first().map(|(d, _, _)| d.clone())
}

/// 把逐节点的探测结果收敛成一个结论。
///
/// 优先级：有就是有 → 有「确实没有」才敢判无硬件 → 只剩「没权限」就说明判不了。
/// 「没权限」绝不能被当成「没有这块硬件」：udev 的 `uaccess` 没生效时读不到描述符，
/// 按无硬件拒绝启动会把排查方向直接带偏。
pub fn presence_from(probes: &[NumpadProbe]) -> NumpadPresence {
    if probes.contains(&NumpadProbe::Has) {
        return NumpadPresence::Present;
    }
    if probes.contains(&NumpadProbe::Missing) {
        return NumpadPresence::Absent;
    }
    if probes.contains(&NumpadProbe::NoAccess) {
        return NumpadPresence::NoAccess;
    }
    NumpadPresence::Unknown
}

#[cfg(test)]
mod tests {
    use super::{presence_from, NumpadPresence, NumpadProbe, *};

    #[test]
    fn numpad_presence_any_has_wins() {
        // 触摸板会导出多个 hidraw 接口，只有带 0x0d 的那个算数
        let probes = [
            NumpadProbe::Missing,
            NumpadProbe::Has,
            NumpadProbe::NoAccess,
        ];
        assert_eq!(presence_from(&probes), NumpadPresence::Present);
    }

    #[test]
    fn numpad_presence_missing_is_absent() {
        // 4 区 RGB 配置：节点都在、描述符读得到，就是没有 0x0d
        let probes = [NumpadProbe::Missing, NumpadProbe::Missing];
        assert_eq!(presence_from(&probes), NumpadPresence::Absent);
    }

    #[test]
    fn numpad_presence_no_access_is_not_absent() {
        let probes = [NumpadProbe::NoAccess, NumpadProbe::Unknown];
        assert_eq!(presence_from(&probes), NumpadPresence::NoAccess);
    }

    #[test]
    fn numpad_presence_nothing_is_unknown() {
        assert_eq!(presence_from(&[]), NumpadPresence::Unknown);
        assert_eq!(
            presence_from(&[NumpadProbe::Unknown]),
            NumpadPresence::Unknown
        );
    }

    #[test]
    fn omni_receiver_is_never_a_candidate() {
        assert_eq!(candidate_rank(0x0b05, 0x1ace, true), None);
        assert_eq!(candidate_rank(0x0b05, 0x1ace, false), None);
    }

    #[test]
    fn non_vendor_nodes_are_rejected() {
        assert_eq!(candidate_rank(0x046d, 0x3275, true), None); // Logitech
        assert_eq!(candidate_rank(0x0000, 0x0000, true), None);
    }

    #[test]
    fn elan_with_feature_beats_everything() {
        // 本机 04f3:3275 + 描述符 0x0d
        assert_eq!(candidate_rank(0x04f3, 0x3275, true), Some(0));
        // 未知 PID 的 ELAN 触摸板（其它 ASUS 机型），只要带 0x0d 同样 rank 0
        assert_eq!(candidate_rank(0x04f3, 0x3286, true), Some(0));
        assert_eq!(candidate_rank(0x04f3, 0x31b9, true), Some(0));
        // ASUS 节点带 0x0d 排第二
        assert_eq!(candidate_rank(0x0b05, 0x19b6, true), Some(1));
    }

    #[test]
    fn unknown_pid_without_feature_is_below_known_pid() {
        let known = candidate_rank(0x04f3, 0x3275, false).unwrap();
        let unknown = candidate_rank(0x04f3, 0x3286, false).unwrap();
        assert!(
            known < unknown,
            "已知 PID 应优先于未知 PID（同为无描述符证据）"
        );
        assert!(unknown < candidate_rank(0x0b05, 0x1866, false).unwrap());
    }

    #[test]
    fn pick_best_prefers_feature_proof_over_vendor_only() {
        // 模拟 G634JY：无线接收器 + ASUS N-KEY + 触摸板本体
        let cands = vec![
            (0x0b05, 0x1ace, false), // OMNI RECEIVER，必须被忽略
            (0x0b05, 0x19b6, false), // ASUS N-KEY Device
            (0x04f3, 0x3275, true),  // 触摸板本体，带 0x0d
        ];
        assert_eq!(pick_best(&cands), Some(2));
    }

    #[test]
    fn pick_best_works_for_other_models_without_known_pid() {
        // 其它 ASUS 机型：ELAN PID 不是 3275，但描述符同样带 0x0d
        let cands = vec![(0x0b05, 0x19b6, true), (0x04f3, 0x3286, true)];
        assert_eq!(pick_best(&cands), Some(1));
    }

    #[test]
    fn pixart_touchpad_is_a_candidate() {
        // ASCP1400 / ASCF1400 模组上报 093A:2808
        assert_eq!(candidate_rank(0x093a, 0x2808, true), Some(0));
        assert_eq!(candidate_rank(0x093a, 0x2808, false), Some(3));
        let cands = vec![
            (0x0b05, 0x1ace, false), // OMNI RECEIVER
            (0x093a, 0x2808, true),  // PixArt 触摸板本体，带 0x0d
        ];
        assert_eq!(pick_best(&cands), Some(1));
    }

    #[test]
    fn pick_best_returns_none_when_only_receiver() {
        let cands = vec![(0x0b05, 0x1ace, true)];
        assert_eq!(pick_best(&cands), None);
    }
    #[test]
    fn reattach_prefers_same_node_number() {
        let present = vec![
            ("/dev/hidraw7".to_string(), ELAN_VID, 0x3275),
            ("/dev/hidraw3".to_string(), ELAN_VID, 0x3275),
        ];
        // 编号回来了就用它，别去动兄弟接口
        assert_eq!(
            reattach_channel("/dev/hidraw3", ELAN_VID, 0x3275, &present).as_deref(),
            Some("/dev/hidraw3")
        );
    }

    #[test]
    fn reattach_falls_back_to_same_model_new_number() {
        let present = vec![("/dev/hidraw9".to_string(), ELAN_VID, 0x3275)];
        assert_eq!(
            reattach_channel("/dev/hidraw3", ELAN_VID, 0x3275, &present).as_deref(),
            Some("/dev/hidraw9")
        );
    }

    #[test]
    fn reattach_never_crosses_models() {
        // 只剩无线接收器 / 别的触摸板时宁可保持断开，也不能往错设备写报文
        let present = vec![
            ("/dev/hidraw4".to_string(), ASUS_VID, OMNI_PID),
            ("/dev/hidraw5".to_string(), PIXART_VID, 0x2808),
        ];
        assert_eq!(
            reattach_channel("/dev/hidraw3", ELAN_VID, 0x3275, &present),
            None
        );
        assert_eq!(
            reattach_channel("/dev/hidraw3", ELAN_VID, 0x3275, &[]),
            None
        );
    }
}
