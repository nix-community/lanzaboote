let
  self = import ./default.nix { };
  inherit (self.passthru) pkgs;
  cargo-release = pkgs.cargo-release.overrideAttrs (_: {
    # Upstream snapshot tests are currently failing in nixpkgs, which breaks
    # shell evaluation even though this tool is only a dev-shell convenience.
    doCheck = false;
  });
in
pkgs.mkShell {
  packages = [
    pkgs.lon
    cargo-release
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
