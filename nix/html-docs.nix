{
  lib,
  stdenv,
  mdbook,
}:

stdenv.mkDerivation {
  name = "lanzaboote-docs-html";

  src = lib.sourceByRegex ../. [
    "book.toml"
    "^docs.*"
  ];

  nativeBuildInputs = [
    mdbook
  ];

  buildPhase = ''
    mdbook build -d $out
  '';
}
