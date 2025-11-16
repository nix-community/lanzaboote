{
  description = "Secure Boot for NixOS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

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
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              rust-overlay.overlays.default
            ];
          };
        in
        import ./nix/packages {
          inherit pkgs;
          crane = crane.mkLib pkgs;
        }
      );
    };
}
