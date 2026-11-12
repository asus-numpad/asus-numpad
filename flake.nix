{
  description = "numdeck — ASUS 触摸板数字小键盘 Linux 驱动";

  # 不用 flake-utils：只支持 Linux 两个架构，手写 genAttrs 更少一层依赖。
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = eachSystem (pkgs: {
        default = pkgs.callPackage ./packaging/nix/package.nix { };
        numdeck = self.packages.${pkgs.system}.default;
      });

      # NixOS 模块：services.numdeck.enable = true;
      nixosModules.default = ./packaging/nix/module.nix;
      nixosModules.numdeck = self.nixosModules.default;

      formatter = eachSystem (pkgs: pkgs.nixpkgs-fmt);

      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rust-analyzer clippy rustfmt ];
        };
      });
    };
}
