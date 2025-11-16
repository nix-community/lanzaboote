{
  system ? builtins.currentSystem,
}:

let
  sources = import ./lon.nix;
  rust-overlay = import sources.rust-overlay;
  pkgs = import sources.nixpkgs {
    inherit system;
    overlays = [ rust-overlay ];
  };
  crane = import sources.crane { inherit pkgs; };
  inherit (pkgs) lib;
in
rec {
  packages = lib.recurseIntoAttrs (
    import ./nix/packages {
      inherit pkgs crane;
    }
  );

  checks = lib.recurseIntoAttrs {
    stub = lib.recurseIntoAttrs {
      package = packages.stub;
      inherit (packages.stub.tests)
        clippy
        rustfmt
        ;
    };
    lzbt = lib.recurseIntoAttrs {
      package = packages.stub;
      inherit (packages.stub.tests)
        clippy
        rustfmt
        ;
    };

    pre-commit = import ./nix/pre-commit.nix { inherit pkgs; };

    tests = lib.recurseIntoAttrs (
      import ./nix/tests {
        inherit pkgs;
        extraBaseModules = {
          lanzaboote = ./nix/modules/lanzaboote.nix;
          testInstrumentation = {
            boot.lanzaboote.package = packages.lzbt;
          };
        };
      }
    );
  };

  passthru = { inherit pkgs; };
}
