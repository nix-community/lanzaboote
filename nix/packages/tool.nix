{ stdenv
, systemd
, binutils-unwrapped
, sbsigntool
, rustPlatform
, lib
, runCommand
, fetchurl
, clippy
, rustfmt
, path
, enableLint ? false
, enableFmt ? false
}:
rustPlatform.buildRustPackage
  ({
    pname = "lanzaboote_tool";
    version = "0.3.0";
    src = runCommand "src" { } ''
      install -D ${../../rust/tool/Cargo.toml} $out/Cargo.toml
      install -D ${../../rust/tool/Cargo.lock} $out/Cargo.lock
      cp -r ${../../rust/tool/src} $out/src
    '';

    TEST_SYSTEMD = systemd;

    nativeBuildInputs = lib.optional enableLint clippy ++ lib.optional enableFmt rustfmt;

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
  } // lib.optionalAttrs enableLint {
    doCheck = false;
    buildPhase = ''
      cargo clippy --all-targets --all-features -- -D warnings
      if grep -R 'dbg!' ./src; then
        echo "use of dbg macro found in code!"
        false
      fi
    '';

    installPhase = ''
      touch $out
    '';
  } // lib.optionalAttrs enableFmt {
    doCheck = false;

    buildPhase = ''
      echo "checking formatting..."
      cargo fmt --all -- --check
    '';

    installPhase = ''
      touch $out
    '';
  })
