{ pkgs, ... }:

{
  name = "lanzaboote-systemd-measured-uki";

  nodes.machine = { config, lib, pkgs, ... }: {
    imports = [ ./common/lanzaboote.nix ];

    virtualisation.tpm.enable = true;
  };

  testScript = ''
    machine.start()

    with subtest("Check if systemd considers measured-uki condition met"):
      machine.succeed("systemd-analyze condition ConditionSecurity=measured-uki")

    with subtest("Check if systemd-measure finds the correct PCR index"):
      (status, measure_out) = machine.execute("${pkgs.systemd}/lib/systemd/systemd-measure 2>&1")
      assert "Failed to parse EFI variable" not in measure_out, "systemd-measure failed to parse EFI variable - encoding issue?"
  '';
}

