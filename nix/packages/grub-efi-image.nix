{ lib, stdenvNoCC, grub2_efi, fetchFromGitHub, git, gnulib }:
let
  grub_uki = grub2_efi.overrideAttrs (finalAttrs: previousAttrs: rec {
    patches = [ ];
    src = fetchFromGitHub {
      owner = "GovanifY";
      repo = "grub";
      rev = "master";
      sha256 = "sha256-2XhZdRCK73QJ5QIeTdqXVFinzXq36pJ/6EwnluY5rnA=";
    };


    postPatch = ''
      patchShebangs autogen.sh bootstrap
      # copy gnulib into build dir and make writable.
      # Otherwise ./bootstrap copies the non-writable files from nix store and fails to modify them
      cp -r ${gnulib} gnulib
      chmod -R u+w gnulib/{build-aux,lib}
      ./bootstrap --no-git --gnulib-srcdir=gnulib --skip-po
      ./autogen.sh
    '';
    nativeBuildInputs = previousAttrs.nativeBuildInputs ++ [ git ];
    preConfigure = "";


  });
in
stdenvNoCC.mkDerivation rec {
  pname = "grub-efi-image";
  version = "unstable-uki";

  buildCommand = ''
    mkdir $out
    cd /tmp
    # this is very probably a terrible idea but grub doesn't allow to generate
    # at runtime menus like systemd-boot does, so we will need to wait for
    # lanzaboot to be ready to generate config for both sd-boot and grub.
    echo -e 'set root=(hd0,gpt2)\nboot_loader_interface\nchainloader (hd0,gpt1)/efi/Linux/nixos-generation-1.efi\nboot' > /tmp/grub.cfg
    ${grub_uki}/bin/grub-mkstandalone -O x86_64-efi -o $out/boot.efi \
    --disable-shim-lock --modules="part_gpt part_msdos tpm boot_loader_interface" "boot/grub/grub.cfg=/tmp/grub.cfg"
  '';

  nativeBuildInputs = [
    grub_uki
  ];

  meta = with lib; {
    platforms = platforms.all;
    license = licenses.asl20;
  };
}
