{ pkgs, runTest, lanzabooteModule }:

let
  efiSecurityImageGuid = "d719b2cb-3d3a-4596-a3bc-dad00e67656f";
  setSecurityEfiVariable = out: variable: filepath: "${pkgs.ubootTools}/bin/efivar.py set -i ${out} -n ${variable} --attrs nv,bs,rt,at --type file --guid ${efiSecurityImageGuid} --data ${filepath}";
  common = _: {
    imports = [ lanzabooteModule ];

    virtualisation = {
      useBootLoader = true;
      useEFIBoot = true;
      firmware = "uboot";
      uboot.efiVariableSeed =
        let
          setVar = setSecurityEfiVariable "$out";
        in
        # ESL are EFI Signatures List
          # i.e. list of PKs, KEKs, db(s), either hashes or certificates.
          # they are constructed using `efisecdb` (from `efivar`) which can manipulate them.
        pkgs.runCommand "prepare-efi-variable" { } ''
          ${setVar "PK" ./fixtures/uefi-keys/keys/PK/PK.esl}
          ${setVar "KEK" ./fixtures/uefi-keys/keys/KEK/KEK.esl}
          ${setVar "db" ./fixtures/uefi-keys/keys/db/db.esl}
        '';
    };

    boot.lanzaboote = {
      enable = true;
      pkiBundle = ./fixtures/uefi-keys;
      backend = "extlinux-compatible";
    };
    boot.loader.efi.canTouchEfiVariables = true;
  };
in
{
  # This test serves as a baseline to make sure that the custom boot installer
  # script defined in the ukiModule works with the upstream systemd-stub. When
  # this test fails something is very wrong.
  uboot-basic = runTest {
    name = "uboot-secureboot-basic";
    nodes.machine = _: {
      imports = [ common ];
    };
    testScript = ''
      machine.start()
      assert "Secure Boot: enabled (user)" in machine.succeed("bootctl status")
    '';
  };
}
