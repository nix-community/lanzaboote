# Read sources from flake.lock
let
  lock = builtins.fromJSON (builtins.readFile ./flake.lock);

  fetchSource =
    args:
    builtins.fetchTarball {
      url = "https://github.com/${args.owner}/${args.repo}/archive/${args.rev}.tar.gz";
      sha256 = args.narHash;
    };
in
builtins.mapAttrs (name: value: fetchSource value.locked) lock.nodes
