{ pkgs ? import <nixpkgs> { } }:

with pkgs;

mkShell rec {
  nativeBuildInputs = [ pkg-config cargo rustc rust-analyzer rustfmt clippy ];
  buildInputs = [ cmake libclang openssl udev pango gtk4 ];
  LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
}

