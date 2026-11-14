# Encrypt Initrd Secrets

Initrd secrets are normally appended to the initrd in plaintext, so anybody who
can read the unencrypted ESP can read them. With `boot.lanzaboote.encryptedSecrets.enable`,
Lanzaboote encrypts them against the TPM instead and only appends the ciphertext.
The initrd decrypts them back into place before anything consumes them.

## Prerequisites

- Secure Boot must be enabled: the decryption key is only released for images
  signed with your Secure Boot key.
- The Secure Boot keys must exist when the boot loader is installed. This
  means if you have `boot.lanzaboote.autoGenerateKeys.enable` set to `true`, you
  will need to boot at least once after the keys have been generated and
  enrolled before being able to enable `boot.lanzaboote.encryptedSecrets`.
- A TPM 2.0 device must be present, TPM2 support in the initrd must be enabled,
  and initrd PCR phases must be enabled so decryption is scoped to the initrd.

## Configure Your Secrets

Configure `boot.initrd.secrets` as usual. Every destination must be a regular
file.

```nix
boot.initrd.secrets = {
  "/etc/dropbear/dropbear_rsa_host_key" = "/etc/secrets/initrd_dropbear_rsa_host_key";
};
```

## Enable Encryption

```nix
boot.initrd.systemd.enable = true;
boot.initrd.systemd.tpm2.enable = true;
boot.initrd.systemd.tpm2.pcrphases.enable = true;
boot.lanzaboote.encryptedSecrets.enable = true;
```

Then install the boot loader as usual. Secrets are sealed against a PCR 11
policy signed for the exact generation. The signatures are kept in the global
credential store on the ESP, so every installed generation stays bootable.

## Limitations

Secrets are sealed when the boot loader is installed. Installing in a sandbox
(e.g. while building a disk image) therefore needs a TPM device to be available.

The TPM verifies the PCR signature, so the policy key must be RSA and no larger
than the TPM supports. A dedicated 2048 bit key is generated under
`/var/lib/lanzaboote/` if `boot.lanzaboote.encryptedSecrets.privateKeyFile`
does not exist yet, so the regular (possibly 4096 bit) Secure Boot keys are not
used for this. RSA keys larger than 2048 bits make decryption fail with a TPM
integrity error on most hardware.
