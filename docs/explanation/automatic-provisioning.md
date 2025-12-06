# Automatic Provisioning

You can use Lanzaboote to automatically and fully provision a system.

## Process

This is an overview of the general process involved in fully provision a system
from scratch:

- Assemble a NixOS config that includes the snippet from the next section
- Install a system with unsigned artifacts (either via `nixos-install` or as an
  image)
- On the first boot, a systemd service generates the keys
- Another system service starts after the keys have been generated and prepares
  their enrollment by:
  - Generating EFI Authenticated Variables from the generated keys
  - Storing them in the EFI System Partition (ESP)
  - Re-signing all artifacts on the ESP with the new keys
  - Triggering a reboot of the system if the service succeeded
- On the next boot, `systemd-boot` enrolls the Authenticated Variables into the
  firmware before the system starts
- Now Secure Boot enforcement is enabled and only signed artifacts can be
  booted

This is essentially a trust on first use model where the system is unsigned and
untrusted on the first boot but then becomes signed and trusted on the next
boot.

## Config

To enable fully automatic provisioning, enable these options:

```nix
boot.lanzaboote = {
  autoGenerateKeys.enable = true;
  autoEnrollKeys = {
    enable = true;
    # Automatically reboot to enroll the keys in the firmware
    autoReboot = true;
  };
};
```

If you do not want to enroll Microsoft keys, read the [guide for automatically
enrolling keys](../how-to-guides/automatically-enroll-keys.md) for more info.
