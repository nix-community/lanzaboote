{
  clippy = rustPackage: { lib, rust, clippy }:
    let
      targetSpec = rust.toRustTargetSpec rustPackage.stdenv.hostPlatform;
      inherit (lib) optionalString concatStringsSep;
    in
    rustPackage.overrideAttrs (old: {
      nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ [ clippy ];

      doCheck = false;

      buildPhase = ''
        echo "checking via clippy..."
        cargo clippy --target ${targetSpec} ${optionalString (old.buildNoDefaultFeatures or false) "--no-default-features "}${optionalString ((old.buildFeatures or null) != null) ''--features="${concatStringsSep " " old.buildFeatures}" ''}-- -D warnings
        if grep -R 'dbg!' ./src; then
          echo "use of dbg macro found in code!"
          false
        fi
      '';

      installPhase = ''
        touch $out
      '';
    });
  rustfmt = rustPackage: { rustfmt }: rustPackage.overrideAttrs (old: {
    nativeBuildInputs = (old.nativeBuildInputs or [ ]) ++ [ rustfmt ];

    doCheck = false;

    buildPhase = ''
      echo "checking formatting..."
      cargo fmt --all -- --check
    '';

    installPhase = ''
      touch $out
    '';
  });
}
