# Changelog

## Unreleased

### Added

- Added support for Measured Boot via `systemd-pcrlock`. Refer to the [Measured
  Boot
  guide](https://nix-community.github.io/lanzaboote/how-to-guides/enable-measured-boot.html)
  to get started using this.

### Removed

- Removed support for features that depended on whether Secure Boot was enabled
  in the Lanzaboote stub. This was done so that users can leverage Measured
  Boot without requiring Secure Boot.
    - Removed support for loading kernels or initrds whose hashes do not match
      those embedded into the Lanzaboote image.

## 1.0.0

### Added

- You can now use Lanzaboote completely without flakes or flake-compat,
  explicitly controlling all dependencies:

  ```nix
  system = builtins.currentSystem;
  pkgs = import sources.nixpkgs { inherit system; };

  lanzaboote = import sources.lanzaboote {
    inherit system pkgs;
    rust-overlay = sources.rust-overlay;
    crane = import sources.crane { inherit pkgs; };
  };
  ```

  However, this is optional. You can also just provide an empty attrset `{ }`
  and rely on the versions of the dependencies we have pinned.
- Added the option `boot.lanzaboote.autoGenerateKeys.enable` which allows you
  to automatically generate Secure Boot keys in a systemd service if they do
  not exist yet. Please read the
  [docs](https://nix-community.github.io/lanzaboote/) for more info.
- Added the options `boot.lanzaboote.autoEnrollKeys.*` which allow you to
  automatically enroll your Secure Boot keys into the firmware. A systemd
  service prepares everything and `systemd-boot` finally enrolls the keys on
  the next boot. Please read the
  [docs](https://nix-community.github.io/lanzaboote/) for more info.
- Added the option `boot.lanzaboote.allowUnsigned` which enables installing
  unsigned artifacts to the ESP. This is useful for automatic provisioning of
  systems with Secure Boot.
- Added support for multiple ESPs. You can configure additional ESPs that you
  want Lanzaboote to install boot artifacts to via
  `boot.lanzaboote.extraEfiSysMountPoints = [ "/boot2" ];`:

### Changed

- Changed the non-flakes Nix interface of Lanzaboote. Now needs to be called
  with an argument: `lanzaboote = import sources.lanzaboote { };`.
- `boot.lanzaboote.pkiBundle` now uses the type `externalPath` and thus cannot
  point to Nix Store paths anymore.

### Removed

- Removed the internal option `boot.lanzaboote.enrollKeys` that was only
  intended for testing without replacement.

## 0.4.3

### Added

- Added `boot.lanzaboote.sortKey` option. This can be used to add a custom
  `sort-key` to your boot entries.
