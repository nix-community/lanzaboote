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

## Requirements

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

## Part 1: Preparing Your System

In the first part, we will prepare everything on the software side of
things. At the end of this part, you will have your own Secure Boot
keys and a NixOS that has signed boot binaries.

### Creating Your Keys

To create Secure Boot keys, we will you `sbctl`, the Secure Boot Key
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

### Switching to bootspec

- `boot.bootspec.enable = true;`
- rebuild / reboot
- GC


### Configuring NixOS (without Flakes)

If you are using Flakes, skip to the next section.

... write me ...

### Configuring NixOS (with Flakes)

```nix
nixosConfigurations = {
  yourHost = nixpkgs.lib.nixosSystem {
	system = "x86_64-linux";

	modules = [
	  # ... other modules ...

	  lanzaboote.nixosModules.lanzaboote

	  ({ config, ... }: {
		# Lanzaboote currently replaces the sytemd-boot module.
		boot.loader.systemd-boot.enable = false;

		boot.lanzaboote = {
		  enable = true;
		  pkiBundle = "/etc/secureboot";

		  # Start conservatively, because each generation consumes
		  # space in the ESP. Running out of space in the ESP is currently
		  # not handled well.
		  configurationLimit = 20;
		};
	  })
	];
  };
```

## Part 2: Enabling Secure Boot

Now that NixOS is ready for Secure Boot, we will setup the
firmware. At the end of this section, Secure Boot will be enabled on
your system and your firmware will only boot binaries that are signed
with your keys.

### Entering Setup Mode

### Enrolling Keys

### Reboot and Profit

# old docs

How to get started with Lanzaboote-based SecureBoot?

**Requirements** :

- Using `systemd-boot`
- Either the private certificate to sign files or control over your SecureBoot keys to insert your own, read the section on how to enroll your keys

**Disclaimer** : This project bring its own UEFI stub to circumvent the difficulty introduced by UKI model where everything has to baked in.
The authors are aware of the concept of "extending" initrds through <https://github.com/systemd/systemd/blob/main/src/boot/efi/stub.c#L23>.
We are exploring this avenue to minimize our dependency on this custom stub, but, we want to explore the full needs of the NixOS usecases first.

## Enrolling your own keys

**Disclaimer** : Some device firmware, e.g. GPU (so-called [OpROMs](https://en.wikipedia.org/wiki/Option_ROM)) get executed at boot, replacing your platform keys
with your own, can cause them to not run anymore under SecureBoot, as they are signed using [Microsoft Corporation UEFI CA 2011 certificate], aka the Microsoft 3rd Party UEFI CA certificate.
To know about *limited* mitigations regarding this, **read** the "keeping Microsoft keys" subsection.

**Disclaimer** : TPMs interaction are out of scope for this project, please read more about TPM eventlog, but note that Lanzastub do not implement any TPM event logging for now.

### Entering into Setup Mode

Depending on your machine, you need:

- either, to reset your keys to enter into Setup Mode (removing your Microsoft keys!)
- either, to allow changes to your keys from your running system (**possibly**, keeping your Microsoft keys)

These instructions varies on your BIOS, see <https://www.rodsbooks.com/efi-bootloaders/controlling-sb.html#setuputil> for an example.

You can confirm to be in Setup Mode using `bootctl status`, reading `Secure Boot` line, which should show a line similar to: `Secure Boot: no (setup mode)`.

In this context, **note that** `/sys/firmware/efi/efivars/` becomes writeable from your Linux system.

**Bricking your machine in a bad way can happen if this location gets overwritten by a strange command / program.** ^[I already `rm'd -rf` this location once, it was very not funny.]

### Installing your keys

#### Easy way: `sbctl`

`sbctl` has static locations for keys, in nixpkgs, `sbctl` writes to `/etc/secureboot`, so prepare a directory for this location with the **right permissions**.

- Creating keys is: `sbctl create-keys`
- Rotating keys is: `sbctl rotate-keys` (with `sbctl` ≥ 0.10)

For enrollment, first, read the next subsection:

##### Keeping Microsoft keys

If you want to keep Microsoft keys, run:

```console
$ sbctl enroll-keys --microsoft
```

It will include Microsoft Corporation UEFI CA 2011 certificate.

It does not guarantee that everything will work out of the box, it may be the case you have other certificates that should be included from your OEM which are not distributed in `sbctl`.

Therefore, please double check everything until this project can provide better guarantees.

##### Ignoring Microsoft keys

If you want to **remove** Microsoft keys, you have very good reasons to do so (i.e. not trusting Microsoft to not give a valid certificate to OEM which could pose risk to your threat model), run:

```
$ sbctl enroll-keys --yes-this-might-brick-my-machine
```

The authors of this project declines all responsibility for running this command and bricking your machine, this is **experimental** SecureBoot.

#### Harder ways: follow the Archlinux Wiki

Read about this on <https://wiki.archlinux.org/title/Unified_Extensible_Firmware_Interface/Secure_Boot#Enrolling_keys_in_firmware>.

## Using the `Lanzaboote` project in your NixOS configuration

**Disclaimer** : `Lanzaboote` requires [RFC-0125 aka bootspec](https://github.com/NixOS/rfcs/pull/125) on your system, this is automatically enabled on your behalf.

**Warning** : `Lanzaboote` is compatible only with `systemd-boot` for now, also, it replaces `boot.loader.systemd-boot` and has no feature parity with this NixOS module.

Use your favorite way to import this project in your configuration, i.e. Flakes or [niv](https://github.com/nmattia/niv) or `fetchFromGitHub`:

### `fetchFromGitHub`

```
{ pkgs, config, lib, ... }:
{
  imports = [
    (pkgs.fetchFromGitHub {
      owner = "nix-community";
      repo = "lanzaboote";
      rev = "relevant-revision";
      sha256 = lib.fakeHash;
    })
  ];
}
```

---

Now, assuming the NixOS module is imported, you only need to have:

```
{ pkgs, config, lib, ... }: {
  boot.lanzaboote = {
    enable = true;
    publicKeyFile = "/etc/secureboot/keys/db/db.pem"; # DB public key
    privateKeyFile = "/etc/secureboot/keys/db/db.key"; # DB private key
  };
}
```

Different options can be added:

- `boot.lanzaboote.pkiBundle`: provide the whole PKI bundle generated by `sbctl create-keys`, useful when you want to automate key enrollment, **security warning**: those will be copied at system activation in `/tmp/pki` to perform the enrollment because `sbctl` do not support dynamic arbitrary paths for now.
- `boot.lanzaboote.enrollKeys`: perform automatic enrollment key, if needed, at each rebuild

## Known limitations

- Generations are shown improperly because `/etc/os-release` induces the wrong name in the `systemd-boot` menu according to this algorithm: <>
- `$esp/nixos` is a hardcoded path to store initrds & kernels
- The lifecycle between disabling Lanzaboote and getting back into Lanzaboote is not clear
- No TPM event logging & measurements is taking place
- Alternative Nix store locations are not supported
- PKCS#11 engine support is not available
