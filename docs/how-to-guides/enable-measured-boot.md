# Enable Measured Boot

This guide will walk you through enabling Measured Boot on a system that
already has some form of `LUKS2` disk encryption and migrating this `LUKS2`
volume to Measured Boot.

> [!NOTE]
> We do not support filesystem level encryption via ZFS or brtfs.
>
> While you will be able to use this same basic mechanism (i.e. a managed TPM2
> policy) for unlocking filesystem level encryption, there is no integration we
> provide for it. You will have to implement this yourself.

## Enable Measured Boot in Your Config

> [!NOTE]
> If you enable Measured Boot, the maximum allowed `configurationLimit` is 8.
> This limit is enforced by `systemd-pcrlock` [which currently won't create a
> policy for more than 8
> variants](https://github.com/systemd/systemd/issues/41526).

```nix
boot.lanzaboote = {
  measuredBoot = {
    enable = true;
    pcrs = [
      0
      1
      2
      3
      4
      7
    ];
  };
};
```

## Switch to the New Generation

Switch to the new generation:

```
nixos-rebuild boot
```

> [!NOTE]
> If you're using an ephemeral root, you need to persist
> `boot.lanzaboote.measuredBoot.pcrlockPolicy` and
> `boot.lanzaboote.measuredBoot.pcrlockDirectory` across reboots.

Now reboot:

```
reboot
```

## Enroll the Policy

> [!CAUTION]
> Always enroll some form of recovery key or passphrase!
>
> `systemd-pcrlock` is still considered experimental by systemd. So to avoid
> data loss in the case of misconfiguration or other TPM issues, you should
> have some way to manually unlock your volume.

For an attended system like a workstation, you should enforce some kind of user
secret *in addition* to the TPM for unlocking your encrypted (root) volume.
Thus, use the option `--tpm2-with-pin=true` for systemd-cryptenroll.

```
systemd-cryptenroll \
  --tpm2-device=auto \
  --tpm2-with-pin=true \
  --tpm2-pcrlock=/var/lib/systemd/pcrlock.json \
  /dev/sdX
```

Congratulations, you are now a proud user of Measured Boot! You will not need
to re-enroll anything into your LUKS2 volume. Lanzaboote will automatically
take care of creating measurements and updating the TPM policy whenever you
update your system via `nixos-rebuild`.
