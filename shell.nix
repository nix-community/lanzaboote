let
  self = import ./default.nix { };
  inherit (self.passthru) pkgs;
in
pkgs.mkShell {
  packages = [
    pkgs.lon
    pkgs.clippy
    pkgs.rustfmt
    pkgs.cargo-release
    pkgs.cargo-machete
    pkgs.cargo-edit
    pkgs.cargo-bloat
    pkgs.lixPackageSets.latest.nix-eval-jobs

    # Convenience for test fixtures in nix/tests.
    pkgs.openssl

    # Needed for `cargo test` in rust/tool. We also need
    # TEST_SYSTEMD below for that.
    pkgs.sbsigntool
  ];

  inputsFrom = [
    self.packages.lzbt
    self.packages.stub
  ];

  shellHook = ''
    ${self.checks.pre-commit.shellHook}
  '';

  env = {
    RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
    TEST_SYSTEMD = pkgs.systemd;
  };
}
