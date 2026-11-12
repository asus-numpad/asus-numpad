{ config, lib, pkgs, ... }:

# NixOS 模块：services.numdeck.enable = true;
#
# 做三件事：
#   1. 装包（packaging/nix/package.nix）
#   2. 装 udev 规则，让普通用户拿到 /dev/hidraw* 的 uaccess ACL
#   3. 起一个用户级 systemd 单元，随会话自动运行
#
# 亮度档位等参数直接写配置文件，不需要手改 /etc 下的 JSON。

let
  cfg = config.services.numdeck;
  # 配置由 Nix 生成：改它等于改系统配置，避免 /etc 与 Nix 两处真相
  settingsFile = builtins.toFile "numdeck.json" (builtins.toJSON cfg.settings);
in
{
  options.services.numdeck = {
    enable = lib.mkEnableOption "触摸板数字键盘驱动（NumberPad 类）";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ./package.nix { }";
      description = "numdeck 包。";
    };

    settings = lib.mkOption {
      type = lib.types.attrs;
      default = { };
      example = {
        numlock = "sync";
        manual_brightness_step = 4;
        manual_brightness_levels = [ 0x41 0x43 0x46 0x48 ];
      };
      description = ''
        写入配置文件的键值对，对应 `numdeck --dump-config` 里的字段。
        值按 JSON 编码。
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # udev 规则来自包内；NixOS 的 services.udev.packages 会自动拼接
    services.udev.packages = [ cfg.package ];

    # 语言文件不在 /usr/share 下，显式指路（二进制内置一份兜底，这里给可覆盖副本）
    environment.sessionVariables.NUMDECK_LANG_DIR =
      "${cfg.package}/share/numdeck/lang";

    systemd.user.services.numdeck = {
      description = "触摸板数字键盘驱动（NumberPad 类）";
      wantedBy = [ "default.target" ];
      partOf = [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/numdeck --config ${settingsFile} run";
        Restart = "on-failure";
        RestartSec = 3;
        # 触摸板设备与背光节点都在用户会话里，不需要额外权限
        NoNewPrivileges = true;
        PrivateTmp = true;
      };
    };
  };
}
