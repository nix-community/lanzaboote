

# UEFI Helpers

How to boot a VM: https://rust-osdev.github.io/uefi-rs/HEAD/tutorial/vm.html

```
nix build  --builders "" && cp -f result/bin/lanzaboote.efi esp/EFI/Linux/lanzaboote.efi && qemu-uefi -drive format=raw,file=fat:rw:esp
```
