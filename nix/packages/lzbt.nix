{
  lib,
  buildRustApp,
  makeBinaryWrapper,
  binutils-unwrapped,
  sbsigntool,
  systemd,
  stub,
}:

buildRustApp {
  pname = "lzbt-systemd";
  src = lib.sourceFilesBySuffices ../../rust/tool [
    ".rs"
    ".toml"
    ".lock"
    # Test fixtures
    ".pem"
    ".key"
  ];
  packageArgs = {
    nativeBuildInputs = [
      makeBinaryWrapper
    ];

    nativeCheckInputs = [
      binutils-unwrapped
      sbsigntool
    ];

    env.TEST_SYSTEMD = systemd;

    postInstall =
      let
        path = lib.makeBinPath [
          binutils-unwrapped
          sbsigntool
        ];
      in
      ''
        makeWrapper $out/bin/lzbt-systemd $out/bin/lzbt \
          --set PATH ${path} \
          --set LANZABOOTE_STUB ${stub}/bin/lanzaboote_stub.efi
      '';

    meta.mainProgram = "lzbt";
  };
}
