# Changelog

## 0.5.0 (unreleased)

## Changed

- Exchanged rust-overlay for fenix. When you override your inputs, you now have
  to override fenix instead of rust-overlay.

### Removed

- Removed the internal option `boot.lanzaboote.enrollKeys` that was only
  intended for testing without replacement.

## 0.4.3

### Added

- Added `boot.lanzaboote.sortKey` option. This can be used to add a custom
  `sort-key` to your boot entries.
