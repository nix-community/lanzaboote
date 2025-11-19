{ pkgs }:

let
  sources = import ../sources.nix;
  pre-commit.run = pkgs.callPackage "${sources.pre-commit}/nix/run.nix" {
    inherit pkgs;
    tools = import "${sources.pre-commit}/nix/call-tools.nix" pkgs;
    # Trick pre-commit into not needing gitignore.nix
    isFlakes = true;
    gitignore-nix-src = { };
  };

  globalExcludes = [
    "lon.nix"
    "sources.nix"
  ];
in
pre-commit.run {
  src = pkgs.nix-gitignore.gitignoreSource [ ] ../.;
  hooks = {
    nixfmt-rfc-style = {
      enable = true;
      excludes = globalExcludes;
    };
    deadnix = {
      enable = true;
      excludes = globalExcludes;
    };
    typos.enable = true;
  };
}
