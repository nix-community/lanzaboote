{
  description = "Secure Boot for NixOS";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable-small";

    flake-parts.url = "github:hercules-ci/flake-parts";
    flake-parts.inputs.nixpkgs-lib.follows = "nixpkgs";

    # Only used during development, can be disabled by flake users like this:
    #  lanzaboote.inputs.pre-commit-hooks-nix.follows = "";
    pre-commit-hooks-nix = {
      url = "github:cachix/pre-commit-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-compat.follows = "flake-compat";
    };

    crane = {
      url = "github:ipetkov/crane";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = inputs@{ self, nixpkgs, crane, rust-overlay, flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } ({ moduleWithSystem, ... }: {
      imports = [
        # Derive the output overlay automatically from all packages that we define.
        inputs.flake-parts.flakeModules.easyOverlay

        # Formatting and quality checks.
      ] ++ (if inputs.pre-commit-hooks-nix ? flakeModule then [ inputs.pre-commit-hooks-nix.flakeModule ] else [ ]);

      flake.nixosModules.lanzaboote = moduleWithSystem (
        perSystem@{ config }:
        { ... }: {
          imports = [
            ./nix/modules/lanzaboote.nix
          ];

          boot.lanzaboote.package = perSystem.config.packages.tool;
        }
      );

      systems = [
        "x86_64-linux"

        # Not actively tested, but may work:
        "aarch64-linux"
      ];

      perSystem = { config, system, pkgs, ... }:
        let
          rustTarget = "${pkgs.stdenv.hostPlatform.qemuArch}-unknown-uefi";
          pkgs = import nixpkgs {
            system = system;
            overlays = [
              rust-overlay.overlays.default
            ];
          };

          inherit (pkgs) lib;

          uefi-rust-stable = pkgs.rust-bin.fromRustupToolchainFile ./rust/uefi/rust-toolchain.toml;
          craneLib = (crane.mkLib pkgs).overrideToolchain uefi-rust-stable;

          # Build attributes for a Rust application.
          buildRustApp = lib.makeOverridable (
            { pname
            , src
            , target ? null
            , doCheck ? true
              # By default, it builds the default members of the workspace.
            , packages ? null
            , extraArgs ? { }
            }:
            let
              commonArgs = {
                inherit pname;
                inherit src;
                CARGO_BUILD_TARGET = target;
                inherit doCheck;

                # Workaround for https://github.com/ipetkov/crane/issues/262.
                dummyrs = pkgs.writeText "dummy.rs" ''
                  #![allow(unused)]

                  #![cfg_attr(
                    any(target_os = "none", target_os = "uefi"),
                    no_std,
                    no_main,
                  )]

                  #[cfg_attr(any(target_os = "none", target_os = "uefi"), panic_handler)]
                  fn panic(_info: &::core::panic::PanicInfo<'_>) -> ! {
                      loop {}
                  }

                  #[cfg_attr(any(target_os = "none", target_os = "uefi"), export_name = "efi_main")]
                  fn main() {}
                '';

                cargoExtraArgs = (extraArgs.cargoExtraArgs or "") + (if packages != null then (lib.concatStringsSep " " (map (p: "--package ${p}") packages)) else "");
              } // builtins.removeAttrs extraArgs [ "cargoExtraArgs" ];

              cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            in
            {
              package = craneLib.buildPackage (commonArgs // {
                inherit cargoArtifacts;
              });

              clippy = craneLib.cargoClippy (commonArgs // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "-- --deny warnings";
              });

              rustfmt = craneLib.cargoFmt (commonArgs // { inherit cargoArtifacts; });
            }
          );

          stubCrane = buildRustApp {
            pname = "lanzaboote-stub";
            src = craneLib.cleanCargoSource ./rust/uefi;
            target = rustTarget;
            doCheck = false;
          };

          stub = stubCrane.package;

          # TODO: when we will have more backends
          # let's generalize this properly.
          toolCrane = buildRustApp {
            pname = "lzbt-systemd";
            src = ./rust/tool;
            extraArgs = {
              TEST_SYSTEMD = pkgs.systemd;
              nativeCheckInputs = with pkgs; [
                binutils-unwrapped
                sbsigntool
              ];
            };
          };

          tool = toolCrane.package;

          wrappedTool = pkgs.runCommand "lzbt"
            {
              nativeBuildInputs = [ pkgs.makeWrapper ];
              meta.mainProgram = "lzbt";
            } ''
            mkdir -p $out/bin

            # Clean PATH to only contain what we need to do objcopy. Also
            # tell lanzatool where to find our UEFI binaries.
            makeWrapper ${tool}/bin/lzbt-systemd $out/bin/lzbt \
              --set PATH ${lib.makeBinPath [ pkgs.binutils-unwrapped pkgs.sbsigntool ]} \
              --set LANZABOOTE_STUB ${stub}/bin/lanzaboote_stub.efi
          '';
        in
        {
          packages = {
            inherit stub;
            tool = wrappedTool;
            lzbt = wrappedTool;
          };

          overlayAttrs = {
            inherit (config.packages) tool;
          };

          checks = {
            toolClippy = toolCrane.clippy;
            stubClippy = stubCrane.clippy;
            toolFmt = toolCrane.rustfmt;
            stubFmt = stubCrane.rustfmt;
          } // (import ./nix/tests {
            inherit pkgs;
            extraBaseModules = {
              inherit (self.nixosModules) lanzaboote;
            };
          });
          devShells.default = pkgs.mkShell {
            shellHook = ''
              ${config.pre-commit.installationScript}
            '';

            packages = [
              pkgs.nixpkgs-fmt
              pkgs.statix
              pkgs.cargo-release
              pkgs.cargo-machete

              # Convenience for test fixtures in nix/tests.
              pkgs.openssl

              # Needed for `cargo test` in rust/tool. We also need
              # TEST_SYSTEMD below for that.
              pkgs.sbsigntool
            ];

            inputsFrom = [
              config.packages.stub
              config.packages.tool
            ];

            TEST_SYSTEMD = pkgs.systemd;
          };
        } // lib.optionalAttrs (inputs.pre-commit-hooks-nix ? flakeModule) {
          pre-commit = {
            check.enable = true;

            settings.hooks = {
              nixpkgs-fmt.enable = true;
              typos.enable = true;
            };
          };
        };
    });
}
