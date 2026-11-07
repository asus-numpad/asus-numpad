# asus-numpad 通用 tarball 包

这个包不为某个包管理器设计，给的是**一条不依赖发行版的路**：解包、跑 install.sh、结束。
适合 Debian/Fedora/Arch 之外的发行版，或不想让包管理器托管这个驱动的机器。

目标发行版举例：NixOS（另一种方式见 `packaging/nix`）、Void、Slackware、openSUSE、openEuler、Alpine（有原生 .apk，优先用 `packaging/alpine`）、各种原子更新/不可变系统。

## 安装

```sh
tar xzf asus-numpad-<version>-<arch>.tar.gz
cd asus-numpad-<version>
sudo ./install.sh
```

装完之后：

```sh
asus-numpad check              # 自检：找设备、看权限、读背光能力
systemctl --user status asus-numpad.service
```

### 选项

| 参数 | 作用 |
| --- | --- |
| `--prefix /usr/local` | 装到 `/usr/local` 而不是 `/` |
| `--dry-run` | 只打印将要做的事，不落盘 |
| `--no-service` | 只拷文件，不动 systemd / udev（容器、手工管理时用） |
| `--keep-config` | `/etc/asus-numpad/asus-numpad.json` 已存在时不写示例配置 |

`install.sh` 会用 `sudo` 调用者的身份启用用户级 systemd 单元，并开
`loginctl enable-linger`，这样开机就起来，不必先登录桌面。

## 包里的东西

```
usr/bin/asus-numpad                     二进制
usr/lib/systemd/system/asus-numpad.service
usr/lib/udev/rules.d/99-asus-numpad.rules
usr/share/man/man1/asus-numpad.1
usr/share/asus-numpad/lang/{zh_CN,zh_TW,en}.lang
etc/asus-numpad/asus-numpad.json.sample
BUILD-MODE                              记录链接方式（static-musl / dynamic-glibc）
```

`BUILD-MODE` 是 `dynamic-glibc` 时，二进制要求目标机的 glibc 不低于构建机。
跨年代发行版部署请用 `make-tarball.sh --static` 出的 musl 静态版——那种二进制
在任意 x86_64 Linux 上都能跑，连 Alpine 也行。

## 卸载

```sh
sudo ./uninstall.sh            # 保留 /etc/asus-numpad 里的配置
sudo ./uninstall.sh --purge    # 连配置一起删
```

注意：用本脚本装的文件不归包管理器管。若同一台机器上还装了 deb/rpm/arch 包，
两者会争同一个 `/usr/bin/asus-numpad`，只保留一种安装方式。

## 自己出包

```sh
./packaging/tarball/make-tarball.sh                 # 动态链接版
./packaging/tarball/make-tarball.sh --static        # musl 静态版（推荐对外分发）
./packaging/tarball/make-tarball.sh --no-build      # 复用已有 target/release
```
