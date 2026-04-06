# Measured Boot

Lanzaboote supports Measured Boot by creating a TPM2 policy via
`systemd-pcrlock` that can be used to seal a secret (e.g. your LUKS2 key)
inside a TPM to an expected state of your system.

Most importantly, you can use this to improve the security of your LUKS2 volume
encryption. It will only allow the unlocking of the volume if your system is in
an expected state. You can use this fully unattended by only relying on the
policy or in combination of a user provided pin.

## Supported PCRs

We believe that these are the most important and usable PCRs to achieve a
secure system.

- 0
- 1
- 2
- 3
- 4
- 7

See the
[UAPI.7](https://uapi-group.org/specifications/specs/linux_tpm_pcr_registry/)
spec for a full explanation of what is measured into each PCR.

## Boot Loader and Lanzaboote Measurements

The most important measurement for Lanzaboote is PCR 4. It covers the boot
loader and the Lanzaboote stub. We leverage the same idea that we use for
Secure Boot: only the stub needs to be covered because we check the hash of all
other included components from the stub. We therefore cover the initrd, the
kernel, and the cmdline which is included in the stub itself. Thus, by
including PCR 4 in our policy we cover the entire boot chain after the firmware
has booted.


