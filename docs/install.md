# 安装

支持 Debian 系的 apt、Fedora/RHEL 的 dnf、openSUSE 的 zypper、Arch 的 pacman、
Alpine 的 apk，以及 Gentoo、Nix/NixOS 和任意发行版都能用的静态 tarball。

两种取用方式，任选其一：

- **走软件源**（推荐）：加一条仓库配置，之后 `apt upgrade` / `dnf upgrade` /
  `zypper update` / `pacman -Syu` 跟着升级。
- **直接装包**：从 [Releases](https://github.com/asus-numpad/asus-numpad/releases) 下载本机格式安装一次，升级要自己重复。

只有一个包，机型配置由程序在运行期自动识别，装之前不必查机型表，也不会装错包。表里对不上的
机器会拒绝启动（退出码 `3`，并打印支持机型），不使用其他机型的参数运行。装完想确认识别结果：

```bash
asus-numpad sku          # 识别到哪套配置、依据是什么
```

硬件信息读不准（虚拟机、白牌机箱、BIOS 占位串）时手动指定，写进配置长期生效：

```bash
asus-numpad sku list     # 可选机型 id
asus-numpad sku g614     # 手动指定
asus-numpad sku auto     # 恢复自动识别
```

机型与模组、键表的对应见 [README 机型表](../README.md#支持机型) 与 [machines.md](machines.md)。

项目已迁到 `asus-numpad` 组织，版本号从 1.0.0 重编。旧源（`RunningVAN/asus-numpad`）里 1.2.4
及更早的包不能就地升级到 1.0.0，换源前先卸一次再装：`sudo dnf remove asus-numpad` /
`sudo apt remove asus-numpad`。

## 软件源

仓库地址是 `https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo`，
由 CI 在每次发版后自动重建，每个生态保留最近 3 个版本。索引带 GPG 签名：apt 是
`InRelease`，dnf / zypper 开 `repo_gpgcheck`，pacman 走 `pacman-key`，apk 用 RSA 成员签名。
公钥随索引发布在 `repo/keys/` 下，`install.sh` 先导入公钥再按各生态的校验路径装包，没有
跳过校验的开关。指纹与私钥托管见 [signing.md](signing.md)。

### 源里放的是哪个包

**每个生态只出一份通用包**，不按发行版拆。以前 deb 出 4 份（jammy / noble /
bookworm / trixie）、rpm 出 3 份（el9 / fc44 / suse），但那几份的二进制内容其实
一样、只差编译环境的 C 库下限，纯属重复。现在改成「在 C 库最老的发行版里编一次，
靠 glibc 的向后兼容覆盖整条线」：

| 生态 | 包 | 编自 | 覆盖范围 |
| --- | --- | --- | --- |
| apt | `asus-numpad_<ver>_amd64.deb` | Ubuntu 22.04（glibc 2.35） | Ubuntu 22.04 / 24.04、Debian 12 / 13、Mint 21+ |
| rpm | `asus-numpad-<ver>-1.el9.x86_64.rpm` | AlmaLinux 9（glibc 2.34） | RHEL / Rocky / Alma 9 与 10、Fedora、openSUSE Leap 16 / Tumbleweed |
| pacman | `asus-numpad-<ver>-1-x86_64.pkg.tar.zst` | Arch | Arch / Manjaro / EndeavourOS |
| apk | `asus-numpad-<ver>-r0.apk` | Alpine（musl） | Alpine 3.19+ |
| AUR | `asus-numpad-rs`（源码包） | 你自己 makepkg | Arch / Manjaro，随滚动版一起更新 |

Arch 与 Alpine 不存在「跨版本通用」这回事——Arch 是滚动版，Alpine 走 musl 独立
体系，这两份是各自的原生包，不算重复。

「一份包真能通用」由 CI 兜底，不靠嘴说：每次发版会把**同一份** deb 装进
Debian 12 / 13 与 Ubuntu 22.04 / 24.04，把**同一份** rpm 装进 AlmaLinux 9 /
Fedora / openSUSE Leap 16 / Tumbleweed，逐个跑 `--version`，有一处装不上就不发版。

### Debian / Ubuntu / Mint

```bash
sudo install -d -m 0755 /usr/share/keyrings
curl -fsSL https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/keys/asus-numpad-repo.gpg \
  | sudo tee /usr/share/keyrings/asus-numpad.gpg >/dev/null
echo "deb [signed-by=/usr/share/keyrings/asus-numpad.gpg] https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update
sudo apt install asus-numpad
```

`signed-by` 把公钥钉在这一个源上：`apt` 只认这把钥签的 `Release` / `InRelease`，
顺带按 `Release` 里的 SHA256 校验包体，两处对不上就装不进去。公钥随索引一起发布在
`repo/keys/` 下，换钥时重抄一遍上面的命令即可。

### Fedora / RHEL / Rocky / Alma

```bash
sudo rpm --import https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/keys/asus-numpad-repo.asc
sudo tee /etc/yum.repos.d/asus-numpad.repo >/dev/null <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/rpm/$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/keys/asus-numpad-repo.asc
EOF
sudo dnf install asus-numpad
```

`gpgcheck=1` 校验包签名，`repo_gpgcheck=1` 校验 `repomd.xml` 的签名：索引被改过、
包和索引对不上，`dnf` 都直接拒。`gpgkey` 指回仓库自己的公钥，不用另找地方下载。

### openSUSE Leap / Tumbleweed

仓库是 rpm-md，zypper 直接读，不需要另编一份 SUSE 包：

```bash
sudo rpm --import https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/keys/asus-numpad-repo.asc
sudo zypper addrepo --refresh \
  "https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/rpm/$(uname -m)" asus-numpad
sudo zypper install asus-numpad
```

`rpm --import` 之后 zypper 走默认的签名校验，和 dnf 侧同一个 rpm-md 源、同一把钥。源里和 Release 里就只有
这份 rpm，spec 只用标准宏、不读发行版私有宏，运行时只依赖 glibc 与 systemd，
所以 Leap 16、SLES 16 与 Tumbleweed 上照装；Leap 15 系已 EOL 且 glibc 低于本包要求，
不在覆盖范围内。

装之前不必自己验一遍：每次发版 CI 都会在 10 个干净容器（Debian 12/13、
Ubuntu 22.04/24.04、AlmaLinux 9、Fedora、openSUSE Leap 16、openSUSE Tumbleweed、
Arch、Alpine）里真装一次、跑一遍 `--version`，有一个装不上就不发版。

### Arch / Manjaro

```bash
curl -fsSL https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/keys/asus-numpad-repo.asc -o /tmp/asus-numpad.asc
sudo pacman-key --add /tmp/asus-numpad.asc
sudo pacman-key --lsign-key 79F282157F2D3FF8E3C6806314C2BF611986E168
sudo tee -a /etc/pacman.conf >/dev/null <<'EOF'

[asus-numpad]
SigLevel = Required DatabaseRequired
Server = https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/arch/$arch
EOF
sudo pacman -Syu asus-numpad
```

`Required DatabaseRequired` 要求包与数据库都必须有有效签名：`.sig` 缺失、钥没导入都对不上。
`--lsign-key` 只是把钥标成本地信任级（`pacman-key` 的信任网在没有 keyring 签名仪式时本来
就到不了 ultimate），校验本身一样严格。

### Arch / Manjaro：AUR

不想加第三方 pacman 源、或者想跟着自己系统的滚动版走，用源码包 `asus-numpad-rs`：

```bash
# 立刻可用：从 Release 下 PKGBUILD 与 asus-numpad-rs.install 到空目录
mkdir asus-numpad-rs && cd asus-numpad-rs
curl -fLO https://github.com/asus-numpad/asus-numpad/releases/latest/download/PKGBUILD
curl -fLO https://github.com/asus-numpad/asus-numpad/releases/latest/download/asus-numpad-rs.install
makepkg -si
```

包名带 `-rs` 是因为 AUR 上的 `asus-numpad` 已被另一个 Python 实现的项目占用；
二进制名仍是 `asus-numpad`，PKGBUILD 用 `provides` / `conflicts` 声明了替代关系。

AUR 包是**源码包**：`makepkg` 直接拉 GitHub 的 tag tarball 现编，所以装出来的
是针对你本机工具链的原生二进制。每次发版 CI 都会在 Arch 容器里用同一个
PKGBUILD 真编真装、跑 `--version` 并校验 systemd 单元与 udev 规则落地后才发版。

### Alpine

```bash
curl -fsSL https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/keys/asus-numpad-apk.rsa.pub \
  | sudo tee /etc/apk/keys/asus-numpad-apk.rsa.pub >/dev/null
echo "https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo/alpine/$(apk --print-arch)" \
  | sudo tee -a /etc/apk/repositories
sudo apk update
sudo apk add asus-numpad
```

`apk` 认 `/etc/apk/keys/` 里的公钥，文件名必须与签名成员名 `.SIGN.RSA.asus-numpad-apk.rsa.pub`
对上——所以公钥落盘时不能改名。

## 直接装包

从 Releases 下载对应文件，文件名里的版本号以页面为准：

| 发行版 | 文件 |
| --- | --- |
| Debian / Ubuntu / Mint | `asus-numpad_<ver>_amd64.deb` |
| Fedora / RHEL / openSUSE Leap 16 / Tumbleweed | `asus-numpad-<ver>-1.el9.x86_64.rpm` |
| Arch / Manjaro | `asus-numpad-<ver>-1-x86_64.pkg.tar.zst` |
| Arch / Manjaro（源码） | `PKGBUILD` + `.SRCINFO`（AUR 包的同一份） |
| Alpine | `asus-numpad-<ver>-r0.apk` |
| 任意 x86_64 | `asus-numpad-<ver>-x86_64.tar.gz` |

```bash
sudo apt install ./asus-numpad_<ver>_amd64.deb
sudo dnf install ./asus-numpad-<ver>-1.el9.x86_64.rpm
sudo pacman -U asus-numpad-<ver>-1-x86_64.pkg.tar.zst
sudo apk add ./asus-numpad-1.0.0-r0.apk   # 已按上一节导入公钥；没导入时加 --allow-untrusted
```

发版同时产出 `SHA256SUMS`，校验方式：

```bash
sha256sum -c asus-numpad-1.2.4-SHA256SUMS --ignore-missing
```

## tarball（任意发行版）

静态链接（musl）的 tarball 不依赖 glibc 版本，理论上任何 x86_64 Linux 都能跑；
包里带安装脚本，不经过包管理器：

```bash
tar xzf asus-numpad-<ver>-x86_64.tar.gz
cd asus-numpad-1.2.4
sudo ./install.sh --dry-run     # 先看它要做什么
sudo ./install.sh
```

`install.sh` 的参数：

| 参数 | 作用 |
| --- | --- |
| `--prefix /usr/local` | 装到非默认前缀（默认 `/`） |
| `--dry-run` | 只打印动作，不落盘 |
| `--no-service` | 只放文件，不装 systemd 用户单元与 udev 规则 |
| `--keep-config` | 已存在配置文件时不覆盖 |

卸载用同目录的 `uninstall.sh`（`--purge` 连配置一起删）。两者做的事与 deb/rpm 包等价，
区别只是文件不归包管理器记账。

## Gentoo

```bash
sudo mkdir -p /etc/portage/repos.conf
# 用本地 overlay 即可：
sudo mkdir -p /var/db/repos/asus-numpad/sys-apps/asus-numpad
sudo cp packaging/gentoo/asus-numpad-1.0.1.ebuild /var/db/repos/asus-numpad/sys-apps/asus-numpad/
cd /var/db/repos/asus-numpad && sudo repoman manifest 2>/dev/null || sudo pkgdev manifest
sudo emerge -av sys-apps/asus-numpad
```

`asus-numpad-9999.ebuild` 是 live 版，直接跟 main 分支。

## Nix / NixOS

```bash
# 临时跑
nix run github:asus-numpad/asus-numpad

# 装进 profile
nix profile install github:asus-numpad/asus-numpad

# NixOS（flake）
{
  inputs.asus-numpad.url = "github:asus-numpad/asus-numpad";
  # 在某个 nixosSystem 的 modules 里：
  # { imports = [ inputs.asus-numpad.nixosModules.default ];
  #   services.asus-numpad.enable = true; }
}
```

## 源码构建

```bash
git clone https://github.com/asus-numpad/asus-numpad
cd asus-numpad
cargo build --release --locked
sudo install -Dm755 target/release/asus-numpad /usr/bin/asus-numpad
sudo install -Dm644 packaging/common/asus-numpad.service /usr/lib/systemd/user/
sudo install -Dm644 packaging/common/99-asus-numpad.rules /usr/lib/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger --subsystem-match=hidraw --action=change
sudo udevadm settle
systemctl --user enable --now asus-numpad
```

`udevadm trigger --subsystem-match=hidraw` 这步别省：hidraw 节点在装包前就存在，
`--reload-rules` 不会把新规则重放到既有节点上，漏了这一步会一直 `Permission denied`。

## 装完自检

```bash
asus-numpad check          # 机型、设备、几何换算、背光候选，只读
systemctl --user status asus-numpad
getfacl /dev/hidraw* | grep -E 'user|group'   # 应看到 uaccess 授权
```

`check` 的机型段若报不匹配，会直接列出该装的包名。

## 升级与卸载

| 方式 | 升级 | 卸载 |
| --- | --- | --- |
| apt | `sudo apt upgrade` | `sudo apt remove asus-numpad` |
| dnf / zypper | `sudo dnf upgrade` | `sudo dnf remove asus-numpad` |
| pacman | `sudo pacman -Syu` | `sudo pacman -R asus-numpad` |
| apk | `sudo apk upgrade` | `sudo apk del asus-numpad` |
| tarball | 下新 tarball 重跑 `install.sh` | `sudo ./uninstall.sh` |

配置文件 `~/.config/asus-numpad/asus-numpad.json` 不会随卸载删除，需要重置就手动删掉。
