{ lib, stdenvNoCC, grub2_efi }:
stdenvNoCC.mkDerivation rec {
  pname = "grub-efi-image";
  version = grub2_efi.version;

  buildCommand = ''
    mkdir $out
    ${grub2_efi}/bin/grub-mkimage -O x86_64-efi -p "" -o $out/boot.efi
  '';

  meta = with lib; {
    platforms = platforms.all;
    license = licenses.asl20;
  };
}
