{ pkgs ? import (fetchTarball
  "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz") { } }:

with pkgs;

mkShell rec {
  nativeBuildInputs = [ pkg-config cargo rustc rust-analyzer rustfmt clippy ];
  buildInputs = [ cmake libclang openssl udev wayland libxkbcommon libGL ];
  LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
}

