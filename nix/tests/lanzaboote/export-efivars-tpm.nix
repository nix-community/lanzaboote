{
  name = "lanzaboote-export-efivars-tpm";

  nodes.machine = {
    imports = [ ./common/lanzaboote.nix ];

    virtualisation.tpm.enable = true;
  };

  testScript = (import ./common/efivariables-helper.nix) + ''
    machine.start()

    # TODO: the other variables are not yet supported.
    expected_variables = [
      "StubPcrKernelImage"
    ]

    # Debug all systemd loader specification GUID EFI variables loaded by the current environment.
    print(machine.succeed(f"ls /sys/firmware/efi/efivars/*-{SD_LOADER_GUID}"))
    with subtest("Check if supported variables are exported"):
        for expected_var in expected_variables:
          machine.succeed(f"test -e /sys/firmware/efi/efivars/{expected_var}-{SD_LOADER_GUID}")

    # "Static" parts of the UKI is measured in PCR11
    assert_variable_string("StubPcrKernelImage", "11")
  '';
}
