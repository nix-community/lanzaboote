# Prepare Your System

This guide walks you through setting up your own Secure Boot keys and
configuring your machine to automatically sign your binaries with Lanzaboote.

## Generate Keys

To create Secure Boot keys, we use `sbctl`, a great tool that makes the
handling of Secure Boot keys easy and secure. `sbctl` is available in
[Nixpkgs](https://github.com/NixOS/nixpkgs) as `pkgs.sbctl`.

Once you have installed sbctl (or entered a Nix shell) enter:

```console
$ sudo sbctl create-keys
[sudo] password for julian:
Created Owner UUID 8ec4b2c3-dc7f-4362-b9a3-0cc17e5a34cd
Creating secure boot keys...✓
Secure boot keys created!
```

This takes a couple of seconds. When it is done, your Secure Boot keys
are located in `/var/lib/sbctl`. `sbctl` sets the permissions of the
secret key so that only root can read it.

> [!TIP]
> If you have preexisting keys in `/etc/secureboot` you can migrate these to `/var/lib/sbctl`.
>
> ```sh
> sbctl setup --migrate
> ```

## Configure NixOS (with [`lon`](https://github.com/nikstur/lon))

Add `lanzaboote` as a dependency via `lon` tracking a stable release tag (https://github.com/nix-community/lanzaboote/releases).

```console
$ lon add github nix-community/lanzaboote -r v1.0.0 --frozen
Adding lanzaboote...
Locked revision: v1.0.0
Locked hash: sha256-If6vQ+KvtKs3ARBO9G3l+4wFSCYtRBrwX1z+I+B61wQ=
```

Add this fragment to your `configuration.nix`:

```nix
# file: configuration.nix
{ pkgs, lib, ... }:
let
  sources = import ./lon.nix;
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
    pkiBundle = "/var/lib/sbctl";
  };
}
```

If you're using Lanzaboote from main, you need to call it via

```nix
lanzaboote = import sources.lanzaboote { inherit pkgs; };
```

## Configure NixOS (with Flakes)

Add this fragment to your `flake.nix`:

```nix
{
  description = "A SecureBoot-enabled NixOS configurations";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    lanzaboote = {
      url = "github:nix-community/lanzaboote/v1.0.0";

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
              pkiBundle = "/var/lib/sbctl";
            };
          })
        ];
      };
    };
  };
}
```

Now, rebuild your system with `nixos-rebuild switch`.

## Verify Your Machine is Ready

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

It is expected that the files starting with `kernel-` are _not_ signed.

Now, you need to [enable Secure Boot](./enable-secure-boot.md) so that your
firmware enforces signature verification.
