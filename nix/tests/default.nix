{ pkgs, extraBaseModules }:

let
  runTest =
    module:
    pkgs.testers.runNixOSTest {
      imports = [ module ];
      globalTimeout = 5 * 60;
      extraBaseModules = {
        imports = builtins.attrValues extraBaseModules;
      };
    };

  # Run the test only on the specified systems. Otherwise build hello to work
  # around flake behaviour.
  runTestOn =
    systems: module: if builtins.elem pkgs.system systems then runTest module else pkgs.hello;
in
{
  basic = runTest ./lanzaboote/basic.nix;
  systemd-initrd = runTest ./lanzaboote/systemd-initrd.nix;
  initrd-secrets = runTest ./lanzaboote/initrd-secrets.nix;
  initrd-secrets-update = runTest ./lanzaboote/initrd-secrets-update.nix;
  hash-mismatch-initrd = runTest ./lanzaboote/hash-mismatch-initrd.nix;
  hash-mismatch-initrd-sb = runTest ./lanzaboote/hash-mismatch-initrd-sb.nix;
  hash-mismatch-kernel = runTest ./lanzaboote/hash-mismatch-kernel.nix;
  hash-mismatch-kernel-sb = runTest ./lanzaboote/hash-mismatch-kernel-sb.nix;
  specialisation = runTest ./lanzaboote/specialisation.nix;
  synthesis = runTestOn [ "x86_64-linux" ] ./lanzaboote/synthesis.nix;
  systemd-boot-loader-config = runTest ./lanzaboote/systemd-boot-loader-config.nix;
  export-efivars = runTest ./lanzaboote/export-efivars.nix;
  export-efivars-tpm = runTest ./lanzaboote/export-efivars-tpm.nix;

  systemd-pcrlock = runTest ./lanzaboote/systemd-pcrlock.nix;
  systemd-measured-uki = runTest ./lanzaboote/systemd-measured-uki.nix;
}
