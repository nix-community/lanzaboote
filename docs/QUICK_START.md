# Quick Start: NixOS Secure Boot

This document attempts to guide users into setting up UEFI Secure Boot
for their NixOS system using a custom key chain. The audience are
experienced NixOS users.

This guide has been tested on a Lenovo Thinkpad and is expected to
work on other Thinkpads without change. On other systems, certain
steps may be different.

## ⚠ Disclaimers ⚠

Secure Boot for NixOS is still in development and has some sharp
edges. There may be cases where you end up with a system that does not
boot.

**We only recommend this to NixOS users that are comfortable using
recovery tools to restore their system or have a backup ready.**

## Functional Requirements

To be able to setup Secure Boot on your device, NixOS needs to be
installed in UEFI mode and
[`systemd-boot`](https://www.freedesktop.org/wiki/Software/systemd/systemd-boot/)
must be used as a boot loader.

These prerequisites can be checked via `bootctl status`:

```console
$ bootctl status
System:
     Firmware: UEFI 2.70 (Lenovo 0.4720)
  Secure Boot: disabled (disabled)
 TPM2 Support: yes
 Boot into FW: supported

Current Boot Loader:
      Product: systemd-boot 251.7
...
```

In the `bootctl` output, the firmware needs to be `UEFI` and the
current boot loader needs to be `systemd-boot`. If this is the case,
you are all set to continue.

## Security Requirements

These requirements are _optional_ for a development system. Feel free
to skip them, if you just want to hack on Secure Boot support.

To provide any security your system needs to defend against an
attacker turning UEFI Secure Boot off or being able to sign binaries
with the keys we are going to generate.

The easiest way to achieve this is to:

1. Enable a BIOS password in your system.
2. Use full disk encryption.

**The topic of security around Secure Boot is complex. We are only
scratching the surface here and a comprehensive guide is out of
scope.**

## Part 1: Preparing Your System

In the first part, we will prepare everything on the software side of
things. At the end of this part, you will have your own Secure Boot
keys and a NixOS that has signed boot binaries.

### Finding the UEFI System Partition (ESP)

The UEFI boot process revolves around a special partition on the
disk. This partition is called _ESP_, the (U)EFI System
Partition. This partition is by convention mounted at `/boot` on NixOS
and the rest of this document assumes this.

You can verify that `/boot` is the ESP by looking for `ESP:` in
`bootctl status` output.

### Creating Your Keys

To create Secure Boot keys, we use `sbctl`, a popular Secure Boot Key
Manager. `sbctl` is available in
[Nixpkgs](https://github.com/NixOS/nixpkgs) as `pkgs.sbctl`.

Once you have installed sbctl (or entered a Nix shell), creating your
Secure Boot keys requires this command:

```console
$ sudo sbctl create-keys
[sudo] password for julian:
Created Owner UUID 8ec4b2c3-dc7f-4362-b9a3-0cc17e5a34cd
Creating secure boot keys...✓
Secure boot keys created!
```

This takes a couple of seconds. When it is done, your Secure Boot keys
are located in `/etc/secureboot`. `sbctl` sets the permissions of the
secret key so that only root can read it.

### Configuring NixOS (with [`niv`](https://github.com/nmattia/niv))

Add `lanzaboote` as a dependency of your niv project and track a stable release tag (https://github.com/nix-community/lanzaboote/releases).

```console
$ niv add nix-community/lanzaboote -r v0.3.0 -v 0.3.0
Adding package lanzaboote
  Writing new sources file
Done: Adding package lanzaboote
```

Below is a fragment of a NixOS configuration that enables the SecureBoot stack.

```nix
# file: configuration.nix
{ pkgs, lib, ... }:
let
    sources = import ./nix/sources.nix;
    lanzaboote = import sources.lanzaboote;
in
{
  imports = [ lanzaboote.nixosModules.lanzaboote ];

  environment.systemPackages = [
    # For debugging and troubleshooting Secure Boot.
    pkgs.sbctl
  ];

  # Lanzaboote currently replaces the systemd-boot module.
  # This setting is usually set to true in configuration.nix
  # generated at installation time. So we force it to false
  # for now.
  boot.loader.systemd-boot.enable = lib.mkForce false;

  boot.lanzaboote = {
    enable = true;
    pkiBundle = "/etc/secureboot";
  };
}
```

### Configuring NixOS (with Flakes)

Below is a fragment of a NixOS configuration that enables the Secure
Boot stack.

```nix
{
  description = "A SecureBoot-enabled NixOS configurations";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    lanzaboote = {
      url = "github:nix-community/lanzaboote/v0.3.0";

      # Optional but recommended to limit the size of your system closure.
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, lanzaboote, ...}: {
    nixosConfigurations = {
      yourHost = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";

        modules = [
          # This is not a complete NixOS configuration and you need to reference
          # your normal configuration here.

          lanzaboote.nixosModules.lanzaboote

          ({ pkgs, lib, ... }: {

            environment.systemPackages = [
              # For debugging and troubleshooting Secure Boot.
              pkgs.sbctl
            ];

            # Lanzaboote currently replaces the systemd-boot module.
            # This setting is usually set to true in configuration.nix
            # generated at installation time. So we force it to false
            # for now.
            boot.loader.systemd-boot.enable = lib.mkForce false;

            boot.lanzaboote = {
              enable = true;
              pkiBundle = "/etc/secureboot";
            };
          })
        ];
      };
    };
  };
}
```

### Checking that your machine is ready for Secure Boot enforcement

After you rebuild your system, check `sbctl verify` output:

```console
$ sudo sbctl verify
Verifying file database and EFI images in /boot...
✓ /boot/EFI/BOOT/BOOTX64.EFI is signed
✓ /boot/EFI/Linux/nixos-generation-355.efi is signed
✓ /boot/EFI/Linux/nixos-generation-356.efi is signed
✗ /boot/EFI/nixos/0n01vj3mq06pc31i2yhxndvhv4kwl2vp-linux-6.1.3-bzImage.efi is not signed
✓ /boot/EFI/systemd/systemd-bootx64.efi is signed
```

It is expected that the files ending with `bzImage.efi` are _not_
signed.

## Part 2: Enabling Secure Boot

Now that NixOS is ready for Secure Boot, we will setup the
firmware. At the end of this section, Secure Boot will be enabled on
your system and your firmware will only boot binaries that are signed
with your keys.

These instructions are specific to Thinkpads and may need to be
adapted on other systems.

### Entering Secure Boot Setup Mode

The UEFI firmware allows enrolling Secure Boot keys when it is in
_Setup Mode_.

On a Thinkpad enter the BIOS menu using the "Reboot into Firmware"
entry in the systemd-boot boot menu. Once you are in the BIOS menu:

1. Select the "Security" tab.
2. Select the "Secure Boot" entry.
3. Set "Secure Boot" to enabled.
4. Select "Reset to Setup Mode".

When you are done, press F10 to save and exit.

You can see these steps as a video [here](https://www.youtube.com/watch?v=aLuCAh7UzzQ).

> ⚠️ Do not select "Clear All Secure Boot Keys" as it will drop the Forbidden
> Signature Database (dbx).

### Enrolling Keys

Once you've booted your system into NixOS again, you have to enroll
your keys to activate Secure Boot. We include Microsoft keys here to
avoid boot issues.

```console
$ sudo sbctl enroll-keys --microsoft
Enrolling keys to EFI variables...
With vendor keys from microsoft...✓
Enrolled keys to the EFI variables!
```

> ⚠️ During boot, some hardware might include OptionROMs signed with
> Microsoft keys.
> By using the `--microsoft`, we enroll the Microsoft OEM certificates.
> Another more experimental option would be to enroll OptionROMs checksum seen
> at last boot using `--tpm-eventlog`, but these checksums might change later.

You can now reboot your system. After you've booted, Secure Boot is
activated and in user mode:

```console
$ bootctl status
System:
      Firmware: UEFI 2.70 (Lenovo 0.4720)
 Firmware Arch: x64
   Secure Boot: enabled (user)
  TPM2 Support: yes
  Boot into FW: supported
```

> ⚠️  If you used `--microsoft` while enrolling the keys, you might want
> to check that the Secure Boot Forbidden Signature Database (dbx) is not
> empty.
> A quick and dirty way is by checking the file size of
> `/sys/firmware/efi/efivars/dbx-*`.
> Keeping an up to date dbx reduces Secure Boot bypasses, see for example:
> <https://uefi.org/sites/default/files/resources/dbx_release_info.pdf>.

That's all! 🥳

## Troubleshooting

If your system doesn't boot with Secure Boot enabled, the most likely
issue is that Lanzaboote could not verify a cryptographic hash. To
recover from this, disable Secure Boot in your firmware
settings. Please file a bug, if you hit this issue.

## Disabling Secure Boot and Lanzaboote

When you want to permanently get back to a system without the Secure
Boot stack, **first** disable Secure Boot in your firmware
settings. Then you can disable the Lanzaboote related settings in the
NixOS configuration and rebuild.

You may need to clean up the `EFI/Linux` directory in the ESP manually
to get rid of stale boot entries. **Please backup your ESP, before you
delete any files** in case something goes wrong.

## Alternatives

The [ArchLinux wiki](https://wiki.archlinux.org/title/Unified_Extensible_Firmware_Interface/Secure_Boot)
contains alternatives to handling your keys, in case `sbctl` is not
flexible enough.
