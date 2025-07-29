# Overriding Nixpkgs (or other Inputs) for Non-Flakes

Many users try to minimize the number of Nixpkgs versions pulled into
their NixOS system configuration. While this results in a Lanzaboote
version that has not been tested by CI and has to be built locally, it
does result in faster evaluation times and a smaller system
closure. Nix Flakes have built-in functionality to do this with
`inputs.nixpkgs.follows`, so here we focus on the non-Flakes case.

Our goal here is to extend the [quick-start
example](./QUICK_START.md#non-flakes-nix-conf) to use the same Nixpkgs
for Lanzaboote as for the rest of the system. We are going to use a
custom [`flake-compat`](https://github.com/nilla-nix/flake-compat) to
allow overriding the Flake's inputs. For this we need to add it to the
project first:

```console
$ npins add github nilla-nix flake-compat
[INFO ] Adding 'flake-compat' …
    repository: https://github.com/nilla-nix/flake-compat.git
    pre_releases: false
    submodules: false
    version: v0.0.2
    revision: 2653659fb5d86f2853caae1b6475e63e8c23439c
    hash: 165xl96d5f2xaawyiaj3cl8ccisrjvx5b1db8nfh18swaxyzdckn
    frozen: false
```

Then we can use it in the NixOS configuration to override inputs in
the Lanzaboote Flake:

```nix
# file: configuration.nix
{ pkgs, lib, ... }:
let
    pins = import ./npins;

    # Load the flake compatibility code.
    compat = import pins.flake-compat;

    lanzaboote = (compat.load {
        src = pins.lanzaboote;

        replacements = {
          # Pass your nixpkgs to lanzaboote as an input. We have
          # to pass it as a flake as well.
          nixpkgs = compat.load { src = pins.nixpkgs; };
        };
    }).outputs;
in
{
  imports = [ lanzaboote.nixosModules.lanzaboote ];

  # The rest is identical to the previous example.

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
