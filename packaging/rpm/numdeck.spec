# 单一包 spec：一个二进制覆盖全部机型，机型配置在运行期自动识别。
#   rpmbuild -bb numdeck.spec
# 实际出包走的是 tools/rpmgen/build_rpm.py（Mac/Linux 都能跑，不需要 rpmbuild），
# 这份 spec 保持与之等价，供发行版官方构建使用。

%{!?version:%global version @VERSION@}

Name:           numdeck
Summary:        ASUS 触摸板数字小键盘（NumberPad）用户态驱动
Version:        %{version}
Release:        @RELEASE@%{?dist}
# 旧版按机型分包（numdeck-g614 等）已被单一包取代：升级时由本包接管，
# 装过任意机型包的机器 dnf upgrade 会自动换过来，不会因为共用 /usr/bin/numdeck 报文件冲突。
# 这里的名单只为接管历史包名，不代表机型支持范围。
#
# 两个坑：① %for 是 Fedora 补的扩展宏，RHEL 9 / openSUSE 的 rpm 4.16 直接报
# 「Unknown tag: %for」，只能平铺写死；② %{release} 要等 Release 标签解析出来才有值，
# 这段放在 Version/Release 之前会被 rpm 判成「Possible unexpanded macro」。
Provides:       numdeck-g533 = %{version}-%{release}
Obsoletes:      numdeck-g533 < %{version}-%{release}
Provides:       numdeck-g614 = %{version}-%{release}
Obsoletes:      numdeck-g614 < %{version}-%{release}
Provides:       numdeck-g615 = %{version}-%{release}
Obsoletes:      numdeck-g615 < %{version}-%{release}
Provides:       numdeck-g634 = %{version}-%{release}
Obsoletes:      numdeck-g634 < %{version}-%{release}
Provides:       numdeck-g635 = %{version}-%{release}
Obsoletes:      numdeck-g635 < %{version}-%{release}
Provides:       numdeck-gx550 = %{version}-%{release}
Obsoletes:      numdeck-gx550 < %{version}-%{release}
Provides:       numdeck-gx650 = %{version}-%{release}
Obsoletes:      numdeck-gx650 < %{version}-%{release}
Provides:       numdeck-ux582 = %{version}-%{release}
Obsoletes:      numdeck-ux582 < %{version}-%{release}
License:        GPL-3.0-or-later
URL:            https://github.com/numdeck-dev/numdeck
Source0:        numdeck-%{version}.tar.gz
BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  systemd-rpm-macros

%description
ASUS 触摸板数字小键盘（NumberPad）用户态驱动。参数取自官方 AsusNUMPADFilter.inf
的 AlgoParameters 原值，evdev/uinput 实现，不替换内核模块、不改 ACPI 表。

机型配置在运行期自动识别：读 DMI 型号与触摸板模组挑对应的键表，不需要按机型选包。
硬件信息读不出来时可手动指定（写进配置长期生效）：

    numdeck sku            # 看本机识别到哪套配置、凭什么
    numdeck sku list       # 列出可选机型
    numdeck sku g614       # 手动指定，numdeck sku auto 恢复自动识别

%prep
%autosetup -n numdeck-%{version}

%build
# 发行版构建环境默认禁网：有 Cargo.lock 才用 --locked，没有就让 cargo 自行解析
if [ -f Cargo.lock ]; then
    cargo build --release --locked --offline || cargo build --release --locked
else
    cargo build --release
fi

%install
install -Dm0755 target/release/numdeck %{buildroot}%{_bindir}/numdeck
install -Dm0644 packaging/common/numdeck.service \
    %{buildroot}%{_userunitdir}/numdeck.service
install -Dm0644 packaging/common/99-numdeck.rules \
    %{buildroot}%{_udevrulesdir}/99-numdeck.rules
install -Dm0644 packaging/common/numdeck.1 \
    %{buildroot}%{_mandir}/man1/numdeck.1
install -d %{buildroot}%{_datadir}/numdeck/lang
install -m0644 lang/zh_CN.lang lang/zh_TW.lang lang/en.lang \
    %{buildroot}%{_datadir}/numdeck/lang/

install -d %{buildroot}%{_sysconfdir}/numdeck
cat > %{buildroot}%{_sysconfdir}/numdeck/numdeck.json <<'JSON'
{
  "numlock": "sync",
  "calc_gesture": true,
  "calc_gesture_mode": "long",
  "mixed_mode": "auto",
  "grab": true,
  "follow_keyboard_brightness": true,
  "language": "auto"
}
JSON

%files
%doc README.md
%license LICENSE
%{_bindir}/numdeck
%{_userunitdir}/numdeck.service
%{_udevrulesdir}/99-numdeck.rules
# Fedora/openSUSE 的 brp-compress 会把 man page 压成 .1.gz，
# %files 里写死 .1 会在「Processing files」阶段报 File not found，必须吃通配。
%{_mandir}/man1/numdeck.1*
%dir %{_datadir}/numdeck
%dir %{_datadir}/numdeck/lang
%{_datadir}/numdeck/lang/zh_CN.lang
%{_datadir}/numdeck/lang/zh_TW.lang
%{_datadir}/numdeck/lang/en.lang
%dir %{_sysconfdir}/numdeck
%config(noreplace) %{_sysconfdir}/numdeck/numdeck.json

%post
# 用户级服务：装完不用 root 就随会话自启
systemctl --global enable numdeck.service >/dev/null 2>&1 || :
# 从旧版 system 级服务升级：停掉旧的，避免两个进程抢同一块触摸板
systemctl --no-reload disable --now numdeck.service >/dev/null 2>&1 || :
udevadm control --reload-rules >/dev/null 2>&1 || true
udevadm trigger --subsystem-match=input --action=change >/dev/null 2>&1 || true
udevadm trigger --subsystem-match=misc --action=change >/dev/null 2>&1 || true
# hidraw 节点装包时已存在，规则重载不会重放，必须显式 trigger 才补上 uaccess ACL
udevadm trigger --subsystem-match=hidraw --action=change >/dev/null 2>&1 || true
udevadm settle >/dev/null 2>&1 || true
# 开 linger：用户级单元在开机阶段即被拉起，不必先登录桌面
_installer="${SUDO_USER:-}"
if [ -z "$_installer" ]; then
    # 不用 awk：最小化安装的容器里可能没有它
    _installer=$(loginctl list-users --no-legend 2>/dev/null | while read -r _u _uid _rest; do
        case "$_uid" in ''|*[!0-9]*) continue ;; esac
        if [ "$_uid" -ge 1000 ]; then echo "$_u"; break; fi
    done)
fi
if [ -n "$_installer" ] && [ "$_installer" != "root" ]; then
    loginctl enable-linger "$_installer" >/dev/null 2>&1 || :
fi
loginctl list-users --no-legend 2>/dev/null | while read -r u _rest; do
    uid=$(id -u "$u" 2>/dev/null) || continue
    [ -d "/run/user/$uid" ] || continue
    su "$u" -s /bin/sh -c "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user daemon-reload; XDG_RUNTIME_DIR=/run/user/$uid systemctl --user enable --now numdeck.service" >/dev/null 2>&1 || :
done

%preun
if [ $1 -eq 0 ]; then
    loginctl list-users --no-legend 2>/dev/null | while read -r u _rest; do
        uid=$(id -u "$u" 2>/dev/null) || continue
        [ -d "/run/user/$uid" ] || continue
        su "$u" -s /bin/sh -c "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user disable --now numdeck.service" >/dev/null 2>&1 || :
    done
    systemctl --global disable numdeck.service >/dev/null 2>&1 || :
    systemctl --no-reload disable --now numdeck.service >/dev/null 2>&1 || :
fi

%postun
if [ $1 -ge 1 ]; then
    loginctl list-users --no-legend 2>/dev/null | while read -r u _rest; do
        uid=$(id -u "$u" 2>/dev/null) || continue
        [ -d "/run/user/$uid" ] || continue
        su "$u" -s /bin/sh -c "XDG_RUNTIME_DIR=/run/user/$uid systemctl --user daemon-reload; XDG_RUNTIME_DIR=/run/user/$uid systemctl --user try-restart numdeck.service" >/dev/null 2>&1 || :
    done
fi
udevadm control --reload-rules >/dev/null 2>&1 || true

%changelog
* Thu Nov 05 2026 numdeck maintainers <maintainers@example.invalid> - @VERSION@-22
- 手动三档与跟随映射对齐：bright 1/2/3 → 0x43/0x46/0x48，切换 follow 不再跳变
- 精简程序内提示文案与文档

* Wed Nov 04 2026 numdeck maintainers <maintainers@example.invalid> - @VERSION@-18
- %post 补 hidraw trigger 与 linger：装完即生效、开机自启
- udev 规则加 KERNEL=="hidraw*" 限定
