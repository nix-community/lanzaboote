# Install on Multiple ESPs

If you're running some kind of RAID setup where you split your OS over multiple
disks, you might also want to install the boot artifacts to multiple ESPs over
multiple disks. This feature is sometimes called "mirrored boot".

To install the boot artifacts to multiple mount points:

```nix
boot.lanzaboote.extraEfiSysMountPoints =  [ "/boot2" "/boot3" ];
```
