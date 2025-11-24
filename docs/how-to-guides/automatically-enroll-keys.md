# Automatically Enroll Keys

Automatic key enrollment can be configured via a Lanzaboote option:

```nix
boot.lanzaboote.autoEnrollKeys = {
  enable = true;
};
```

Note that just enabling auto enrollment will usually not be enough to
successfully enroll the keys. You need to decide whether to also enroll
Microsoft keys or not.

## With Microsoft Keys

If you don't know much about Secure Boot, you should enroll the Microsoft keys
in your Secure Boot keyring. Some OptionRoms are signed with the Microsoft keys
and will not be able to be loaded if you don't include the Microsoft keys. Set
this option to enroll the Microsoft keys:

```nix
boot.lanzaboote.autoEnrollKeys = {
  includeMicrosoftKeys = true;
};
```

## Without Microsoft Keys

If you don't want to include the Microsoft keys in your Secure Boot keyring,
you can. However, this has the potential to soft brick your system and you
should only enable this option if you know what you're doing and have the means
to recover from a potential soft brick. Set this option to not enroll the
Microsoft keys:

```nix
boot.lanzaboote.autoEnrollKeys = {
  ignoreOptionRomsMaybeBrickingMyMachine = true;
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
automatic enrollment has been done by a systemd service so that `systemd-boot`
can set up the keys as part of your next boot.

## Enrolling Keys on Physical Systems

As explained above, automatic key enrollment relies on `systemd-boot` to enroll
the keys. However, by default `systemd-boot` will only actually enroll the keys
when it deems it to be safe (which it currently only does when it runs inside a
VM). To enroll the keys on a physical system, set this option:

```nix
boot.lanzaboote.settings.secure-boot-enroll = "force";
```
