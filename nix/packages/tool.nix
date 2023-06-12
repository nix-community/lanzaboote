{ systemd
, binutils-unwrapped
, sbsigntool
, rustPlatform
, lib
}:

rustPlatform.buildRustPackage
{
  pname = "lanzaboote_tool";
  version = "0.3.0";
  src = lib.cleanSource ../../rust/tool;

  TEST_SYSTEMD = systemd;

  cargoLock = {
    lockFile = ../../rust/tool/Cargo.lock;
  };

  nativeCheckInputs = [
    binutils-unwrapped
    sbsigntool
  ];

  meta = with lib; {
    description = "Lanzaboote UEFI tooling for SecureBoot enablement on NixOS systems";
    homepage = "https://github.com/nix-community/lanzaboote";
    license = licenses.mit;
  };
}
