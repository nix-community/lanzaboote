# Enable Secure Boot

Now that NixOS is ready for Secure Boot, we will setup the
firmware. At the end of this section, Secure Boot will be enabled on
your system and your firmware will only boot binaries that are signed
with your keys.

At least on some ASUS boards and others, you may also need to set the `OS Type` to "Windows UEFI Mode" in the Secure Boot settings, so that Secure Boot does get enabled.

These instructions are specific to ThinkPads and may need to be
adapted on other systems.

## Enter Secure Boot Setup Mode

The UEFI firmware allows enrolling Secure Boot keys when it is in
_Setup Mode_.

On a Thinkpad enter the BIOS menu using the "Reboot into Firmware"
entry in the systemd-boot boot menu. Once you are in the BIOS menu:

1. Select the "Security" tab.
2. Select the "Secure Boot" entry.
3. Set "Secure Boot" to enabled.
4. Select "Reset to Setup Mode".

When you are done, press F10 to save and exit.

You can see these steps as a video [here](https://www.youtube.com/watch?v=aLuCAh7UzzQ).

> ⚠️ Do not select "Clear All Secure Boot Keys" as it will drop the Forbidden
> Signature Database (dbx).

### Framework Devices: Enter Setup Mode

On Framework laptops (13th generation or newer) you can enter the setup mode like this:

1. Select "Administer Secure Boot"
2. Select "Erase all Secure Boot Settings"

> [!WARNING]
> **Don't** select "Erase all Secure Boot Settings" in the Framework 13 Core Ultra Series 1 firmware.
> This firmware is bugged, instead delete all keys from the "PK", "KEK" and "DB" sections manually.
> See [this](https://community.frame.work/t/cant-enable-secure-boot-setup-mode/57683/5) thread on the Framework forum.

When you are done, press F10 to save and exit.

### Microsoft Surface Devices: Disable Secure Boot

On Microsoft Surface devices (tested on Surface Book 3 and Surface Go 3), keep Secure Boot disabled in UEFI settings.
On Surface Devices, having Secure Boot disabled defaults to "setup mode", and there is no need to re-enable it in this interface.
After following these instructions, Lanzaboote should enable Secure Boot for you.

### Other Systems

On certain systems (e.g. ASUS desktop motherboards), there is no explicit option to enter Setup Mode.
Instead, choose the option to erase the existing Platform Key.

## Enroll Keys

Once you've booted your system into NixOS again, you have to enroll
your keys to activate Secure Boot. We include Microsoft keys here to
avoid boot issues.

```console
$ sudo sbctl enroll-keys --microsoft
Enrolling keys to EFI variables...
With vendor keys from microsoft...✓
Enrolled keys to the EFI variables!
```

> ⚠️ During boot, some hardware might include OptionROMs signed with
> Microsoft keys.
> By using the `--microsoft`, we enroll the Microsoft OEM certificates.
> Another more experimental option would be to enroll OptionROMs checksum seen
> at last boot using `--tpm-eventlog`, but these checksums might change later.

You can now reboot your system. After you've booted, Secure Boot is
activated and in user mode:

```console
$ bootctl status
System:
      Firmware: UEFI 2.70 (Lenovo 0.4720)
 Firmware Arch: x64
   Secure Boot: enabled (user)
  TPM2 Support: yes
  Boot into FW: supported
```

> ⚠️  If you used `--microsoft` while enrolling the keys, you might want
> to check that the Secure Boot Forbidden Signature Database (dbx) is not
> empty.
> A quick and dirty way is by checking the file size of
> `/sys/firmware/efi/efivars/dbx-*`.
> Keeping an up to date dbx reduces Secure Boot bypasses, see for example:
> <https://uefi.org/sites/default/files/resources/dbx_release_info.pdf>.

### Framework Devices: Enable Secure Boot

On Framework laptops you may need to manually enable Secure Boot:

1. Select "Administer Secure Boot"
2. Enable "Enforce Secure Boot"

When you are done, press F10 to save and exit.

That's all! 🥳
