{ lib, rustPlatform, installShellFiles }:

rustPlatform.buildRustPackage {
  pname = "asus-numpad";
  # 版本号与 Cargo.toml 保持一致；改动版本时只需改这一处
  version = "1.0.0";

  src = ../.;

  # 依赖版本由仓库内的 Cargo.lock 锁定，不需要 cargoHash
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];

  # libc 是 Linux 专属依赖，其它平台构建逻辑层时会被 Cargo 自动跳过
  doCheck = true;

  postInstall = ''
    install -Dm0644 packaging/common/asus-numpad.1 $out/share/man/man1/asus-numpad.1
    install -Dm0644 packaging/common/99-asus-numpad.rules \
      $out/lib/udev/rules.d/99-asus-numpad.rules
    install -Dm0644 packaging/common/asus-numpad.service \
      $out/lib/systemd/user/asus-numpad.service

    # 语言文件：二进制内置一份，这里放可覆盖副本
    install -d $out/share/asus-numpad/lang
    install -m0644 lang/zh_CN.lang lang/zh_TW.lang lang/en.lang \
      $out/share/asus-numpad/lang/
  '';

  meta = {
    description = "ASUS 触摸板数字小键盘（NumberPad）Linux 用户态驱动";
    homepage = "https://github.com/asus-numpad/asus-numpad";
    license = lib.licenses.gpl3Plus;
    maintainers = [ ];
    mainProgram = "asus-numpad";
    platforms = [ "x86_64-linux" "aarch64-linux" ];
  };
}
