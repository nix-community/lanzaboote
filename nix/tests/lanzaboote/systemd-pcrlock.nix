{ pkgs, ... }:

{
  name = "lanzaboote-systemd-pcrlock";

  nodes.machine = { config, pkgs, ... }: {
    imports = [ ./common/lanzaboote.nix ];

    virtualisation.tpm.enable = true;

    boot.initrd.systemd.storePaths = [
      "${config.systemd.package}/lib/systemd/systemd-pcrextend"
      "${pkgs.tpm2-tss}/lib"
    ];

    boot.initrd.systemd.additionalUpstreamUnits = [
      "systemd-pcrphase-initrd.service"
    ];

    systemd.additionalUpstreamSystemUnits = [
      "systemd-pcrphase.service"
      "systemd-pcrphase-sysinit.service"
    ];

    environment.etc = {
      systemd-pcrlock-builtin = {
        target = "pcrlock.d";
        source = "${config.systemd.package}/lib/pcrlock.d";
      };
    };
  };

  testScript = (import ./common/efivariables-helper.nix) + ''
    import json

    machine.start()

    with subtest("Check if systemd-pcrphase measurements have been made"):
      machine.wait_for_unit("systemd-pcrphase.service")
      machine.wait_for_unit("systemd-pcrphase-sysinit.service")

    with subtest("Check if all expected IPL measurements are present"):
      (status, log_json) = machine.execute("${pkgs.systemd}/lib/systemd/systemd-pcrlock log --json=short")
      log_data = json.loads(log_json)

      ipl_entries = [entry["description"] for entry in log_data["log"] if entry["pcr"] == 11 and entry["event"] == "ipl"]

      for section in [".osrel", ".cmdline", ".initrd", ".linux"]:
          assert f"String: {section}" in ipl_entries, f"Failed to find IPL measurement for section `{section}`"
  '';
}

