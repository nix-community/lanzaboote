{ rust, rustPlatform, clippy, rustfmt, stdenv, lib, runCommand, enableFmt ? false, enableLint ? false, fatVariant ? false }:

let
  targetSpec = rust.toRustTargetSpec stdenv.hostPlatform;
  targetIsJSON = lib.hasSuffix ".json" targetSpec;
  shortTarget =
    if targetIsJSON then
      (lib.removeSuffix ".json" (builtins.baseNameOf "${targetSpec}"))
    else targetSpec;
in
rustPlatform.buildRustPackage
  ({
    pname = "lanzaboote_stub";
    version = "0.3.0";
    src = runCommand "src" { } ''
      install -D ${../../rust/stub/Cargo.toml} $out/Cargo.toml
      install -D ${../../rust/stub/Cargo.lock} $out/Cargo.lock
      cp -r ${../../rust/stub/src} $out/src
    '';

  # We don't want the thin code.
  buildNoDefaultFeatures = true;
  buildFeatures = if fatVariant then [ "fat" ] else [ "thin" ];

    # We don't want the thin code.
    buildNoDefaultFeatures = fatVariant;
    buildFeatures = lib.optional fatVariant "fat";

    cargoLock = {
      lockFile = ../../rust/stub/Cargo.lock;
    };

    # Necessary because our `cc-wrapper` doesn't understand MSVC link options.
    RUSTFLAGS = "-Clinker=${stdenv.cc.bintools}/bin/${stdenv.cc.targetPrefix}ld.lld -Clinker-flavor=lld-link";
    # Necessary because otherwise we will get (useless) hardening options in front of
    # -flavor link which will break the whole command-line processing for the ld.lld linker.
    hardeningDisable = [ "all" ];

    meta = with lib; {
      description = "Lanzaboote UEFI stub for SecureBoot enablement on NixOS systems";
      homepage = "https://github.com/nix-community/lanzaboote";
      license = licenses.mit;
      platforms = [ "x86_64-windows" "aarch64-windows" "i686-windows" ];
    };
  } // lib.optionalAttrs enableLint {
    buildPhase = ''
      cargo clippy --target ${shortTarget} --all-features -- -D warnings
      if grep -R 'dbg!' ./src; then
        echo "use of dbg macro found in code!"
        false
      fi
    '';

    installPhase = ''
      touch $out
    '';
  } // lib.optionalAttrs enableFmt {
    buildPhase = ''
      echo "checking formatting..."
      cargo fmt --all -- --check
    '';

    installPhase = ''
      touch $out
    '';
  })
