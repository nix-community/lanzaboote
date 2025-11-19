{
  pkgs,
  nixosModules,
}:

let
  eval = pkgs.lib.evalModules {
    modules = [
      (
        { lib, ... }:
        {
          config._module.args.pkgs = pkgs;
          imports = [
            lib.types.noCheckForDocsModule
            # Needed for the assertions from the repart module.
            "${pkgs.path}/nixos/modules/misc/assertions.nix"
          ]
          ++ builtins.attrValues nixosModules;
        }
      )
    ];
  };
  optionsDoc = pkgs.nixosOptionsDoc {
    inherit (eval) options;
  };
in
optionsDoc.optionsCommonMark
