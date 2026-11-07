# 软件源签名

四个生态（apt / rpm-md / pacman / apk）共用同一套发布流程，签名做在索引上：包变了索引才变，
索引变了签名才失效，所以用户端只要信任公钥一次，之后每次升级都是端到端校验过的。

## 密钥

| 用途 | 算法 | 标识 | 公钥文件 |
| --- | --- | --- | --- |
| apt / dnf / pacman 共用 | GPG RSA 4096，`[SC]`，无到期日 | 指纹 `79F282157F2D3FF8E3C6806314C2BF611986E168`，`uid` 为 `asus-numpad repository signing key <repo@asus-numpad.github.io>` | `tools/keys/asus-numpad-repo.asc`（文本）、`tools/keys/asus-numpad-repo.gpg`（二进制，给 apt 的 `signed-by` 用） |
| Alpine apk | RSA 4096 | 由私钥文件名决定签名成员名：`.SIGN.RSA.asus-numpad-apk.rsa.pub` | `tools/keys/asus-numpad-apk.rsa.pub` |

公钥随索引一起发布在 `repo/keys/` 下，所以下面的安装命令都能直接从仓库里取到公钥，不需要
用户去别处下载。私钥不进仓库，只存在于本地 `keys-private/`（已被 `.gitignore` 排除）和
仓库的 Actions Secrets 里。

## CI 侧配置

在仓库 `Settings → Secrets and variables → Actions` 加两条 Secret：

| Secret | 内容 |
| --- | --- |
| `REPO_GPG_PRIVATE_KEY` | `asus-numpad-repo.secret.asc` 的完整内容，含 `-----BEGIN PGP PRIVATE KEY BLOCK-----` 与结尾行 |
| `REPO_APK_PRIVATE_KEY` | `asus-numpad-apk.rsa` 的完整内容，PEM 格式，含 `-----BEGIN RSA PRIVATE KEY-----` 与结尾行 |

两条都可缺省，缺省时该生态退回不校验签名（见下），流水线不会因此失败：

- 两条都有：`signed-by`、`gpgcheck=1`、`SigLevel = Required DatabaseRequired`、apk 走公钥校验。
- 只有 `REPO_GPG_PRIVATE_KEY`：apt / dnf / pacman 强校验，apk 未签名。
- 都没有：`trusted=yes`、`gpgcheck=0`、`SigLevel = Optional TrustAll`、`apk --allow-untrusted`。

`publish` job 里私钥只落在 runner 的内存与 `$RUNNER_TEMP`，不上传 artifacts、不写进 git。

## 签名流程

`tools/repo-gen.sh` 带 `--gpg-key <指纹>` 与 `--apk-key <私钥路径>` 走签名模式，四类签名产物：

| 生态 | 签名对象 | 产物 |
| --- | --- | --- |
| apt | `dists/stable/Release` | `Release.gpg`（二进制分离签名）+ `InRelease`（明文签名） |
| rpm-md | `repodata/repomd.xml` | `repomd.xml.asc`；包本身由 `rpm --addsign` 加签名头 |
| pacman | `asus-numpad.db.tar.gz`、每个包 | `<db>.sig`、`<包>.sig` |
| apk | 每个 `.apk`、`APKINDEX.tar.gz` | 签名追加在文件尾部，或进 tar 成为 `.SIGN.RSA.*` 成员 |

包必须签在索引之前：索引里存的是包的校验和，先建索引再签包会让校验和对不上。脚本因此先把包
复制进临时目录签好、再建索引。

CI 里 `repo-add` 与 `abuild-sign` 是从 Arch / Alpine 容器借来的工具，容器里没有 gpg 也没有宿主机
的 keyring，所以 pacman 的数据库签名交回宿主机做（`repo-add` 只负责建库），apk 则把私钥与待签
文件搬进挂载目录再让容器签。

## 用户端校验强度

`publish` job 在发 Release 之前会按官网给用户的命令原样跑一遍 apt / dnf / pacman / apk 四个容器，
全程开着严格校验：任何一处密钥没配、公钥没随索引发出、索引签名顺序错了，这一步都会红掉，版本
不会发出去。

## 轮换

私钥泄露或需要定期换钥时：

1. 生成新钥（GPG：`gpg --quick-generate-key "asus-numpad repository signing key <repo@asus-numpad.github.io>" rsa4096 sign never`；apk：`openssl genrsa -out asus-numpad-apk.rsa 4096` 再 `openssl rsa -in ... -pubout -out asus-numpad-apk.rsa.pub`）。
2. 覆盖 `tools/keys/` 下的公钥，更新上面的 Secrets。
3. 发一个补丁版本。索引会用新钥重签，用户端需重新导入一次公钥——安装命令里这一步本来就有，
   所以用户重抄一次命令即可，`trusted=yes` 之类的回退不需要。
4. 旧公钥同时保留一个版本（`keys/` 里加 `-old` 后缀的文件不参与签名，仅供用户过渡时手工导入）。

apk 换钥要注意公钥文件名：`.SIGN.RSA.<私钥文件名>.pub` 是按私钥文件名生成的，换钥必须同时改
用户端 `/etc/apk/keys/` 下的文件名，安装命令里已经和私钥文件名对齐。
