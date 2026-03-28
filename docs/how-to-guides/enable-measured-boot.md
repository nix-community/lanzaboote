# Enable Measured Boot

This guide will walk you through enabling measured boot on a system that
already has some form of `LUKS2` disk encryption and migrating this `LUKS2`
volume to measured boot.

## Enable Measured Boot in Your Config

> [!NOTE]
> If you enable Measured Boot, the maximum allowed `configurationLimit` is 8.
> This limit is enforced by `systemd-pcrlock` which won't create a policy for
> more than 8 variants.

```nix
boot.lanzaboote = {
  measuredBoot = {
    enable = true;
    pcrs = [ 0 4 7 ];
  };
};
```

## Switch to the New Generation

Switch to the new generation and reboot:

```
nixos-rebuild boot
reboot
```

## Enroll the Generated `systemd-pcrlock` Policy into Your `LUKS2` Volume

> [!CAUTION]
> Since `systemd-pcrlock` is still experimental, we strongly suggest to enroll
> some form of recovery key or passphrase to avoid data loss in the case of
> misconfiguration or other TPM issues.

For an attended system like a workstation,you want to enforce some kind of user
secret *in addition* to the TPM for unlocking your encrypted (root) volume.
Thus, we suggest the option `--tpm2-with-pin=true`.

```
systemd-cryptenroll \
  --tpm2-device=auto \
  --tpm2-with-pin=true \
  --tpm2-pcrlock=/var/lib/systemd/pcrlock.json \
  /dev/sdX
```
