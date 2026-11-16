let
  self = import ./default.nix { };
  inherit (self.passthru) pkgs;
in
pkgs.mkShell {
  packages = [
    pkgs.lon
    pkgs.cargo-release
    pkgs.cargo-machete
    pkgs.cargo-edit
    pkgs.cargo-bloat
    pkgs.lixPackageSets.latest.nix-eval-jobs

    # Convenience for test fixtures in nix/tests.
    pkgs.openssl

    # Needed for `cargo test` in rust/tool. sbsigntool provides `sbverify` for
    # signature verification and `sbattach` for the test fixtures. Signing is
    # done by `systemd-sbsign`, which is taken from TEST_SYSTEMD.
    pkgs.sbsigntool
  ];

  inputsFrom = [
    self.packages.lzbt
    self.packages.stub
    self.docs.html
  ];

  shellHook = ''
    ${self.checks.pre-commit.shellHook}
  '';

  env = {
    # For rust-analyzer support
    RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    TEST_SYSTEMD = pkgs.systemd;
  };
}
