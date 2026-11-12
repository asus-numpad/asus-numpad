# Copyright 1999-2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

# 跟随仓库主干的 live ebuild：直接拉最新提交编译，用于在提交合入前试用。
# 复制到 overlay 后 `sudo emerge -av =sys-apps/numdeck-9999`。

EAPI=8

inherit git-r3 systemd udev

DESCRIPTION="Touchpad numeric keypad userspace driver for Linux (ASUS NumberPad lineage, official AlgoParameters aligned)"
HOMEPAGE="https://github.com/numdeck-dev/numdeck"
EGIT_REPO_URI="https://github.com/numdeck-dev/numdeck.git"
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
	dobin target/release/numdeck
	doman packaging/common/numdeck.1

	insinto /usr/share/numdeck/lang
	doins lang/zh_CN.lang lang/zh_TW.lang lang/en.lang

	udev_dorules packaging/common/99-numdeck.rules

	if use systemd; then
		systemd_douserunit packaging/common/numdeck.service
	fi

	insinto /etc/numdeck
	newins - numdeck.json <<-EOF
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
	elog "启用用户级服务：systemctl --user enable --now numdeck.service"
	elog "自检：numdeck check"
}
