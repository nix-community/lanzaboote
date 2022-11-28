{ fetchFromGitHub, craneLib, makeWrapper, OVMF, qemu }:
craneLib.buildPackage {
  src = fetchFromGitHub {
    owner = "Richard-W";
    repo = "uefi-run";

    rev = "8ba33c934525458a784a6620705bcf46c3ca91d2";
    sha256 = "fwzWdOinW/ECVI/65pPB1shxPdl2nZThAqlg8wlWg/g=";
  };

  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    # The hook runs for the dependency-only derivation where the binary is not
    # produced. We need to skip it there.
    if [ -f $out/bin/uefi-run ]; then
      wrapProgram "$out/bin/uefi-run" \
        --add-flags '--bios-path ${OVMF.fd}/FV/OVMF.fd --qemu-path ${qemu}/bin/qemu-system-x86_64'
    fi
  '';
}
