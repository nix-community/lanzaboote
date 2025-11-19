# Contributing

Please check the [issues](https://github.com/nix-community/lanzaboote/issues),
if you want to take something up. If you plan to implement a bigger feature,
it is a good idea to open an issue beforehand and discuss it with the
maintainers.

You can use the [Matrix room](https://matrix.to/#/#secure-boot:nixos.org) to
coordinate with the maintainers and other contributors.

## Code Organization

### `lzbt-*`, the Lanzaboote tool
We plan to have multiple backends for `lzbt`:

- `lzbt-systemd` lives in [`rust/tool/systemd`](rust/tool/systemd)

In the future, `lzbt` may support more backends.

Shared code lives in [`rust/tool/shared`](rust/tool/shared).

### Stub

The stub lives in [`rust/uefi/stub`](rust/uefi/stub).

