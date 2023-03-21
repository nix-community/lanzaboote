# Lanzaboote: Secure Boot for NixOS

[![Chat on Matrix](https://matrix.to/img/matrix-badge.svg)](https://matrix.to/#/#nixos-secure-boot:ukvly.org)
![GitHub branch checks state](https://img.shields.io/github/checks-status/blitz/lanzaboote/master)
[![made-with-rust](https://img.shields.io/badge/Made%20with-Rust-1f425f.svg)](https://www.rust-lang.org/)
![GitHub](https://img.shields.io/github/license/blitz/lanzaboote)

This repository contains tooling for [UEFI Secure
Boot](https://en.wikipedia.org/wiki/UEFI#Secure_Boot) on
[NixOS](https://nixos.org/). The goal is to make Secure Boot available
from [nixpkgs](https://github.com/NixOS/nixpkgs) for any platform that
supports UEFI.

## ⚡ Quickstart ⚡

If you want to try this out, head over [here](./docs/QUICK_START.md) for
instructions.

## 🪛 Get Involved 🪛

There is still a bunch of work to do before this work can be
upstreamed into [nixpkgs](https://github.com/NixOS/nixpkgs). Please
coordinate in the [Matrix
room](https://matrix.to/#/#nixos-secure-boot:ukvly.org) or check the
[issues](https://github.com/nix-community/lanzaboote/issues), if you
want to take something up.

## Overview

### Secure Boot

The goal of UEFI Secure Boot is to allow only trusted operating
systems to boot on a system. This can be used to defend against
certain classes of attacks that compromise the boot flow of a
system. For example, an attacker will have difficulty replacing the
Linux kernel that boots a system when Secure Boot is active.

UEFI Secure Boot works by digitally signing all drivers, bootloaders,
the Linux kernel and its initrd. This establishes a chain of trust
where one trusted component only hands off control to the next part of
the boot flow when the integrity of the chain is cryptographically
validated.

### Caveats

There are some additional steps that are required to make UEFI Secure
Boot effective:

- There must be a BIOS password or a similar restriction that prevents
  unauthorized changes to the Secure Boot policy.
- The booted system must have some form of integrity protection.
- The firmware must be kept up-to-date.

These steps will not be covered here.

### `lzbt`, the Lanzaboote tool

At the moment, boot loaders, kernels and initrds on NixOS are signed
on the current system. These then need to be prepared as [Unified
Kernel Images
(UKI)](https://uapi-group.org/specifications/specs/boot_loader_specification/#type-2-efi-unified-kernel-images) and placed on the [EFI System Partition (ESP)](https://en.wikipedia.org/wiki/EFI_system_partition).

`lzbt` is a Linux command line application that takes care of
this flow. It takes a [NixOS
bootspec](https://github.com/NixOS/rfcs/pull/125) document, signs the
relevant files, creates a UKI using the stub (see below) and
installs the UKI along with other required files to the
ESP. `lzbt` is also aware of multiple NixOS generations and will
sign all configurations that should be bootable.

`lzbt` lives in `rust/tool`.

### Stub

When the Linux kernel and initrd are packed into a UKI, they need an
UEFI application stub. This role is typically filled by
[`systemd-stub`](https://www.freedesktop.org/software/systemd/man/systemd-stub.html).

The downside of `systemd-stub` is that it requires the kernel and
initrd to be packed into the UKI, which makes it pretty large. As we
need one UKI per NixOS configuration, systems with many configurations
quickly run out of the limited disk space in the ESP.

The Lanzaboote stub is a UEFI stub that solves the same problem as
`systemd-stub`, but allows kernel and initrd to be stored separately
on the ESP. The chain of trust is maintained by validating the
signature on the Linux kernel and embedding a cryptographic hash of
the initrd into the signed UKI.

The stub lives in `rust/stub`.

### Fwupd

When both Lanzaboote and `services.fwupd` are enabled, for
`fwupd.service` a `preStart` will be added that ensures a signed fwupd
binary is placed in `/run` that fwupd will use.

## State of Upstreaming to Nixpkgs

Secure Boot is available as an Nixpkgs out-of-tree feature using the
[bootspec feature preview](https://github.com/NixOS/rfcs/pull/125). It
works with current nixpkgs-unstable.

## Funding

<pre><img alt="Logo of NLnet Foundation" src="https://nlnet.nl/logo/banner-bw.svg" width="320px" height="120px" />     <img alt="Logo of NGI Assure" src="https://nlnet.nl/image/logos/NGIAssure_tag_black_mono.svg" width="320px" height="120px" /></pre>

[This project](https://nlnet.nl/project/NixOS-UEFI/) was funded through the [NGI Assure](https://nlnet.nl/assure) Fund, a fund established by [NLnet](https://nlnet.nl/) with financial support from the European Commission's [Next Generation Internet](https://ngi.eu/) programme, under the aegis of DG Communications Networks, Content and Technology under grant agreement No 957073. **Applications are still open, you can [apply today](https://nlnet.nl/propose)**.

If your organization wants to support the project with extra funding in order to add support for more architectures, PKCS#11 workflows or integration, please contact one of the maintainers.
