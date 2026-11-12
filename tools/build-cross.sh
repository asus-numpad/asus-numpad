#!/usr/bin/env bash
# 在 macOS / 任意主机上交叉构建 Windows 与 Linux 的 numdeck.exe / numdeck。
#
# 为什么要 zig：Rust 的 windows-gnu 目标要一个 MinGW 链接器，linux-gnu 目标要一个
# glibc 工具链。zig 自带这两套（`zig cc` 内含 mingw-w64 与 glibc），不用装
# mingw-w64、也不用起容器；cargo-zigbuild 负责把 Rust 的 `-nodefaultlibs`
# 链接参数翻译成 zig 认的形式。
#
#   tools/build-cross.sh              # 四个目标全建
#   tools/build-cross.sh windows      # x86_64 + ARM64 的 exe
#   tools/build-cross.sh linux        # x86_64 + ARM64 的 elf
#
# 产物：
#   target/x86_64-pc-windows-gnu/release/numdeck.exe
#   target/aarch64-pc-windows-gnullvm/release/numdeck.exe
#   target/x86_64-unknown-linux-gnu/release/numdeck
#   target/aarch64-unknown-linux-gnu/release/numdeck
#
# Linux 目标固定 glibc 2.17 作为下限，Fedora / Ubuntu 18.04 之后都能直接跑。
# ARM64 目标（RTX Spark、骁龙 X 一类的笔记本）与 x86_64 走同一条 zig 链路，
# 构建机不需要是 ARM 机器。
set -euo pipefail
cd "$(dirname "$0")/.."

ZIG_VERSION="${ZIG_VERSION:-0.17.1}"
GLIBC="${GLIBC:-2.17}"
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/numdeck-zig"

case "$(uname -m)" in
aarch64 | arm64) ZIG_ARCH=aarch64 ;;
x86_64 | amd64) ZIG_ARCH=x86_64 ;;
*) echo "不认识的架构：$(uname -m)" >&2; exit 1 ;;
esac
case "$(uname -s)" in
Darwin) ZIG_OS=macos ;;
Linux) ZIG_OS=linux ;;
*) echo "只支持 macOS / Linux 作为构建机（Windows 主机直接 cargo build 即可）" >&2; exit 1 ;;
esac

if ! command -v cargo-zigbuild >/dev/null; then
  echo "缺 cargo-zigbuild，正在安装……"
  cargo install cargo-zigbuild --locked
fi

ZIG_BIN="$CACHE/zig-$ZIG_ARCH-$ZIG_OS-$ZIG_VERSION/zig"
if [ ! -x "$ZIG_BIN" ]; then
  mkdir -p "$CACHE"
  tarball="$CACHE/zig-$ZIG_VERSION.tar.xz"
  url="https://ziglang.org/download/$ZIG_VERSION/zig-$ZIG_ARCH-$ZIG_OS-$ZIG_VERSION.tar.xz"
  echo "下载 $url"
  curl -fsSL --retry 3 -o "$tarball" "$url"
  tar -xf "$tarball" -C "$CACHE"
fi
export PATH="$(dirname "$ZIG_BIN"):$PATH"
echo "zig $("$ZIG_BIN" version)"

want="${1:-all}"
# Windows ARM64 用 gnullvm ABI：zig 自带 llvm-mingw，不必装 MSVC 工具链
if [ "$want" = all ] || [ "$want" = both ] || [ "$want" = windows ]; then
  echo "==> x86_64-pc-windows-gnu"
  cargo zigbuild --release --target x86_64-pc-windows-gnu --bin numdeck
  echo "==> aarch64-pc-windows-gnullvm"
  cargo zigbuild --release --target aarch64-pc-windows-gnullvm --bin numdeck
fi
if [ "$want" = all ] || [ "$want" = both ] || [ "$want" = linux ]; then
  echo "==> x86_64-unknown-linux-gnu (glibc >= $GLIBC)"
  cargo zigbuild --release --target "x86_64-unknown-linux-gnu.$GLIBC" --bin numdeck
  echo "==> aarch64-unknown-linux-gnu (glibc >= $GLIBC)"
  cargo zigbuild --release --target "aarch64-unknown-linux-gnu.$GLIBC" --bin numdeck
fi

for f in target/x86_64-pc-windows-gnu/release/numdeck.exe \
  target/aarch64-pc-windows-gnullvm/release/numdeck.exe \
  target/x86_64-unknown-linux-gnu/release/numdeck \
  target/aarch64-unknown-linux-gnu/release/numdeck; do
  [ -f "$f" ] && printf '%-58s %s\n' "$f" "$(du -h "$f" | cut -f1)"
done
