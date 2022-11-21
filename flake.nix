{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, rust-overlay, naersk }:
    let
      pkgs = import nixpkgs {
        system = "x86_64-linux";
        overlays = [
          rust-overlay.overlays.default
        ];
      };

      rust = pkgs.rust-bin.fromRustupToolchainFile ./rust/rust-toolchain.toml;

      naersk' = pkgs.callPackage naersk {
        cargo = rust;
        rustc = rust;
      };
    in
      {
        packages.x86_64-linux.default = naersk'.buildPackage {
          src = ./rust;
          cargoBuildOptions = old: old ++ [
            "--target x86_64-unknown-uefi"
          ];
        };
      };
}
