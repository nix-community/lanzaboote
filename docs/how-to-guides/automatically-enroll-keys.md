# Automatically Enroll Keys

Automatic key enrollment can be configured via:

```nix
boot.lanzaboote.autoEnrollKeys = {
  enable = true;
};
```

> [!NOTE]
> If you're using an ephemeral root, you need to persist `/var/lib/auto-cryptenroll` across reboots.

By default, when you enable automatic enrollment, Microsoft keys are enrolled
alongside your own keys. If you don't know much about Secure Boot, you should
stick with this default. Some Option ROMs are signed with Microsoft keys and
will not be able to be loaded if you don't include them.

## Without Microsoft Keys (Potentially Dangerous)

If you don't have Option ROMs you're worried about and don't want to enroll
Microsoft keys, you can. However, this has the potential to soft brick your
system and you should only enable this option if you know what you're doing and
have the means to recover from a potential soft brick. Set these options to not
enroll Microsoft keys:

```nix
boot.lanzaboote.autoEnrollKeys = {
  includeMicrosoftKeys = false;
  allowBrickingMyMachine = true;
};
```

### Enroll Checksums from TPM Eventlog (Experimental)

You can also use the experimental and potentially dangerous option of `sbctl`
to read the checksums of your Option ROMs from the TPM eventlog and enroll
them in your firmware instead of enrolling Microsoft keys. This also has the
potential to soft brick your system after firmware updates that will not be
picked up by this mechanism. Only do this if you know what you're doing:

```nix
boot.lanzaboote.autoEnrollKeys = {
  includeMicrosoftKeys = false;
  allowBrickingMyMachine = true;

  includeChecksumsFromTPM = true;
};
```

### Include Firmware Built-In Keys

On some machines (such as Framework laptops), the firmware requires OEM certificates (pre-provisioned keys) to be retained so that features like vendor-provided firmware updates continue to function. To preserve these default certificates during enrollment, enable `includeFirmwareBuiltinKeys`:

```nix
boot.lanzaboote.autoEnrollKeys = {
  includeFirmwareBuiltinKeys = true;
};
```

## Rebooting

The final step of automatic enrollment is rebooting because it relies on
`systemd-boot` setting up the keys in your firmware. You can either do this
manually after the first boot of the system that has been configured for
automatic enrollment or by enabling automatic reboot:

```nix
boot.lanzaboote.autoEnrollKeys = {
  autoReboot = true;
};
```

This restarts your system automatically right after the preparations for
automatic enrollment have been completed by a systemd service so that
`systemd-boot` can set up the keys as part of your next boot.
