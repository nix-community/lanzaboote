# Disable Measured Boot

#### Remove Policy from LUKS2 Volume

> [!CAUTION]
> You can end up being locked out of your system if you don't perform this step
> as the policy will become outdated on future updates.

To remove all TPM2 slots from your LUKS2 volume:

```sh
systemd-cryptenroll \
  --tpm2-device=auto \
  --wipe-slot=tpm2 \
  /dev/sdX
```

If you didn't set some kind of (recovery) key or password, you will have to do
this now.

#### Disable Measured Boot in Your Config

```nix
boot.lanzaboote.measuredBoot = {
  enable = false;
};
```

#### Remove Policy from Disk and Deallocate NV Index

```console
$ /run/current-system/systemd/lib/systemd/systemd-pcrlock remove-policy
```
