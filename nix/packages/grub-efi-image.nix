{ lib, stdenvNoCC, grub2_efi }:
stdenvNoCC.mkDerivation rec {
  pname = "grub-efi-image";
  version = grub2_efi.version;

  buildCommand = ''
    mkdir $out
    ${grub2_efi}/bin/grub-mkstandalone -O x86_64-efi -o $out/boot.efi --modules="part_gpt part_msdos"
  '';

  meta = with lib; {
    platforms = platforms.all;
    license = licenses.asl20;
  };
}
