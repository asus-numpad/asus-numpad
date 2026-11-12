#!/usr/bin/env bash
# 无人值守相关的静态自检：只读，不碰设备、不改配置。
#
# 这些项在开发机上无法真机验证（挂起、热插拔、systemd 重启策略），
# 但它们决定了「用户不动手时，服务能不能自己恢复」，所以至少把
# 代码与 unit 里的既有约定钉成检查项，避免后续改动悄悄回退。
set -u

cd "$(dirname "$0")/.." || exit 1
OUT="verify-service.txt"
: > "$OUT"

pass=0
fail=0
skip=0

chk() { # chk <描述> <文件> <正则>
    local desc="$1" file="$2" pat="$3"
    if [ ! -f "$file" ]; then
        echo "SKIP  $desc（缺文件 $file）" | tee -a "$OUT"
        skip=$((skip + 1))
        return
    fi
    if grep -Eq "$pat" "$file"; then
        echo "PASS  $desc" | tee -a "$OUT"
        pass=$((pass + 1))
    else
        echo "FAIL  $desc" | tee -a "$OUT"
        fail=$((fail + 1))
    fi
}

echo "== 服务单元：重试语义 ==" | tee -a "$OUT"
chk "无限重试（设备暂时不在时要能自愈）" packaging/common/numdeck.service '^StartLimitIntervalSec=0'
chk "永久性失败不重启（机型不支持 / 无硬件）" packaging/common/numdeck.service '^RestartPreventExitStatus=2 3'
chk "reload 走重启而非原地换配置" packaging/common/numdeck.service '^ExecReload=/bin/kill -HUP \$MAINPID'
chk "重启间隔 >= 2s（别把日志刷爆）" packaging/common/numdeck.service '^RestartSec=[2-9]'

echo "== 进程侧：退出码与锁 ==" | tee -a "$OUT"
chk "单实例锁（防两个实例互抢 grab）" src/linux/mod.rs 'fn acquire_single_instance'
chk "锁失败以非零码退出（否则服务停在 inactive）" src/cli/run.rs 'ServiceExit::Busy => Ok\(75\)'
chk "SIGHUP = 重载而不是停机" src/linux/mod.rs 'RELOAD\.store\(true'
chk "挂起恢复补灯" src/linux/mod.rs 'resume_detected\('
chk "grab 运行期重试" src/engine/mod.rs 'should_retry_grab'
chk "uinput 写失败重建" src/linux/dev.rs 'fn rebuild|write_or_recover'
chk "SYN_DROPPED 自愈" src/engine/mod.rs 'fn handle_syn_dropped'

echo "== 通道漂移：按型号认设备，不按节点编号 ==" | tee -a "$OUT"
chk "hidraw 背光通道可重连" src/linux/led.rs 'fn reattach\('
chk "按 (vid,pid) 找回而不是按 hidrawN" src/led_policy.rs 'fn reattach_channel'
chk "键盘背光 sysfs 路径每次读失败重新解析" src/brightness.rs 'let fresh = resolve_dir'

echo "== 界面不被后台探测拖住 ==" | tee -a "$OUT"
chk "平台状态在后台线程读（同步起 systemctl 会卡帧）" gui/src/app/mod.rs 'state_rx: Option<std::sync::mpsc::Receiver<Snapshot>>'
chk "命令面板在后台线程跑" gui/src/app/command.rs 'std::thread::spawn\(move \|\| \{'
chk "服务启停走后台线程（systemctl restart 慢时会卡界面）" gui/src/app/mod.rs 'op_rx: Option<std::sync::mpsc::Receiver<Result<String, String>>>'
chk "服务操作同一时刻只放行一件" gui/src/app/tasks.rs '上一步服务操作还没结束'
chk "状态读取有超时上限（否则重读被永久挡住）" gui/src/app/tasks.rs 'STATE_TIMEOUT'
chk "原子写先 fsync 临时文件再改名" gui/src/settings.rs 'f\.sync_all\(\)'
chk "命令输出有采集上限" gui/src/cli_bridge.rs 'MAX_CAPTURE'
chk "界面设置解析失败时留副本" gui/src/settings.rs 'fn keep_broken_copy'
chk "档表按亮度排序（否则高档更暗）" gui/src/settings.rs 'kbd_map.sort_unstable'
chk "驱动配置保存回显被修正项" gui/src/app/driver.rs '见右侧列表'

echo "== 键位/配置不被静默丢弃 ==" | tee -a "$OUT"
chk "配置坏字段前移 sanitize" src/config/sanitize.rs 'fn sanitize'
chk "片段应用先全量解析" gui/src/app/extend/fragment.rs 'apply_fragment'

echo "----------------------------------------------------------------" | tee -a "$OUT"
echo "汇总: PASS=$pass FAIL=$fail SKIP=$skip" | tee -a "$OUT"
[ "$fail" -eq 0 ] || exit 1
