{ lib, stdenvNoCC, grub2_efi }:
stdenvNoCC.mkDerivation rec {
  pname = "grub-efi-image";
  version = grub2_efi.version;

  buildCommand = ''
    mkdir $out
    cd /tmp
    # this is very probably a terrible idea but grub doesn't allow to generate
    # at runtime menus like systemd-boot does, so we will need to wait for
    # lanzaboot to be ready to generate config for both sd-boot and grub.
    echo -e 'chainloader (hd0,gpt1)/efi/Linux/nixos-generation-1.efi\nboot' > /tmp/grub.cfg
    ${grub2_efi}/bin/grub-mkstandalone -O x86_64-efi -o $out/boot.efi \
    --disable-shim-lock --modules="part_gpt part_msdos" "boot/grub/grub.cfg=/tmp/grub.cfg"
  '';

  meta = with lib; {
    platforms = platforms.all;
    license = licenses.asl20;
  };
}
