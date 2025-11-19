# Disable Secure Boot

When you want to permanently get back to a system without the Secure
Boot stack, **first** disable Secure Boot in your firmware
settings. Then you can disable the Lanzaboote related settings in the
NixOS configuration and rebuild.

You may need to clean up the `EFI/Linux` directory in the ESP manually
to get rid of stale boot entries. **Please backup your ESP, before you
delete any files** in case something goes wrong.
