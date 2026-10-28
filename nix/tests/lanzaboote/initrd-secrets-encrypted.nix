# Test that `boot.lanzaboote.encryptedSecrets.enable` appends only
# TPM-encrypted secrets to the initrd and that the initrd decrypts them before
# any consumer runs.
#
# The encrypted generation is installed at runtime because sealing against the
# TPM requires a TPM device, which is not available during a sandboxed build.
# The first generation of the machine keeps the plaintext appender and serves as
# a regression check for the unchanged code path.

{ pkgs, nodes, ... }:

let
  encryptedSecret = pkgs.writeText "oh-so-secure" "uhh-ooh-uhh-security";
  plaintextSecret = pkgs.writeText "plain-to-see" "definitely-readable";

  extraSystem = (import "${pkgs.path}/nixos/lib/eval-config.nix") {
    system = pkgs.stdenv.hostPlatform.system;
    modules = [
      "${pkgs.path}/nixos/modules/virtualisation/qemu-vm.nix"
      "${pkgs.path}/nixos/modules/testing/test-instrumentation.nix"
      (toString ./../../modules/lanzaboote.nix)
      ./common/lanzaboote.nix
      ({ ... }: {
        boot.lanzaboote.package = nodes.machine.boot.lanzaboote.package;
      })
      ({ ... }: {
        boot.loader.efi.efiSysMountPoint = "/boot";
        boot.loader.efi.canTouchEfiVariables = true;
        boot.loader.timeout = 0;

        boot.lanzaboote = {
          enable = true;
          pkiBundle = "/var/lib/lanzaboote-test-fixture";
          encryptedSecrets = {
            enable = true;
            privateKeyFile = "/boot/lanzaboote-test-secrets/private.pem";
            publicKeyFile = "/boot/lanzaboote-test-secrets/public.pem";
          };
        };

        boot.initrd.systemd.enable = true;
        boot.initrd.systemd.tpm2.enable = true;
        boot.initrd.systemd.tpm2.pcrphases.enable = true;
        testing.initrdBackdoor = true;
        # QEMU's mkVMOverride normally disables initrd-secret support.
        boot.loader.supportsInitrdSecrets = pkgs.lib.mkOverride 5 true;

        boot.initrd.secrets = {
          "/test" = toString encryptedSecret;
          "/plaintext" = toString plaintextSecret;
        };
        boot.initrd.systemd.storePaths = [
          "${pkgs.coreutils}/bin/cat"
        ];
        # Needed to modify the ESP from the initrd in the negative test.
        boot.initrd.availableKernelModules = [
          "vfat"
          "nls_cp437"
          "nls_iso8859-1"
        ];

        system.stateVersion = "25.05";
      })
    ];
  };

  extraToplevel = extraSystem.config.system.build.toplevel;
  extraInstaller = extraSystem.config.system.build.installBootLoader;

  extraInstallerPkg = pkgs.runCommand "lanzaboote-encrypted-secrets-installer" { } ''
    mkdir -p $out/bin
    ln -s ${extraInstaller} $out/bin/install-lanzaboote-with-encrypted-secrets
  '';
in
{
  name = "lanzaboote-initrd-secrets-encrypted";
  # Installing and booting three generations takes longer than the default.
  globalTimeout = pkgs.lib.mkForce (10 * 60);

  nodes.machine =
    { lib, ... }:
    {
      imports = [ ./common/lanzaboote.nix ];

      virtualisation.tpm.enable = true;
      virtualisation.mountHostNixStore = lib.mkForce true;
      testing.initrdBackdoor = true;
      boot.initrd.systemd.enable = true;
      boot.initrd.systemd.tpm2.enable = true;
      boot.initrd.systemd.tpm2.pcrphases.enable = true;
      boot.loader.supportsInitrdSecrets = lib.mkOverride 5 true;

      # The first generation keeps using the plaintext appender.
      boot.initrd.secrets = {
        "/plaintext" = toString plaintextSecret;
      };
      boot.initrd.systemd.storePaths = [
        "${pkgs.diffutils}/bin/cmp"
        plaintextSecret
      ];

      system.extraDependencies = [
        extraToplevel
        extraInstallerPkg
        encryptedSecret
        plaintextSecret
      ];
      environment.systemPackages = [
        extraInstallerPkg
        pkgs.bintools
        pkgs.coreutils
        pkgs.diffutils
        pkgs.findutils
        pkgs.gnugrep
        pkgs.openssl
        pkgs.zstd
      ];
    };

  testScript =
    { nodes, ... }:
    (import ./common/image-helper.nix { inherit (nodes) machine; })
    + ''
      with subtest("the plaintext appender still works for the first generation"):
        machine.succeed("test ! -e /.initrd-secrets/etc/systemd/tpm2-pcr-public-key.pem")
        machine.succeed("${pkgs.diffutils}/bin/cmp ${plaintextSecret} /plaintext")

      machine.switch_root()
      machine.wait_for_unit("default.target")

      with subtest("install a generation with encrypted initrd secrets at runtime"):
        machine.succeed("mkdir -p /nix/var/nix/profiles")
        machine.succeed("ln -sf ${extraToplevel} /nix/var/nix/profiles/system-2-link")
        machine.succeed(
          "${extraInstallerPkg}/bin/install-lanzaboote-with-encrypted-secrets ${extraToplevel}"
        )

      with subtest("the secrets key pair was generated"):
        machine.succeed("test -f /boot/lanzaboote-test-secrets/private.pem")
        machine.succeed("test -f /boot/lanzaboote-test-secrets/public.pem")
        # vfat derives the mode from the mount umask; only root may have access.
        mode = machine.succeed("stat -c '%a' /boot/lanzaboote-test-secrets/private.pem").strip()
        machine.succeed(f"test $(( 0{mode} & 077 )) -eq 0")
        assert (
          "2048 bit"
          in machine.succeed(
            "${pkgs.openssl}/bin/openssl pkey -in /boot/lanzaboote-test-secrets/private.pem -text -noout | ${pkgs.coreutils}/bin/head -n 1"
          ).strip()
        )
        machine.succeed(
          "${pkgs.diffutils}/bin/cmp /boot/lanzaboote-test-secrets/public.pem <(${pkgs.openssl}/bin/openssl pkey -in /boot/lanzaboote-test-secrets/private.pem -pubout)"
        )

      with subtest("the initrd contains ciphertext instead of plaintext"):
        image = machine.succeed(
          "${pkgs.findutils}/bin/find /boot/EFI/Linux -maxdepth 1 -type f -name 'nixos-generation-2-*.efi' | ${pkgs.coreutils}/bin/head -n 1",
        ).strip()
        assert image != "", "no generation-2 image found on the ESP"

        tmp = machine.succeed("${pkgs.coreutils}/bin/mktemp -d").strip()
        machine.succeed(
          f"${pkgs.bintools}/bin/objcopy -O binary --only-section=.initrd {image} {tmp}/initrd_path",
        )
        initrd_rel = (
          machine.succeed(f"${pkgs.coreutils}/bin/cat {tmp}/initrd_path")
          .strip()
          .replace("\\", "/")
          .lstrip("/")
        )
        initrd = f"/boot/{initrd_rel}"

        machine.succeed(f"${pkgs.zstd}/bin/zstd -dc {initrd} > {tmp}/initrd.unzst")
        machine.succeed(f"test -s {tmp}/initrd.unzst")
        machine.succeed(
          f"${pkgs.gnugrep}/bin/grep -a -q 'tpm2-pcr-public-key.pem' {tmp}/initrd.unzst"
        )
        machine.succeed(
          f"! ${pkgs.gnugrep}/bin/grep -a -q 'uhh-ooh-uhh-security' {tmp}/initrd.unzst"
        )
        machine.succeed(
          f"! ${pkgs.gnugrep}/bin/grep -a -q 'definitely-readable' {tmp}/initrd.unzst"
        )

        # The signature is stored as a global credential and the public key is in
        # the initrd for verification.
        machine.succeed(
          "${pkgs.findutils}/bin/find /boot/loader/credentials -name 'tpm2-pcr-signature.json.cred'"
        )
        machine.succeed(f"${pkgs.gnugrep}/bin/grep -a -q 'BEGIN PUBLIC KEY' {tmp}/initrd.unzst")

      with subtest("select the new generation and simulate a cold boot"):
        machine.succeed("bootctl set-default 'nixos-generation-2-*.efi'")
        machine.succeed("sync")
        machine.crash()

      machine.wait_for_unit("initrd.target")

      with subtest("the secrets were decrypted before the initrd target"):
        assert machine.succeed("${pkgs.coreutils}/bin/cat /test").strip() == "uhh-ooh-uhh-security"
        assert (
          machine.succeed("${pkgs.coreutils}/bin/cat /plaintext").strip() == "definitely-readable"
        )
        assert (
          machine.succeed("${pkgs.coreutils}/bin/cat /.initrd-secrets/test").strip()
          != "uhh-ooh-uhh-security"
        )
        machine.succeed("test -f /etc/systemd/tpm2-pcr-public-key.pem")
        machine.succeed("test -f /.extra/global_credentials/tpm2-pcr-signature.json.cred")

      with subtest("remove the PCR signature and cold boot again"):
        machine.succeed("mkdir -p /mnt")
        machine.succeed("mount /dev/disk/by-partlabel/esp /mnt")
        machine.succeed(
          "mv /mnt/loader/credentials/tpm2-pcr-signature.json.cred /mnt/loader/credentials/sig.bak"
        )
        machine.succeed("sync")
        machine.succeed("umount /mnt")
        machine.crash()

      machine.wait_for_unit("initrd.target")

      with subtest("without a matching signature the secrets stay ciphertext"):
        ciphertext = machine.succeed("${pkgs.coreutils}/bin/cat /.initrd-secrets/test").strip()
        assert machine.succeed("${pkgs.coreutils}/bin/cat /test").strip() == ciphertext
        assert ciphertext != "uhh-ooh-uhh-security"
    '';
}
