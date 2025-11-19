# Changelog

## 0.5.0 (unreleased)

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

### Changed

- Changed the non-flakes Nix interface of Lanzaboote. Now needs to be called
  with an argument: `lanzaboote = import sources.lanzaboote { };`.

### Removed

- Removed the internal option `boot.lanzaboote.enrollKeys` that was only
  intended for testing without replacement.

## 0.4.3

### Added

- Added `boot.lanzaboote.sortKey` option. This can be used to add a custom
  `sort-key` to your boot entries.
