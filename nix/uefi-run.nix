{ fetchFromGitHub, naersk, makeWrapper, OVMF, qemu }:
naersk.buildPackage {
  src = fetchFromGitHub {
    owner = "Richard-W";
    repo = "uefi-run";

    rev = "8ba33c934525458a784a6620705bcf46c3ca91d2";
    sha256 = "fwzWdOinW/ECVI/65pPB1shxPdl2nZThAqlg8wlWg/g=";
  };

  nativeBuildInputs = [ makeWrapper ];

  postInstall = ''
    wrapProgram "$out/bin/uefi-run" \
      --add-flags '--bios-path ${OVMF.fd}/FV/OVMF.fd --qemu-path ${qemu}/bin/qemu-system-x86_64'
  '';
}
