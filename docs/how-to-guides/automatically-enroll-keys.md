# Automatically Enroll Keys

Automatic key enrollment can be configured via:

```nix
boot.lanzaboote.autoEnrollKeys = {
  enable = true;
};
```

By default, when you enable auto enrollment, you will enroll the Microsoft keys
alongside your own keys. If you don't know much about Secure Boot, you should
stick with this default. Some Option ROMs are signed with the Microsoft keys
and will not be able to be loaded if you don't include the Microsoft keys.

## Without Microsoft Keys (Potentially Dangerous)

If you don't have Option ROMs you're worried about and don't want to enroll the
Microsoft keys, you can. However, this has the potential to soft brick your
system and you should only enable this option if you know what you're doing and
have the means to recover from a potential soft brick. Set these option to not
enroll the Microsoft keys:

```nix
boot.lanzaboote.autoEnrollKeys = {
  includeMicrosoftKeys = false;
  allowBrickingMyMachine = true;
};
```

### Enroll Checksums from TPM Eventlog (Experimental)

You can also use the experimental and potentially dangerous option of `sbctl`
to read the checksums of your Option ROMs from the TPM eventlog and enroll
these in your firmware instead of enrolling the Microsoft keys. This also has
the potential to soft brick your system after firmware updates that will not be
picked up by this mechanism. Only do this when you know what you're doing:

```nix
boot.lanzaboote.autoEnrollKeys = {
  includeMicrosoftKeys = false;
  allowBrickingMyMachine = true;

  includeChecksumsFromTPM = true;
};
```

## Rebooting

The final step of automatic enrollment is rebooting because it relies on
`systemd-boot` setting the keys up in your firmware. You can either do this
manually after the first boot of the system that has been configured for auto
enrollment or by enabling automatic reboot:


```nix
boot.lanzaboote.autoEnrollKeys = {
  autoReboot = true;
};
```

This restarts your system automatically right after the preparations for
automatic enrollment have been done by a systemd service so that `systemd-boot`
can set up the keys as part of your next boot.
