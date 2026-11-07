# Copyright 1999-2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

# 跟随仓库主干的 live ebuild：直接拉最新提交编译，用于在提交合入前试用。
# 复制到 overlay 后 `sudo emerge -av =sys-apps/asus-numpad-9999`。

EAPI=8

inherit git-r3 systemd udev

DESCRIPTION="ASUS NumberPad userspace driver for Linux (official AlgoParameters aligned)"
HOMEPAGE="https://github.com/asus-numpad/asus-numpad"
EGIT_REPO_URI="https://github.com/asus-numpad/asus-numpad.git"
EGIT_BRANCH="main"

LICENSE="GPL-3.0-or-later"
SLOT="0"
IUSE="+systemd"

RDEPEND="virtual/libc
	systemd? ( sys-apps/systemd )"
BDEPEND=">=dev-lang/rust-1.75"

src_unpack() {
	git-r3_src_unpack
}

src_compile() {
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

	einstalldocs
}

pkg_postinst() {
	udev_reload
	elog "启用用户级服务：systemctl --user enable --now asus-numpad.service"
	elog "自检：asus-numpad check"
}
