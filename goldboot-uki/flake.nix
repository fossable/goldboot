{
  description = "Goldboot - Immutable infrastructure for bare metal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" ];
        };

        # Common build inputs for Rust projects
        rustBuildInputs = with pkgs; [
          pkg-config
          openssl
        ];

        # GTK4 dependencies for goldboot-uki
        gtkDeps = with pkgs; [
          gtk4
          gdk-pixbuf
          glib
          cairo
          pango
          graphene
          libdrm
          udev  # Required by libudev-sys (block-utils dependency)
        ];

      in
      {
        packages = {
          # Build the goldboot-uki binary
          goldboot-uki = pkgs.rustPlatform.buildRustPackage {
            pname = "goldboot-uki";
            version = "0.0.3";

            # Source is the parent directory (workspace root)
            src = pkgs.lib.cleanSource ../.;

            cargoLock = {
              lockFile = ../Cargo.lock;
            };

            nativeBuildInputs = rustBuildInputs ++ [
              rustToolchain
              pkgs.python3  # Required by pyo3-build-config
            ];

            buildInputs = gtkDeps ++ rustBuildInputs;

            # Build only the goldboot-uki package from workspace
            cargoBuildFlags = [ "-p" "goldboot-uki" ];

            meta = with pkgs.lib; {
              description = "Goldboot UKI - Image deployment tool in a Unified Kernel Image";
              homepage = "https://goldboot.org";
              license = licenses.unlicense;
              maintainers = [ ];
            };
          };

          # Build the complete UKI image
          goldboot-uki-image = pkgs.stdenv.mkDerivation {
            pname = "goldboot-uki-image";
            version = "0.0.3";

            src = pkgs.lib.cleanSource ../.;

            nativeBuildInputs = with pkgs; [
              binutils
              cpio
              dracut
              kmod
              systemd  # for ukify and systemd-boot stub
            ];

            buildInputs = with pkgs; [
              linux
            ] ++ gtkDeps;

            buildPhase = ''
              set -x

              # Create working directory
              mkdir -p $out/build
              cd $out/build

              # Copy goldboot-uki binary
              cp ${self.packages.${system}.goldboot-uki}/bin/goldboot-uki ./goldboot-uki
              chmod +x ./goldboot-uki

              # Set up dracut module (from goldboot-uki/dracut)
              DRACUT_MODULE_DIR="./dracut-modules/99goldboot"
              mkdir -p "$DRACUT_MODULE_DIR"

              cp -r $src/goldboot-uki/dracut/99goldboot/* "$DRACUT_MODULE_DIR/"
              chmod +x "$DRACUT_MODULE_DIR/module-setup.sh"
              chmod +x "$DRACUT_MODULE_DIR/goldboot-init.sh"

              # Get kernel version from nixpkgs
              KERNEL_VERSION="${pkgs.linux.version}"
              KERNEL_IMAGE="${pkgs.linux}/bzImage"

              # Create initramfs with dracut
              export goldboot_uki_path="$(pwd)/goldboot-uki"
              export DRACUT_MODULE_PATH="$(pwd)/dracut-modules"

              ${pkgs.dracut}/bin/dracut \
                --force \
                --kver "$KERNEL_VERSION" \
                --kmoddir "${pkgs.linux}/lib/modules/$KERNEL_VERSION" \
                --add "goldboot drm" \
                --add-drivers "i915 amdgpu radeon nouveau virtio_gpu bochs" \
                --install "lsblk blkid blockdev" \
                --no-hostonly \
                --no-hostonly-cmdline \
                --modules-dir "$DRACUT_MODULE_PATH" \
                initramfs.img

              echo "Initramfs created: $(du -h initramfs.img)"

              # Create kernel command line
              echo "quiet loglevel=0 rd.systemd.show_status=no rd.udev.log_level=0 console=tty0" > cmdline.txt

              # Create minimal os-release
              cat > os-release <<EOF
              NAME="Goldboot UKI"
              ID=goldboot-uki
              VERSION="${self.packages.${system}.goldboot-uki-image.version}"
              PRETTY_NAME="Goldboot UKI"
              HOME_URL="https://goldboot.org"
              EOF

              # Build UKI using objcopy
              SYSTEMD_STUB="${pkgs.systemd}/lib/systemd/boot/efi/linuxx64.efi.stub"

              if [ ! -f "$SYSTEMD_STUB" ]; then
                echo "Error: systemd EFI stub not found at $SYSTEMD_STUB"
                exit 1
              fi

              ${pkgs.binutils}/bin/objcopy \
                --add-section .osrel=os-release --change-section-vma .osrel=0x20000 \
                --add-section .cmdline=cmdline.txt --change-section-vma .cmdline=0x30000 \
                --add-section .linux="$KERNEL_IMAGE" --change-section-vma .linux=0x2000000 \
                --add-section .initrd=initramfs.img --change-section-vma .initrd=0x3000000 \
                "$SYSTEMD_STUB" $out/goldboot.efi

              echo "UKI created: $(du -h $out/goldboot.efi)"
            '';

            installPhase = ''
              # Already installed to $out/goldboot.efi in buildPhase
              echo "UKI installed to $out/goldboot.efi"
            '';

            meta = with pkgs.lib; {
              description = "Goldboot UKI - Complete bootable image";
              homepage = "https://goldboot.org";
              license = licenses.unlicense;
            };
          };

          default = self.packages.${system}.goldboot-uki-image;
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo
            rustfmt
            clippy
            rust-analyzer

            # Build tools
            pkg-config
            binutils
            dracut
            systemd

            # GTK4 dependencies
          ] ++ gtkDeps ++ rustBuildInputs;

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "Goldboot development environment"
            echo "Rust version: $(rustc --version)"
            echo ""
            echo "Available commands:"
            echo "  nix build .#goldboot-uki        - Build just the binary"
            echo "  nix build .#goldboot-uki-image  - Build the complete UKI"
            echo "  nix build                       - Build default (UKI image)"
          '';
        };
      }
    );
}
