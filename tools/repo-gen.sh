#!/usr/bin/env bash
# 把 dist/ 里的包整理成四套可被包管理器直接消费的仓库，供给 GitHub 分发。
#
# 产出目录（默认 <repo>/repo）：
#   apt/     Debian / Ubuntu      deb [signed-by=...] <base>/apt stable main
#   rpm/     Fedora / RHEL / SUSE baseurl=<base>/rpm/$basearch
#   arch/    Arch / Manjaro       Server = <base>/arch/$arch
#   alpine/  Alpine               <base>/alpine（apk 自己拼 <arch>）
#   keys/    对外分发的公钥（GPG + apk RSA）
#   index.html                    给人看的安装说明
#
# 索引工具缺失时只跳过对应生态并告警，不中断其它生态——打包机不必装齐所有工具。
#
# 签名：
#   --gpg-key <KEYID> 给 apt（InRelease / Release.gpg）、rpm（repomd.xml.asc + 包签名）、
#                     pacman（db 签名 + 每包 .sig）签名；--apk-key <PEM> 给 Alpine 的
#                     包与 APKINDEX 签名。两个密钥都不给时退化为未签名仓库，
#                     安装命令里自动写 trusted=yes / gpgcheck=0。
#   签名必须在算索引之前完成（包内容一变，索引里的校验和就失效），
#   因此带签名时先把产物复制到 staging 目录签好，再从 staging 建索引。
#
# 用法:
#   ./tools/repo-gen.sh                       # dist/ -> repo/（未签名）
#   ./tools/repo-gen.sh --dist DIR --out DIR
#   ./tools/repo-gen.sh --keep 2              # 每个生态只留最近 2 个版本
#   ./tools/repo-gen.sh --base-url https://... # 写进 index.html 的地址
#   ./tools/repo-gen.sh --gpg-key 79F28215 --apk-key ~/keys/apk.rsa

set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
DIST="$ROOT/dist"
OUT="$ROOT/repo"
KEEP=3
BASE_URL="https://raw.githubusercontent.com/asus-numpad/asus-numpad/main/repo"

# 签名密钥。CI 里通过环境变量注入，本地不给就是未签名仓库。
GPG_KEY="${REPO_GPG_KEY:-}"
APK_KEY="${REPO_APK_KEY:-}"

log()  { printf '==> %s\n' "$*"; }
warn() { printf '警告: %s\n' "$*" >&2; }
die()  { printf '错误: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

while (($#)); do
    case "$1" in
        --dist)     DIST="${2:?}"; shift 2 ;;
        --out)      OUT="${2:?}"; shift 2 ;;
        --keep)     KEEP="${2:?}"; shift 2 ;;
        --base-url) BASE_URL="${2:?}"; shift 2 ;;
        --gpg-key)  GPG_KEY="${2:?}"; shift 2 ;;
        --apk-key)  APK_KEY="${2:?}"; shift 2 ;;
        -h|--help)  sed -n '2,32p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *)          die "未知参数: $1" ;;
    esac
done

[[ -d "$DIST" ]] || die "产物目录不存在: $DIST"

# 版本从 deb/rpm 文件名里取，四个生态共用同一套版本号
# deb 名字有两种：带改版号（asus-numpad_1.0.0-1_amd64.deb）和不带（asus-numpad_1.0.0_amd64.deb）
VERSION="$(ls "$DIST" | sed -n \
    -e 's/^asus-numpad_\([0-9][^-]*\)_[a-z0-9]*\.deb$/\1/p' \
    -e 's/^asus-numpad_\([0-9][^-]*\)-[0-9][^-]*_[a-z0-9]*\.deb$/\1/p' \
    -e 's/^asus-numpad-\([0-9][^-]*\)-[0-9]*\..*\.rpm$/\1/p' \
  | sort -V | tail -1)"
[[ -n "$VERSION" ]] || warn "推不出版本号，索引里的 Date 用当天"
VERSION="${VERSION:-0.0.0}"

log "版本 ${VERSION}，产物 ${DIST}，输出 $OUT"

# --------------------------------------------------------------------------- #
# 版本倒退闸门
# --------------------------------------------------------------------------- #
# prune 只按版本号保留最新 KEEP 个，所以当本次版本低于仓库里已有的版本时，新包会
# 被当成"最旧"直接删掉，索引里只剩旧版本，而脚本照样以成功退出。发布侧看不出异常，
# 装的人却拿到旧包。这里宁可让流水线红掉，也不产出这种索引。
existing_versions=()
for d in "$OUT/apt/pool/main/a/asus-numpad" "$OUT/rpm/x86_64" \
         "$OUT/arch/x86_64" "$OUT/alpine/x86_64"; do
    [[ -d "$d" ]] || continue
    for f in "$d"/*; do
        [[ -e "$f" ]] || continue
        # -m1 而不是 | head -1：head 读一行就关管道，grep 收到 SIGPIPE 退出 141，
        # 配合 set -o pipefail 会让整个脚本在赋值这一步直接退出。
        v="$(printf '%s\n' "${f##*/}" | grep -oE -m1 '[0-9]+\.[0-9]+\.[0-9]+' || true)"
        if [[ -n "$v" ]]; then existing_versions+=("$v"); fi
    done
done
EXISTING=""
if ((${#existing_versions[@]})); then
    EXISTING="$(printf '%s\n' "${existing_versions[@]}" | sort -V | tail -1)"
fi
if [[ -n "$EXISTING" && "$EXISTING" != "$VERSION" ]]; then
    newest="$(printf '%s\n%s\n' "$VERSION" "$EXISTING" | sort -V | tail -1)"
    if [[ "$newest" != "$VERSION" ]]; then
        die "仓库里已有 ${EXISTING}，本次要发 ${VERSION}：版本更低，包管理器不会把它当升级，prune 还会把新包当最旧的删掉。先清掉 $OUT 下的旧版本，或改一个更高的版本号。"
    fi
fi
mkdir -p "$OUT"

# --------------------------------------------------------------------------- #
# 签名准备
# --------------------------------------------------------------------------- #
GPG_SIG=0
if [[ -n "$GPG_KEY" ]]; then
    have gpg || die "给了 --gpg-key 但找不到 gpg"
    # 私钥不在 keyring 里就别装作签过了：后面整条链路都靠它
    gpg --batch --list-secret-keys "$GPG_KEY" >/dev/null 2>&1 \
        || die "私钥不在 keyring 里: ${GPG_KEY}（先 gpg --import 或设 GNUPGHOME）"
    GPG_SIG=1
fi
APK_SIG=0
if [[ -n "$APK_KEY" ]]; then
    [[ -r "$APK_KEY" ]] || die "apk 私钥读不到: $APK_KEY"
    have abuild-sign || die "给了 --apk-key 但找不到 abuild-sign（apk add abuild）"
    APK_SIG=1
fi

# 无密码私钥 + 无 tty 的 CI 环境，必须 loopback 才能签，否则 gpg 会卡住等口令
gpg_common() { gpg --batch --yes --pinentry-mode loopback --local-user "$GPG_KEY" "$@"; }
gpg_detach()  { gpg_common --detach-sign -o "$2" "$1"; }           # 二进制 .sig（pacman / apt Release.gpg）
gpg_asc()     { gpg_common --armor --detach-sign -o "$2" "$1"; }   # 文本 .asc（dnf repomd.xml.asc）
gpg_clear()   { gpg_common --clearsign -o "$2" "$1"; }             # apt InRelease

# 每个生态一个状态位：index.html 的安装命令按位生成。
# 不能「配了密钥就写 gpgcheck=1」——环境里缺 rpm / abuild-sign 时包其实没签，
# 那样写等于给用户一条必然失败的安装命令。
APT_SIGNED=0
RPM_SIGNED=0
ARCH_SIGNED=0
APK_SIGNED=0
# 每个生态分两步签（包 / 索引），两步都成了才算这个源能开强校验
RPM_PKG_SIGNED=0
RPM_REPO_SIGNED=0
ARCH_PKG_SIGNED=0
ARCH_DB_SIGNED=0
APK_PKG_SIGNED=0
APK_IDX_SIGNED=0

GPG_FPR=""
if ((GPG_SIG)); then
    GPG_FPR="$(gpg --batch --with-colons --list-secret-keys "$GPG_KEY" 2>/dev/null \
               | awk -F: '/^fpr/{print $10; exit}')"
    log "签名密钥指纹 ${GPG_FPR}"
fi

# 签名会改写包本身（rpm 加签名头、apk 加尾部签名、arch 生成 .sig），
# 而索引里存的是包的校验和——所以先在 staging 里签完，再从 staging 建索引。
SRC="$DIST"
STAGE=""
if ((GPG_SIG || APK_SIG)); then
    STAGE="$(mktemp -d "${TMPDIR:-/tmp}/asus-numpad-repo.XXXXXX")"
    trap 'rm -rf -- "$STAGE"' EXIT
    cp -a "$DIST"/. "$STAGE"/
    SRC="$STAGE"
    log "签名模式：包先复制到 $STAGE 签好再建索引"
fi

# --------------------------------------------------------------------------- #
# 清理旧版本：仓库在 git 里，无限堆积会把仓库撑爆
# --------------------------------------------------------------------------- #
# 按「版本」保留最近 KEEP 个，而不是按文件名整串保留 KEEP 个。
#
# 老写法有两个坑，pacman 那条线都被踩到了：
#   1. '*.pkg.tar.*' 会把 asus-numpad-1.0.1-1-x86_64.pkg.tar.zst.sig 也算一个包，
#      计数虚高，drop 数量算多了；
#   2. sort -V 排整串时 'asus-numpad-1.0.1-...' 排在 'asus-numpad-debug-1.0.1-...'
#      前面，最新正式包反而被当成最旧的一批删掉，repo-add 于是只索引到 debug 包，
#      pacman 端就是 target not found。
# 现在先按版本号分组（同一版本的正式包 / debug 包 / 各生态包同进同退），
# 再删最旧的那几个版本，连带的 .sig 一起清。
prune() {
    local dir="$1"
    local -a pkgs=()
    local f b v x seen

    while IFS= read -r f; do pkgs+=("$f"); done < <(
        find "$dir" -maxdepth 1 -type f \
            \( -name '*.deb' -o -name '*.rpm' -o -name '*.pkg.tar.zst' -o -name '*.apk' \) | sort
    )
    ((${#pkgs[@]})) || return 0

    local -a vers=()
    for f in "${pkgs[@]}"; do
        b=${f##*/}
        v=$(printf '%s\n' "$b" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n1 || true)
        [[ -n "$v" ]] || continue
        if ((${#vers[@]})); then
            seen=0
            for x in "${vers[@]}"; do [[ "$x" == "$v" ]] && { seen=1; break; }; done
            ((seen)) && continue
        fi
        vers+=("$v")
    done
    ((${#vers[@]})) || return 0

    local -a sorted=()
    while IFS= read -r v; do sorted+=("$v"); done < <(printf '%s\n' "${vers[@]}" | sort -V)

    local nver=${#sorted[@]}
    ((nver <= KEEP)) && return 0
    local drop=$((nver - KEEP))
    log "清理 $dir 下最旧 $drop 个版本（保留最近 $KEEP 个）"
    local i old newest=${sorted[$((nver - 1))]}
    for ((i = 0; i < drop; i++)); do
        old=${sorted[$i]}
        # 最新版本永远不删，KEEP 被人设成 0 也不能删到当次构建。
        [[ "$old" == "$newest" ]] && continue
        log "  删除 $old"
        find "$dir" -maxdepth 1 -type f \
            \( -name "*-${old}-*" -o -name "*-${old}.*" -o -name "*_${old}-*" -o -name "*_${old}.*" \) \
            -exec rm -f -- {} +
    done
    return 0
}

# --------------------------------------------------------------------------- #
# APT
# --------------------------------------------------------------------------- #
do_apt() {
    local debs; shopt -s nullglob; debs=("$SRC"/asus-numpad_*.deb); shopt -u nullglob
    ((${#debs[@]})) || { log "跳过 apt（没有 .deb）"; return 0; }

    log "布置 apt 仓库"
    local pool="$OUT/apt/pool/main/a/asus-numpad"
    mkdir -p "$pool" "$OUT/apt/dists/stable/main"
    cp -f "${debs[@]}" "$pool/"
    prune "$pool"

    # 按 deb 架构分目录；apt 靠它决定给谁下发。
    # 不单独做 binary-all：Architecture: all 的包 dpkg-scanpackages 会算进各具体架构，
    # 再额外来一份 binary-all 只会让 apt 拿到重复条目。
    local arch listed=0
    for a in amd64 arm64 armhf; do
        local dir="$OUT/apt/dists/stable/main/binary-$a"
        mkdir -p "$dir"
        ( cd "$OUT/apt" && dpkg-scanpackages --multiversion --arch "$a" pool/main 2>/dev/null ) > "$dir/Packages" || true
        if [[ -s "$dir/Packages" ]]; then
            gzip -9c "$dir/Packages" > "$dir/Packages.gz"
            listed=1
        else
            rm -rf "$dir"
        fi
    done
    ((listed)) || warn "apt：dpkg-scanpackages 没扫出包（工具缺失？）"

    # Release：不签名的仓库，apt 用 [trusted=yes] 绕过 GPG；校验值仍然写全，
    # 这样即使以后加签名也不用重做结构
    local rel="$OUT/apt/dists/stable/Release"

    # 只声明真扫出包来的架构。声明了却没有对应索引的架构，apt 在本机架构上不报错，
    # 但换到那个架构的机器上就是 404。
    local archs="" a2
    for a2 in amd64 arm64 armhf; do
        [[ -s "$OUT/apt/dists/stable/main/binary-$a2/Packages" ]] && archs="$archs $a2"
    done
    archs="${archs# }"

    {
        printf 'Origin: asus-numpad\n'
        printf 'Label: asus-numpad\n'
        printf 'Suite: stable\n'
        printf 'Codename: stable\n'
        printf 'Version: %s\n' "$VERSION"
        printf 'Architectures: %s\n' "${archs:-amd64}"
        printf 'Components: main\n'
        printf 'Description: ASUS NumberPad userspace driver for Linux\n'
        printf 'Date: %s\n' "$(LC_ALL=C date -u '+%a, %d %b %Y %H:%M:%S UTC')"
        # 路径必须相对 dists/stable/。以前写成 dists/stable/main/...，apt 认不出这是
        # 本仓库索引，update 静默跳过（退出码 0、无任何输出），装包时才报
        # E: Unable to locate package asus-numpad。
        printf 'SHA256:\n'
        ( cd "$OUT/apt/dists/stable" && find main -name 'Packages*' -type f | sort | while read -r f; do
            printf ' %s %s %s\n' "$(sha256sum "$f" | cut -d' ' -f1)" "$(wc -c < "$f" | tr -d ' ')" "$f"
        done )
        printf 'MD5Sum:\n'
        ( cd "$OUT/apt/dists/stable" && find main -name 'Packages*' -type f | sort | while read -r f; do
            printf ' %s %s %s\n' "$(md5sum "$f" | cut -d' ' -f1)" "$(wc -c < "$f" | tr -d ' ')" "$f"
        done )
    } > "$rel"
    gzip -9c "$rel" > "$rel.gz"

    # 签的必须是「指回 Packages.gz 的那份 Release」，所以顺序只能是先写 Release 再签。
    # InRelease 是内嵌签名（新 apt 优先用），Release.gpg 是分离签名（老 apt 兜底），
    # 两个都得出，缺一个都会让一部分发行版退回 trusted=yes 的老路子。
    if ((GPG_SIG)); then
        log "签名 apt Release"
        rm -f "$rel.gpg" "$OUT/apt/dists/stable/InRelease"
        gpg_detach "$rel" "$rel.gpg"
        gpg_clear  "$rel" "$OUT/apt/dists/stable/InRelease"
        APT_SIGNED=1
    fi
}

# --------------------------------------------------------------------------- #
# RPM (dnf / yum / zypper)
# --------------------------------------------------------------------------- #
do_rpm() {
    shopt -s nullglob
    local rpms=("$SRC"/asus-numpad-*.rpm)
    shopt -u nullglob
    ((${#rpms[@]})) || { log "跳过 rpm（没有 .rpm）"; return 0; }

    # 包签名必须早于 createrepo：repomd 记录的是包的头/校验和，
    # 先建索引后签名会让 dnf 拿到「元数据对得上但包没签」的组合。
    if ((GPG_SIG)); then
        local signer=""
        if have rpmsign; then signer=rpmsign
        elif have rpm; then signer=rpm
        fi
        if [[ -n "$signer" ]]; then
            log "给 rpm 包加签名（${signer}）"
            if "$signer" --define "_gpg_name $GPG_KEY" \
                         --define "_gpg_path ${GNUPGHOME:-$HOME/.gnupg}" \
                         --addsign "${rpms[@]}" >/dev/null 2>&1; then
                RPM_PKG_SIGNED=1
            else
                warn "rpm --addsign 失败：rpm 源只能给 gpgcheck=0"
            fi
        else
            warn "缺少 rpm/rpmsign，跳过包签名：rpm 源只能给 gpgcheck=0"
        fi
    fi

    local arch
    for r in "${rpms[@]}"; do
        arch="$(basename "$r")"
        case "$arch" in
            *.x86_64.rpm)  arch=x86_64 ;;
            *.aarch64.rpm) arch=aarch64 ;;
            *)             arch=x86_64 ;;
        esac
        local dir="$OUT/rpm/$arch"
        mkdir -p "$dir"
        cp -f "$r" "$dir/"
    done

    for d in "$OUT"/rpm/*/; do
        [[ -d "$d" ]] || continue
        prune "$d"
        if have createrepo_c; then
            log "生成 rpm repodata: $(basename "$d")"
            createrepo_c --quiet --update --database "$d"
        elif have createrepo; then
            log "生成 rpm repodata（createrepo）: $(basename "$d")"
            createrepo --quiet --update --database "$d"
        else
            warn "缺少 createrepo_c，跳过 $(basename "$d") 的元数据（apt install createrepo-c）"
            continue
        fi
        # repomd.xml.asc 是 dnf 校验「元数据本身」的签名文件：没有它，
        # 即使包装了签名、写 repo_gpgcheck=1 也会直接报错。
        if ((GPG_SIG)) && [[ -f "$d/repodata/repomd.xml" ]]; then
            log "签名 repomd.xml: $(basename "$d")"
            rm -f "$d/repodata/repomd.xml.asc"
            gpg_asc "$d/repodata/repomd.xml" "$d/repodata/repomd.xml.asc"
            RPM_REPO_SIGNED=1
        fi
    done

    # 只有「包签了 + 元数据签了」才能让用户开 gpgcheck=1 / repo_gpgcheck=1
    if ((RPM_PKG_SIGNED && RPM_REPO_SIGNED)); then RPM_SIGNED=1; fi
}

# --------------------------------------------------------------------------- #
# Pacman
# --------------------------------------------------------------------------- #
do_arch() {
    shopt -s nullglob
    local pkgs=("$SRC"/asus-numpad-*.pkg.tar.*)
    shopt -u nullglob
    ((${#pkgs[@]})) || { log "跳过 arch（没有 .pkg.tar.*）"; return 0; }

    # pacman 的包签名是同目录下的 <包名>.sig。repo-add 只读包本身，
    # 所以 .sig 先于建库生成、并跟着包一起搬进仓库目录即可。
    if ((GPG_SIG)); then
        log "给 pacman 包签 .sig"
        local p
        for p in "${pkgs[@]}"; do
            if [[ "$p" == *.sig ]]; then continue; fi
            rm -f "$p.sig"
            gpg_detach "$p" "$p.sig"
        done
        ARCH_PKG_SIGNED=1
    fi

    local arch
    for p in "${pkgs[@]}"; do
        if [[ "$p" == *.sig ]]; then continue; fi
        arch="$(basename "$p")"
        case "$arch" in
            *.aarch64.*) arch=aarch64 ;;
            *)           arch=x86_64 ;;
        esac
        local dir="$OUT/arch/$arch"
        mkdir -p "$dir"
        cp -f "$p" "$dir/"
        if ((GPG_SIG)); then cp -f "$p.sig" "$dir/"; fi
    done

    for d in "$OUT"/arch/*/; do
        [[ -d "$d" ]] || continue
        prune "$d"
        ( cd "$d" && rm -f asus-numpad.db* asus-numpad.files* )
        if have repo-add; then
            log "生成 pacman 数据库: $(basename "$d")"
            local files=() mains=0
            while IFS= read -r f; do
                files+=("$(basename "$f")")
                [[ "${f##*/}" == asus-numpad-debug-* ]] || mains=$((mains + 1))
            done < <(find "$d" -maxdepth 1 -name 'asus-numpad-*.pkg.tar.zst' ! -name '*.sig' | sort)
            # 只剩 debug 包说明 prune 把正式包删了，直接失败，不要等用户装不上才发现。
            ((mains)) || die "pacman 仓库里没有正式包（只剩 debug 或为空）: $d"
            ( cd "$d" && repo-add -q asus-numpad.db.tar.gz "${files[@]}" >/dev/null )
            if ((GPG_SIG)); then
                # 不用 repo-add --sign：CI 里 repo-add 是从 Arch 容器借来的，
                # 容器里既没有 gpg 也没有私钥。建完库由宿主机签，顺序也更稳——
                # 签完不能再动 db，否则 pacman 端一律 sig invalid。
                rm -f "$d/asus-numpad.db.tar.gz.sig" "$d/asus-numpad.db.sig" "$d/asus-numpad.files.sig"
                if gpg_detach "$d/asus-numpad.db.tar.gz" "$d/asus-numpad.db.tar.gz.sig"; then
                    # pacman -Sy 取的是「库名 + .sig」，也就是 asus-numpad.db.sig；
                    # 只签 db.tar.gz.sig 的话它请求会 404，直接
                    # failed to synchronize all databases。db / files 本来就是
                    # repo-add 建的符号链接，这里同规格指过去，共用同一份签名。
                    for base_name in asus-numpad.db asus-numpad.files; do
                        [[ -e "$d/$base_name.tar.gz.sig" ]] \
                            && ln -sf "$base_name.tar.gz.sig" "$d/$base_name.sig"
                    done
                    ARCH_DB_SIGNED=1
                else
                    warn "签名 pacman 数据库失败：pacman 源只能 Optional TrustAll"
                fi
            fi
        else
            warn "缺少 repo-add（pacman），跳过 $(basename "$d") 的数据库"
        fi
    done

    if ((ARCH_PKG_SIGNED && ARCH_DB_SIGNED)); then ARCH_SIGNED=1; fi
}

# --------------------------------------------------------------------------- #
# Alpine (apk)
# --------------------------------------------------------------------------- #
do_alpine() {
    shopt -s nullglob
    local apks=("$SRC"/*.apk)
    shopt -u nullglob
    ((${#apks[@]})) || { log "跳过 alpine（没有 .apk）"; return 0; }

    # apk 的签名是追加在包尾的独立段，会改变文件内容 → 必须签完再 apk index，
    # 否则索引里的 csum 指向的是未签名那份，装的时候校验直接对不上。
    if ((APK_SIG)); then
        log "给 apk 包签名"
        local a
        for a in "${apks[@]}"; do
            abuild-sign -k "$APK_KEY" "$a" || die "apk 包签名失败: $(basename "$a")"
        done
        APK_PKG_SIGNED=1
    fi

    for a in "${apks[@]}"; do
        local arch
        case "$(basename "$a")" in
            *aarch64*) arch=aarch64 ;;
            *)         arch=x86_64 ;;
        esac
        local dir="$OUT/alpine/$arch"
        mkdir -p "$dir"
        cp -f "$a" "$dir/"
    done

    for d in "$OUT"/alpine/*/; do
        [[ -d "$d" ]] || continue
        prune "$d"
        if have apk; then
            log "生成 APKINDEX: $(basename "$d")"
            # 不传 --rewrite-arch：apk-tools 3（Alpine 3.22 起）删掉了这个选项，
            # 包的架构本来就写在 .PKGINFO 里，索引自己会读，用不着改写
            local out
            if ! out=$( cd "$d" && apk index --allow-untrusted -o APKINDEX.tar.gz ./*.apk 2>&1 ); then
                printf '%s\n' "$out" | tail -n 5 >&2
                warn "apk index 失败（$(basename "$d")）"
                return 1
            fi
            # 索引本身也要签：apk update 会拿 /etc/apk/keys 里的公钥校 APKINDEX.tar.gz，
            # 只签包不签索引，用户端在 trust 这一步就过不去
            if ((APK_SIG)); then
                log "签名 APKINDEX: $(basename "$d")"
                ( cd "$d" && abuild-sign -k "$APK_KEY" APKINDEX.tar.gz ) \
                    || die "APKINDEX 签名失败（$(basename "$d")）"
                APK_IDX_SIGNED=1
            fi
        else
            warn "缺少 apk 工具，跳过 $(basename "$d") 的索引（apk-tools）"
        fi
    done

    if ((APK_PKG_SIGNED && APK_IDX_SIGNED)); then APK_SIGNED=1; fi
}

do_apt
do_rpm
do_arch
do_alpine

# --------------------------------------------------------------------------- #
# 公钥：随仓库一起分发，用户 curl 一个文件就够，不必去别处找
# --------------------------------------------------------------------------- #
KEYS_SRC="$ROOT/tools/keys"
if [[ -d "$KEYS_SRC" ]]; then
    mkdir -p "$OUT/keys"
    for f in "$KEYS_SRC"/*; do
        [[ -f "$f" ]] || continue
        # 私钥绝不允许跟着仓库走：拦在这里，比事后从 git 历史里擦干净便宜
        case "${f##*/}" in
            *.secret.*|*secret*|*private*) warn "跳过 ${f##*/}：看着像私钥，不进仓库"; continue ;;
        esac
        cp -f "$f" "$OUT/keys/"
    done
    if ((GPG_SIG)); then
        [[ -f "$OUT/keys/asus-numpad-repo.asc" ]] \
            || warn "签名开着，但 tools/keys/ 里没有 asus-numpad-repo.asc：用户拿不到公钥"
    fi
    if ((APK_SIG)); then
        [[ -f "$OUT/keys/asus-numpad-apk.rsa.pub" ]] \
            || warn "apk 签名开着，但 tools/keys/ 里没有 asus-numpad-apk.rsa.pub"
    fi
    log "公钥已随仓库发布: $(ls "$OUT/keys" | tr '\n' ' ')"
else
    warn "没有 tools/keys/，仓库里不会带公钥"
fi

# --------------------------------------------------------------------------- #
# 人看的入口页
# --------------------------------------------------------------------------- #
# 安装命令按「这个生态到底签成没签」生成，绝不给用户一条必然失败的命令。
log "生成 index.html"

if ((APT_SIGNED)); then
APT_CMD="sudo install -d -m 0755 /usr/share/keyrings
curl -fsSL ${BASE_URL}/keys/asus-numpad-repo.gpg \\
  | sudo tee /usr/share/keyrings/asus-numpad.gpg >/dev/null
echo \"deb [signed-by=/usr/share/keyrings/asus-numpad.gpg] ${BASE_URL}/apt stable main\" \\
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update && sudo apt install asus-numpad"
else
APT_CMD="echo \"deb [trusted=yes] ${BASE_URL}/apt stable main\" \\
  | sudo tee /etc/apt/sources.list.d/asus-numpad.list
sudo apt update && sudo apt install asus-numpad"
fi

if ((RPM_SIGNED)); then
RPM_CMD="sudo rpm --import ${BASE_URL}/keys/asus-numpad-repo.asc
sudo tee /etc/yum.repos.d/asus-numpad.repo <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=${BASE_URL}/rpm/\$basearch
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=${BASE_URL}/keys/asus-numpad-repo.asc
EOF
sudo dnf install asus-numpad"
else
RPM_CMD="sudo tee /etc/yum.repos.d/asus-numpad.repo <<'EOF'
[asus-numpad]
name=asus-numpad
baseurl=${BASE_URL}/rpm/\$basearch
enabled=1
gpgcheck=0
EOF
sudo dnf install asus-numpad"
fi

if ((ARCH_SIGNED)); then
ARCH_CMD="curl -fsSL ${BASE_URL}/keys/asus-numpad-repo.asc -o /tmp/asus-numpad.asc
sudo pacman-key --add /tmp/asus-numpad.asc
sudo pacman-key --lsign-key ${GPG_FPR}
sudo tee -a /etc/pacman.conf <<'EOF'

[asus-numpad]
SigLevel = Required DatabaseRequired
Server = ${BASE_URL}/arch/\$arch
EOF
sudo pacman -Sy asus-numpad"
else
ARCH_CMD="sudo tee -a /etc/pacman.conf <<'EOF'

[asus-numpad]
SigLevel = Optional TrustAll
Server = ${BASE_URL}/arch/\$arch
EOF
sudo pacman -Sy asus-numpad"
fi

if ((APK_SIGNED)); then
APK_CMD="curl -fsSL ${BASE_URL}/keys/asus-numpad-apk.rsa.pub \\
  | sudo tee /etc/apk/keys/asus-numpad-apk.rsa.pub >/dev/null
echo \"${BASE_URL}/alpine\" | sudo tee -a /etc/apk/repositories
sudo apk update && sudo apk add asus-numpad"
else
APK_CMD="echo \"${BASE_URL}/alpine\" \\
  | sudo tee -a /etc/apk/repositories
sudo apk update && sudo apk add --allow-untrusted asus-numpad"
fi

if ((APT_SIGNED || RPM_SIGNED || ARCH_SIGNED || APK_SIGNED)); then
SIGN_LINE="所有源用同一把 GPG 密钥签名，指纹 <code>${GPG_FPR}</code>；公钥在 <code>keys/</code> 下，下面每条命令都已含导入公钥这一步。"
else
SIGN_LINE="仓库未做 GPG 签名，命令里因此带 <code>trusted=yes</code> / <code>gpgcheck=0</code> / <code>--allow-untrusted</code>；介意的话请到 Releases 下 .deb/.rpm/.pkg.tar.zst/.apk 手工装。"
fi

cat > "$OUT/index.html" <<HTML
<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>asus-numpad ${VERSION} 软件源</title>
<style>
  :root { color-scheme: light dark; }
  body { font: 15px/1.65 -apple-system, "Segoe UI", "Noto Sans SC", sans-serif;
         max-width: 46rem; margin: 3rem auto; padding: 0 1.2rem; }
  h1 { font-size: 1.5rem; margin-bottom: .2rem; }
  .sub { opacity: .65; margin-top: 0; }
  h2 { font-size: 1.05rem; margin-top: 2.2rem; border-bottom: 1px solid #8884; padding-bottom: .3rem; }
  pre { background: #8881; padding: .8rem 1rem; border-radius: 6px; overflow-x: auto; font-size: 13px; }
  code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  table { border-collapse: collapse; width: 100%; font-size: 14px; }
  th, td { text-align: left; padding: .35rem .6rem; border-bottom: 1px solid #8883; }
  .fp { font-size: 12px; word-break: break-all; opacity: .7; }
</style>
</head>
<body>
<h1>asus-numpad ${VERSION}</h1>
<p class="sub">ASUS 触摸板数字小键盘（NumberPad）的 Linux 用户态驱动 · GPL-3.0-or-later</p>

<p>本页是软件源根地址：<code>${BASE_URL}</code>。${SIGN_LINE}</p>
<p class="fp">签名指纹：${GPG_FPR:-无（未签名仓库）}</p>

<h2>一条命令（导入公钥 + 写源 + 安装）</h2>
<p>脚本自己认发行版，按上面的方式写源再装，重复运行不会重复追加；只想写源不装就加 <code>--repo-only</code>。</p>
<pre><code>curl -fsSL ${BASE_URL}/install.sh | sudo sh</code></pre>

<h2>Debian / Ubuntu / Mint</h2>
<pre><code>${APT_CMD//&/&amp;}</code></pre>

<h2>Fedora / RHEL / Rocky / openSUSE</h2>
<pre><code>${RPM_CMD//&/&amp;}</code></pre>

<h2>Arch / Manjaro</h2>
<pre><code>${ARCH_CMD//&/&amp;}</code></pre>

<h2>Alpine</h2>
<pre><code>${APK_CMD//&/&amp;}</code></pre>

<h2>Nix / NixOS</h2>
<pre><code>nix run github:asus-numpad/asus-numpad
# NixOS: services.asus-numpad.enable = true;</code></pre>

<h2>其它发行版</h2>
<p>用 Release 里的通用 tarball（musl 静态链接，任意 x86_64 Linux 可跑）：</p>
<pre><code>tar xzf asus-numpad-${VERSION}-x86_64.tar.gz
cd asus-numpad-${VERSION} &amp;&amp; sudo ./install.sh</code></pre>

<h2>内容</h2>
<table>
<tr><th>路径</th><th>说明</th></tr>
<tr><td><code>apt/</code></td><td>APT 仓库（dists/stable + pool）</td></tr>
<tr><td><code>rpm/&lt;arch&gt;/</code></td><td>RPM 仓库（含 repodata）</td></tr>
<tr><td><code>arch/&lt;arch&gt;/</code></td><td>Pacman 仓库（asus-numpad.db）</td></tr>
<tr><td><code>alpine/&lt;arch&gt;/</code></td><td>Alpine 仓库（APKINDEX）</td></tr>
<tr><td><code>keys/</code></td><td>公钥：apt 用 .gpg、dnf 与 pacman 用 .asc、apk 用 .rsa.pub</td></tr>
</table>
<p class="sub">每个生态只保留最近 ${KEEP} 个版本；更早的版本仍可从 GitHub Releases 取。</p>
</body>
</html>
HTML

# --------------------------------------------------------------------------- #
# 一键安装脚本：把「导入公钥 + 写源 + 安装」三步合成一条命令。
#    curl -fsSL <base>/install.sh | sudo sh
# 模板里 @@XXX@@ 是生成期替换的占位符，其余 $BASE/$BASE_URL 等原样留给运行期。
# --------------------------------------------------------------------------- #
if ((APT_SIGNED && RPM_SIGNED && ARCH_SIGNED && APK_SIGNED)); then
    INST_SIGNED=1
else
    INST_SIGNED=0
fi

cat > "$OUT/install.sh" <<'INSTALL_SH'
#!/bin/sh
# asus-numpad 一键安装：导入公钥 → 写入软件源 → 安装 → 版本自检
#
#   curl -fsSL @@BASE_URL@@/install.sh | sudo sh
#
# 换仓库地址（镜像、本地目录等）：
#   curl -fsSL @@BASE_URL@@/install.sh | sudo ASUS_NUMPAD_BASE=https://... sh
#
# 只写源不装：上面命令末尾加 --repo-only
set -eu

BASE="${ASUS_NUMPAD_BASE:-@@BASE_URL@@}"
SIGNED=@@SIGNED@@
FPR="@@GPG_FPR@@"
PKG=asus-numpad
MODE="${1:-install}"

log() { printf '==> %s\n' "$*"; }
die() { printf '错误: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

fetch() {
    if have curl; then curl -fsSL "$1"
    elif have wget; then wget -qO- "$1"
    else die "需要 curl 或 wget"
    fi
}

[ "$(id -u)" = 0 ] || die "需要 root：curl -fsSL $BASE/install.sh | sudo sh"

install_deb() {
    log "Debian 系：写 apt 源"
    install -d -m 0755 /usr/share/keyrings
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.gpg" > /usr/share/keyrings/asus-numpad.gpg
        echo "deb [signed-by=/usr/share/keyrings/asus-numpad.gpg] $BASE/apt stable main" \
            > /etc/apt/sources.list.d/asus-numpad.list
    else
        echo "deb [trusted=yes] $BASE/apt stable main" \
            > /etc/apt/sources.list.d/asus-numpad.list
    fi
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    apt-get update -q
    apt-get install -y "$PKG"
}

install_rpm() {
    mgr="$1"
    log "$mgr：写 rpm 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.asc" > /tmp/asus-numpad.asc
        rpm --import /tmp/asus-numpad.asc
    fi
    {
        echo "[asus-numpad]"
        echo "name=asus-numpad"
        echo "baseurl=$BASE/rpm/\$basearch"
        echo "enabled=1"
        if [ "$SIGNED" = 1 ]; then
            echo "gpgcheck=1"
            echo "repo_gpgcheck=1"
            echo "gpgkey=$BASE/keys/asus-numpad-repo.asc"
        else
            echo "gpgcheck=0"
            echo "repo_gpgcheck=0"
        fi
    } > /etc/yum.repos.d/asus-numpad.repo
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    "$mgr" install -y "$PKG"
}

install_zypper() {
    log "openSUSE：写 zypper 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.asc" > /tmp/asus-numpad.asc
        rpm --import /tmp/asus-numpad.asc
    fi
    {
        echo "[asus-numpad]"
        echo "name=asus-numpad"
        echo "enabled=1"
        echo "autorefresh=1"
        echo "type=rpm-md"
        echo "baseurl=$BASE/rpm/\$basearch"
        if [ "$SIGNED" = 1 ]; then
            echo "gpgcheck=1"
            echo "repo_gpgcheck=1"
            echo "gpgkey=$BASE/keys/asus-numpad-repo.asc"
        else
            echo "gpgcheck=0"
            echo "repo_gpgcheck=0"
        fi
    } > /etc/zypp/repos.d/asus-numpad.repo
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    zypper --gpg-auto-import-keys --non-interactive refresh asus-numpad
    zypper --non-interactive install "$PKG"
}

install_pacman() {
    log "Arch 系：写 pacman 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-repo.asc" > /tmp/asus-numpad.asc
        pacman-key --add /tmp/asus-numpad.asc
        pacman-key --lsign-key "$FPR"
    fi
    if ! grep -q '^\[asus-numpad\]' /etc/pacman.conf; then
        {
            echo ""
            echo "[asus-numpad]"
            if [ "$SIGNED" = 1 ]; then
                echo "SigLevel = Required DatabaseRequired"
            else
                echo "SigLevel = Optional TrustAll"
            fi
            echo "Server = $BASE/arch/\$arch"
        } >> /etc/pacman.conf
    fi
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    pacman -Sy --noconfirm "$PKG"
}

install_apk() {
    log "Alpine：写 apk 源"
    if [ "$SIGNED" = 1 ]; then
        fetch "$BASE/keys/asus-numpad-apk.rsa.pub" > /etc/apk/keys/asus-numpad-apk.rsa.pub
    fi
    if ! grep -qxF "$BASE/alpine" /etc/apk/repositories; then
        echo "$BASE/alpine" >> /etc/apk/repositories
    fi
    if [ "$MODE" = "--repo-only" ]; then
        log "已写入源，未安装"
        return 0
    fi
    apk update
    if [ "$SIGNED" = 1 ]; then
        apk add "$PKG"
    else
        apk add --allow-untrusted "$PKG"
    fi
}

if have zypper; then
    install_zypper
elif have dnf; then
    install_rpm dnf
elif have yum; then
    install_rpm yum
elif have pacman; then
    install_pacman
elif have apk; then
    install_apk
elif have apt-get; then
    install_deb
else
    die "认不出包管理器，请按 $BASE/index.html 手工装"
fi

if [ "$MODE" = "--repo-only" ]; then
    exit 0
fi

log "装好了，自检："
"$PKG" --version || die "命令跑不起来，先看 dmesg 与 README 的排查段"
INSTALL_SH

sed -e "s|@@BASE_URL@@|$BASE_URL|g" \
    -e "s|@@SIGNED@@|$INST_SIGNED|g" \
    -e "s|@@GPG_FPR@@|$GPG_FPR|g" \
    "$OUT/install.sh" > "$OUT/install.sh.new"
mv -f "$OUT/install.sh.new" "$OUT/install.sh"
chmod 0755 "$OUT/install.sh"
log "一键安装脚本: $OUT/install.sh（签名=$INST_SIGNED）"

log "仓库已生成于 $OUT"
find "$OUT" -maxdepth 2 -type d | sort | sed 's/^/  /'
if ((APT_SIGNED || RPM_SIGNED || ARCH_SIGNED || APK_SIGNED)); then
    log "签名状态：apt=$APT_SIGNED rpm=$RPM_SIGNED pacman=$ARCH_SIGNED apk=$APK_SIGNED"
fi
