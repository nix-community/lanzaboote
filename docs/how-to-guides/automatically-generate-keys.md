# Automatically Generate Keys

You can automatically generate Secure Boot keys in the directory specified via
`boot.lanzaboote.pkiBundle` when they do not exist yet via a Lanzaboote option:
```nix
boot.lanzaboote.autoGenerateKeys.enable = true;
```

Note that you Lanzaboote does nothing to keep your keys secure. You need to do
this yourself, e.g. by using full disk encryption.

This is implemented as a systemd service, so you will need to actually start
the system to generate the keys. They will not be generated as part of
`switch-to-configuration` or `nixos-install`.

You can combine generating keys with [automatically enrolling
them](automatically-enroll-keys.md) to set up Secure Boot in one go.
