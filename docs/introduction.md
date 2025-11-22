# Introduction

This documentation helps users set up UEFI Secure Boot for their NixOS system
using a custom keychain. The audience are **experienced NixOS users**.

Secure Boot for NixOS is still in development and has some sharp
edges. There may be cases where you end up with a system that does not
boot.

**We only recommend setting up Secure Boot to NixOS users that are comfortable
using recovery tools to restore their system or have a backup ready.**

At this point we have tested a few notebooks and are confident about Lenovo
ThinkPads and Framework notebooks. However, Secure Boot support is known to be
inconsistent even on notebooks from the same product line so we cannot give
guarantees on the applicability of what we describe here.

## Prerequisites

To be able to setup Secure Boot on your device, NixOS needs to be
installed in UEFI mode and
[`systemd-boot`](https://www.freedesktop.org/wiki/Software/systemd/systemd-boot/)
must be used as a boot loader.
This means if you wish to install Lanzaboote on a new machine,
you need to follow the install instruction for systemd-boot
and then switch to Lanzaboote after the first boot.

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

## Getting Started

To setup Secure Boot on your machine, first [prepare your
system](./how-to-guides/prepare-your-system.md) and then [enable Secure
Boot](./how-to-guides/enable-secure-boot.md).
