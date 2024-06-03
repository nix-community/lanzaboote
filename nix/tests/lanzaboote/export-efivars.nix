{ lib, pkgs, ... }:

let

  inherit (pkgs.stdenv.hostPlatform) efiArch;
  efiArchUppercased = lib.toUpper efiArch;

in

{
  name = "lanzaboote-export-efivars";

  nodes.machine = { pkgs, ... }: {
    imports = [ ./common/lanzaboote.nix ];
  };

  testScript = (import ./common/efivariables-helper.nix) + ''
    import struct

    # We will choose to boot directly on the stub.
    # To perform this trick, we will boot first with systemd-boot.
    # Then, we will add a new boot entry in EFI with higher priority
    # pointing to our stub.
    # Finally, we will reboot.
    # We will also assert that systemd-boot is not running
    # by checking for the sd-boot's specific EFI variables.
    machine.start()

    # By construction, nixos-generation-1.efi is the stub we are interested in.
    # TODO: this should work -- machine.succeed("efibootmgr -d /dev/vda -c -l \\EFI\\Linux\\nixos-generation-1.efi") -- efivars are not persisted
    # across reboots atm?
    # cheat code no 1
    machine.succeed("cp /boot/EFI/Linux/nixos-generation-1-*.efi /boot/EFI/BOOT/BOOT${efiArchUppercased}.EFI")
    machine.succeed("cp /boot/EFI/Linux/nixos-generation-1-*.efi /boot/EFI/systemd/systemd-boot${efiArch}.efi")

    # Let's reboot.
    machine.succeed("sync")
    machine.crash()
    machine.start()

    # This is the sd-boot EFI variable indicator, we should not have it at this point.
    print(machine.execute("bootctl")[1]) # Check if there's incorrect value in the output.
    machine.succeed(
        "test -e /sys/firmware/efi/efivars/LoaderEntrySelected-4a67b082-0a4c-41cf-b6c7-440b29bb8c4f && false || true"
    )

    expected_variables = ["LoaderDevicePartUUID",
      "LoaderImageIdentifier",
      "LoaderFirmwareInfo",
      "LoaderFirmwareType",
      "StubInfo",
      "StubFeatures"
    ]

    # Debug all systemd loader specification GUID EFI variables loaded by the current environment.
    print(machine.succeed(f"ls /sys/firmware/efi/efivars/*-{SD_LOADER_GUID}"))
    with subtest("Check if supported variables are exported"):
        for expected_var in expected_variables:
            machine.succeed(f"test -e /sys/firmware/efi/efivars/{expected_var}-{SD_LOADER_GUID}")

    with subtest("Is `StubInfo` correctly set"):
        assert "lanzastub" in read_string_variable("StubInfo"), "Unexpected stub information, provenance is not lanzaboote project!"

    assert_variable_string("LoaderImageIdentifier", "\\EFI\\BOOT\\BOOT${efiArchUppercased}.EFI")
    # TODO: exploit QEMU test infrastructure to pass the good value all the time.
    assert_variable_string("LoaderDevicePartUUID", "1c06f03b-704e-4657-b9cd-681a087a2fdc")
    # OVMF tests are using EDK II tree.
    assert_variable_string_contains("LoaderFirmwareInfo", "EDK II")
    assert_variable_string_contains("LoaderFirmwareType", "UEFI")

    with subtest("Is `StubFeatures` non-zero"):
        assert struct.unpack('<Q', read_raw_variable("StubFeatures")) != 0
  '';
}
