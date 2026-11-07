# Copyright 1999-2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

# Gentoo 版本化 ebuild。
#
# 用法：把本文件与 Manifest 一起放进本地 overlay
#   /var/db/repos/local/sys-apps/asus-numpad/asus-numpad-1.0.1.ebuild
# 然后
#   sudo ebuild asus-numpad-1.0.1.ebuild manifest
#   sudo emerge -av sys-apps/asus-numpad
#
# 若想直接源码跟随仓库最新提交，用同目录的 asus-numpad-9999.ebuild。

EAPI=8

inherit systemd udev

DESCRIPTION="ASUS NumberPad userspace driver for Linux (official AlgoParameters aligned)"
HOMEPAGE="https://github.com/asus-numpad/asus-numpad"

if [[ ${PV} == 9999 ]]; then
	inherit git-r3
	EGIT_REPO_URI="https://github.com/asus-numpad/asus-numpad.git"
else
	SRC_URI="https://github.com/asus-numpad/asus-numpad/archive/v${PV}.tar.gz -> ${P}.tar.gz"
	KEYWORDS="~amd64 ~arm64"
fi

LICENSE="GPL-3.0-or-later"
SLOT="0"
IUSE="+systemd"

# 硬件访问直接走 libc 的 evdev/uinput ABI，不需要 x11-libs/libevdev 之类的中间层
DEPEND=""
RDEPEND="
	virtual/libc
	systemd? ( sys-apps/systemd )
"
BDEPEND=">=dev-lang/rust-1.75"

src_unpack() {
	if [[ ${PV} == 9999 ]]; then
		git-r3_src_unpack
	else
		default
	fi
}

src_compile() {
	# Gentoo 支持按需联网；Cargo.lock 存在时锁死依赖版本，避免版本漂移
	if [[ -f Cargo.lock ]]; then
		cargo build --release --locked || die "cargo build 失败"
	else
		cargo build --release || die "cargo build 失败"
	fi
}

src_install() {
	dobin target/release/asus-numpad
	doman packaging/common/asus-numpad.1

	insinto /usr/share/asus-numpad/lang
	doins lang/zh_CN.lang lang/zh_TW.lang lang/en.lang

	udev_dorules packaging/common/99-asus-numpad.rules

	if use systemd; then
		systemd_douserunit packaging/common/asus-numpad.service
	fi

	insinto /etc/asus-numpad
	newins - asus-numpad.json <<-EOF
		{
		  "numlock": "sync",
		  "calc_gesture": true,
		  "calc_gesture_mode": "long",
		  "mixed_mode": "auto",
		  "grab": true,
		  "follow_keyboard_brightness": true,
		  "manual_brightness_step": 4,
		  "language": "auto"
		}
	EOF
	# 配置归用户，升级不覆盖
	keepdir /etc/asus-numpad

	einstalldocs
}

pkg_postinst() {
	udev_reload

	if use systemd; then
		elog "启用用户级服务（当前用户）："
		elog "  systemctl --user enable --now asus-numpad.service"
		elog "开机即起（不依赖登录）："
		elog "  loginctl enable-linger \$USER"
	else
		elog "无 systemd：把 'asus-numpad run' 加进桌面会话自启即可。"
	fi
	elog "自检：asus-numpad check"
}
