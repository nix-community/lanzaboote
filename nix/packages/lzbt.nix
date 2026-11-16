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
      # `sbverify` and `sbattach` are still used to verify signatures and in the
      # test fixtures. Signing uses `systemd-sbsign`.
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
          --prefix PATH : ${path} \
          --set LANZABOOTE_STUB ${stub}/bin/lanzaboote_stub.efi
      '';

    meta.mainProgram = "lzbt";
  };
}
