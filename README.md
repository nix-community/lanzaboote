# Lanzaboote

![GitHub branch checks state](https://img.shields.io/github/checks-status/blitz/lanzaboote/master)
[![made-with-rust](https://img.shields.io/badge/Made%20with-Rust-1f425f.svg)](https://www.rust-lang.org/)
[![GitHub license](https://img.shields.io/github/license/Naereen/StrapDown.js.svg)](https://github.com/Naereen/StrapDown.js/blob/master/LICENSE)

🚧🚧🚧 **This is not working yet. Come back later.*** 🚧🚧🚧

This repository contains experimental tooling for Secure Boot on
[NixOS](https://nixos.org/).

## lanzatool

`lanzatool` is a Linux command line application that takes a
[bootspec](https://github.com/NixOS/rfcs/pull/125) document and
installs the boot files into the UEFI
[ESP](https://en.wikipedia.org/wiki/EFI_system_partition).

## lanzaboote

`lanzaboote` is a UEFI application that is started by systemd-boot (or
any other EFI boot loader) and loads a Linux kernel and initrd without
breaking the Secure Boot chain of trust.

The information what kernel with what command line and initrd to boot
is embedded into the `lanzaboote` by `lanzatool`.
