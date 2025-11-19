{
  description = "Secure Boot for NixOS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    # Not used in the flake itself. Only used to make the source available for
    # the project.
    pre-commit = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "github:ipetkov/crane";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      ...
    }:
    let
      eachSystem = nixpkgs.lib.genAttrs [
        "x86_64-linux"
        # Not tested in CI. Best effort support.
        "aarch64-linux"
      ];

      # Instantiate only once for each system.
      #
      # Still allow flakes users to override dependencies in the normal flake
      # way.
      lanzaboote = eachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        import ./. {
          inherit system pkgs rust-overlay;
          crane = crane.mkLib pkgs;
        }
      );
    in
    {
      nixosModules.lanzaboote = (
        { pkgs, ... }:
        {
          imports = [
            ./nix/modules/lanzaboote.nix
          ];

          boot.lanzaboote.package =
            let
              system = pkgs.stdenv.hostPlatform.system;
            in
            self.packages.${system}.lzbt;
        }
      );

      packages = eachSystem (
        system: builtins.removeAttrs lanzaboote.${system}.packages [ "recurseForDerivations" ]
      );

      # Temporarily include the checks in the flake so that CI picks them up.
      checks = eachSystem (
        system:
        let
          checks = lanzaboote.${system}.checks;
        in
        {
          tool = checks.stub.package;
          toolClippy = checks.stub.clippy;
          toolRustfmt = checks.stub.rustfmt;

          stub = checks.stub.package;
          stubClippy = checks.stub.clippy;
          stubRustfmt = checks.stub.rustfmt;

          docsOptions = checks.docs.options;

          inherit (checks) pre-commit;
        }
        // builtins.removeAttrs checks.tests [ "recurseForDerivations" ]
      );

    };
}
