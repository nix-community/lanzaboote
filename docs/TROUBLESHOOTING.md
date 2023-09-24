# Troubleshooting

## Bootloader installation fails with "Failed to install files. … No space left on device (os error 28)"

During the bootloader installation process, Lanzaboote must copy the kernel and initrd to the EFI system partition (ESP).
It is quite possible that the ESP is not large enough to hold these files for all installed generations, in which case this error occurs.

In this case, you must first delete some generations (e.g. run `nixos-collect-garbage --delete-older-than=7d` to delete all generations more than one week old).
After that, some space on the ESP must be freed manually.
To achieve this, delete some kernels and initrds in `/boot/EFI/nixos` (they will be recreated in the next step if they are in fact still required).
Finally, run `nixos-rebuild boot` again to finish the installation process that was interrupted by the error.

It is recommended run a garbage collection regularly, and monitor the ESP usage (particularly if it is quite small), to prevent this issue from happening again in the future.

**Warning:** It is recommended to not delete the currently booted kernel and initrd, and to not reboot the system before running `nixos-rebuild boot` again, to minimize the risk of accidentally rendering the system unbootable.

**Note:** When upgrading Lanzaboote from version 0.3.0, or from git master prior to the merge of PR #204, ESP space usage is temporarily doubled.
Hence it is possible for this error to occur even if there was plenty (but less than half) free space available prior to the installation.
In this case, it is not necessary to delete any generations, and you can proceed directly to deleting some kernels and initrds before running the installation again.

## Power failed during bootloader installation, and now the system does not boot any more

Due to the shortcomings of the FAT32 filesystem, in rare cases, it is possible for the ESP to become corrupted after power loss.
With Lanzaboote enabled, this will lead to "secure boot errors" or "hash verification failures" (the exact wording depends on the firmware).
In these cases, recovery is usually still possible with the steps below.

**Note:** If the system fails to boot after the Linux kernel has already been started, then the problem is not caused by a corrupted ESP.
In this case, the steps below will not help, and standard rollback procedures should be followed instead.

### The system can still boot an older generation

In case an older generation still works, the recovery can be carried out from within the booted system.
Run `nix-shell -p openssl sbctl` to ensure the tools required for recovery are available.

1. Run `sudo sbctl verify /boot/EFI/Linux/nixos-generation-*.efi` to check the Lanzaboote stubs.
   Files that have a crossmark on their left are corrupted and must be deleted.
2. Run `for file in /boot/EFI/nixos/*.efi; do hash=$(openssl dgst -sha256 -binary "$file" | base32 | tr -d = | LC_ALL=C tr [:upper:] [:lower:]); if [[ $file != *$hash.efi ]]; then echo $file; fi; done` to check the kernels and initrds.
   Any files that are printed are corrupted and must be deleted.
3. Run `nixos-rebuild boot`.
   This should reinstate all files that are required for the newer generations to boot.
4. Reboot the system, it should now work again.

### The system cannot boot any generation anymore

If no available generation can boot any more, the system must be recovered from a rescue system.
First make sure that you have a recent NixOS install medium available.

**Note:** Nix versions from before August 2023 contain a bug that can prevent `nixos-enter` from working.
A more recent medium must be used for the recovery procedure to work reliably.

1. Disable Secure Boot in the firmware settings.
   The NixOS install medium is not signed and thus cannot be booted when Secure Boot is active.
2. Boot the NixOS install medium.
3. Mount all partitions belonging to the system to be recovered under `/mnt`, just like you would for installation.
   1. In case the ESP does not mount, or only mounts in read-only mode, due to corruption, try `fsck.fat` first.
      If that fails as well or the ESP still does not mount, it needs to be reformatted using `mkfs.fat`.
4. Delete the corrupted files on the ESP, using `rm -fr /mnt/boot/EFI/Linux/nixos-generation-*.efi /mnt/boot/efi/nixos`.
5. Enter the recovery shell by running `nixos-enter`.
   Then, run `nixos-rebuild boot` to install the bootloader again.
6. Exit the recovery shell and unmount all filesystems.
7. Reboot the system to verify that everything works again.
8. Enable Secure Boot again in the firmware settings.

## The system doesn't boot with Secure Boot enabled

It is the most likely issue that Lanzaboote could not verify a cryptographic hash.
To recover from this, disable Secure Boot in your firmware settings.
Please file a bug, if you hit this issue.
