# 打包说明

打包层全部是 Bash：编译交给 cargo，装包交给发行版原生工具（`dpkg-deb` / `rpmbuild` /
`makepkg` / `abuild`），没有 Python，不需要 debhelper / fpm。Nix 与 Gentoo 走各自生态的
原生描述文件，不塞进 Bash。

## 目录

```
packaging/
├── build.sh          统一入口：编译一次，按目标出包
├── lib/common.sh     共用函数（版本读取、目录布置、日志）
├── common/           service / udev 规则 / man page
├── deb/              dpkg-deb 直接打包（不走 dh）
├── rpm/              spec + tarball → rpmbuild -bb（Fedora / RHEL / openSUSE 通用）
├── arch/             PKGBUILD，无 makepkg 时手工拼同构包
├── alpine/           APKBUILD + abuild，无 abuild 时手工拼 .apk
├── tarball/          静态 tarball + install.sh / uninstall.sh（任意发行版）
├── gentoo/           ebuild（版本化 + 9999 live）
└── nix/              package.nix + NixOS module（配合根目录 flake.nix）
```

分发包只装 `numdeck`、man 页、service 与 udev 规则。GUI（`numdeck-gui`）不进包：
它在 Linux 上只是可选项，egui / winit 的依赖体积比驱动本体大一个量级，编译一次要拉几十个
crate。要用就自行 `cargo build --release -p numdeck-gui`。

## 用法

```bash
./packaging/build.sh                 # 按 /etc/os-release 自动判断
./packaging/build.sh deb             # 只出 .deb
./packaging/build.sh rpm arch        # 指定多目标
./packaging/build.sh --check         # 只自检文件齐全性与工具链
./packaging/build.sh --no-build deb  # 复用已有 target/release/numdeck
```

产物落在 `dist/`。也可单独调脚本：

```bash
./packaging/deb/make-deb.sh      --version 5.0.1 --no-build --out /tmp/out
./packaging/rpm/make-rpm.sh      --version 5.0.1 --out /tmp/out
./packaging/arch/make-arch.sh    --version 5.0.1 --no-build --out /tmp/out
./packaging/alpine/make-alpine.sh --version 5.0.1 --out /tmp/out
./packaging/tarball/make-tarball.sh --version 5.0.1 --static --out /tmp/out
```

编译产物只出一次即可复用：出多个格式时设 `NUMDECK_BIN=target/release/numdeck`
并给各脚本加 `--no-build`，避免每格式重新编译一遍。

## 分发仓库

`tools/repo-gen.sh` 把 `dist/` 整理成四套仓库，供 GitHub 分发（apt / rpm / pacman / apk
各自的索引 + `index.html` 说明页）：

```bash
./tools/repo-gen.sh                        # dist/ → repo/
./tools/repo-gen.sh --keep 2               # 每个生态只留最近 2 个版本
```

索引工具（`dpkg-scanpackages` / `createrepo_c` / `repo-add` / `apk`）缺失时只跳过对应生态
并告警，不中断其它生态。CI 里 `repo/` 会随发版提交回 `main`，用户侧的源地址见
`docs/install.md`。

`--no-build` 语义不一致：`make-deb.sh` / `make-arch.sh` 表示跳过 cargo，复用已有二进制；
rpm 的编译在 spec 的 `%build` 里，`make-rpm.sh` 只有 `--check`（`rpmbuild --nobuild` 校验）。

## 约定

- 版本号单一来源：所有脚本从 `Cargo.toml` 读 `version`，spec 用 `@VERSION@` 占位，
  Arch 用 sed 改 `pkgver`，三处不会打架。
- rpm 源码走 tarball：`%autosetup` 需要 `%{name}-%{version}.tar.gz`，导出时排除
  `target/`、`dist/`、`.git`。
- Arch 降级路径：非 Arch 主机或以 root 运行时，用 bsdtar 手工拼 `.PKGINFO` + `.pkg.tar.zst`。
- 不动系统：包只装文件与 hooks，不改 `modprobe`、不碰 ACPI，卸载后行为与装前一致。
- `%post` / `post_install` 必须显式 `udevadm trigger --subsystem-match=hidraw --action=change`
  再 `settle`：hidraw 节点在装包前已存在，`reload-rules` 不会重放规则到既有节点，
  漏了这步用户会一直撞 `Permission denied`。

## 静态检查

```bash
shellcheck -x -S warning packaging/build.sh packaging/lib/common.sh \
    packaging/deb/make-deb.sh packaging/rpm/make-rpm.sh packaging/arch/make-arch.sh \
    packaging/arch/PKGBUILD packaging/arch/numdeck.install \
    packaging/alpine/make-alpine.sh packaging/tarball/make-tarball.sh \
    tools/repo-gen.sh
shellcheck -s sh -S warning packaging/deb/postinst packaging/deb/prerm packaging/deb/postrm \
    packaging/tarball/install.sh packaging/tarball/uninstall.sh \
    packaging/alpine/numdeck.post-install
```

## 安装后的文件布局

| 路径 | 内容 |
| --- | --- |
| `/usr/bin/numdeck` | 二进制 |
| `/usr/lib/systemd/user/numdeck.service` | 用户单元（随会话启动，免 root） |
| `/usr/lib/udev/rules.d/99-numdeck.rules` | 权限规则 |
| `/usr/share/man/man1/numdeck.1` | man |
| `/etc/numdeck/numdeck.json` | 系统级配置（rpm `%config(noreplace)`） |
| `~/.config/numdeck/numdeck.json` | 用户配置，日常读写的就是这份 |
| `/usr/share/doc/numdeck/` | README 与 copyright |

## 发版顺序

推 `v*` 标签即触发 `.github/workflows/release.yml`：各发行版容器出包 → 汇总四套仓库索引 → 发 Release → 把 `repo/` 提交回 main。最后一步要写 main，所以顺序有讲究：

1. 该推的代码、文档、站点改动先全部推完，再打标签。
2. 打标签后到流水线跑完之前别再往 main 推提交。publish 跑的是标签指向的提交，这时多出来的提交会让 `repo/` 那次推送变成非快进。工作流里已经先 rebase 到最新 main 再推，但真撞上同一批文件还是要人工处理。
3. 跑完看 publish 是否绿。它红了而 Release 已经挂出去，说明索引没更新，`docs/install.md` 里的安装命令拉到的还是上一版，装出来的版本对不上。修完重打标签再跑一遍。
