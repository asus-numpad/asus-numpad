{ lib, rustPlatform, installShellFiles }:

rustPlatform.buildRustPackage {
  pname = "numdeck";
  # 版本号与 Cargo.toml 保持一致；改动版本时只需改这一处
  version = "1.0.0";

  src = ../.;

  # 依赖版本由仓库内的 Cargo.lock 锁定，不需要 cargoHash
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];

  # libc 是 Linux 专属依赖，其它平台构建逻辑层时会被 Cargo 自动跳过
  doCheck = true;

  postInstall = ''
    install -Dm0644 packaging/common/numdeck.1 $out/share/man/man1/numdeck.1
    install -Dm0644 packaging/common/99-numdeck.rules \
      $out/lib/udev/rules.d/99-numdeck.rules
    install -Dm0644 packaging/common/numdeck.service \
      $out/lib/systemd/user/numdeck.service

    # 语言文件：二进制内置一份，这里放可覆盖副本
    install -d $out/share/numdeck/lang
    install -m0644 lang/zh_CN.lang lang/zh_TW.lang lang/en.lang \
      $out/share/numdeck/lang/
  '';

  meta = {
    description = "触摸板数字小键盘（NumberPad 类）的 Linux 用户态驱动";
    homepage = "https://github.com/numdeck-dev/numdeck";
    license = lib.licenses.gpl3Plus;
    maintainers = [ ];
    mainProgram = "numdeck";
    platforms = [ "x86_64-linux" "aarch64-linux" ];
  };
}
