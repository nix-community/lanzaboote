{ pkgs }:

let
  sources = import ../lon.nix;
  pre-commit = import sources."pre-commit";

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
