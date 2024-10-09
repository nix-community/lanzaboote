# This module introduces a simple boot loader installer that installs a UKI,
# leveraging bootspec. It is only designed to be useful in tests where
# rebuilding is unlikely/hard.

{ config, lib, pkgs, ... }:

let
  cfg = config.boot.loader.uki;
in
{
  options.boot.loader.uki = {
    enable = lib.mkEnableOption "UKI";

    stub = lib.mkOption {
      type = lib.types.path;
      description = "Path to the UKI stub to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    boot.bootspec.enable = true;
    boot.loader.external = {
      enable = true;
      installHook =
        let
          bootspecNamespace = ''"org.nixos.bootspec.v1"'';
          installer = pkgs.writeShellApplication {
            name = "install-uki";
            runtimeInputs = with pkgs; [ jq systemd binutils ];
            text = ''
              boot_json=/nix/var/nix/profiles/system-1-link/boot.json
              kernel=$(jq -r '.${bootspecNamespace}.kernel' "$boot_json")
              initrd=$(jq -r '.${bootspecNamespace}.initrd' "$boot_json")
              init=$(jq -r '.${bootspecNamespace}.init' "$boot_json")

              ${pkgs.systemdUkify}/lib/systemd/ukify \
                "$kernel" \
                "$initrd" \
                --stub=${cfg.stub} \
                --cmdline="init=$init ${builtins.toString config.boot.kernelParams}" \
                --os-release="@${config.system.build.etc}/etc/os-release" \
                --output=uki.efi

              esp=${config.boot.loader.efi.efiSysMountPoint}

              bootctl install --esp-path="$esp"
              install uki.efi "$esp"/EFI/Linux/
            '';
          };
        in
        "${lib.getExe installer}";
    };
  };
}
