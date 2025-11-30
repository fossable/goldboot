{
  description = "Goldboot - Immutable infrastructure for bare metal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Build the goldboot-uki binary
        goldboot-uki = pkgs.rustPlatform.buildRustPackage {
          pname = "goldboot-uki";
          version = "0.0.3";

          src = ../.;

          cargoLock = { lockFile = ../Cargo.lock; };

          nativeBuildInputs = with pkgs; [
            pkg-config
            openssl
            python3 # Required by pyo3-build-config
          ];

          buildInputs = with pkgs; [
            gtk4
            gdk-pixbuf
            glib
            cairo
            pango
            graphene
            libdrm
            udev # Required by libudev-sys (block-utils dependency)
          ];

          # Build only the goldboot-uki package from workspace
          cargoBuildFlags = [ "-p" "goldboot-uki" ];

          # Skip tests for UKI build
          doCheck = false;
        };

        # Minimal init script
        initScript = pkgs.writeScript "init" ''
          #!/bin/busybox sh

          set -e

          # Create busybox symlinks
          /bin/busybox --install -s /bin

          # Mount essential filesystems
          mount -t proc proc /proc
          mount -t sysfs sys /sys
          mount -t devtmpfs dev /dev

          # Create necessary directories
          mkdir -p /tmp /run /var

          # Set up networking (lo interface)
          ip link set lo up

          # Launch goldboot
          exec /sbin/goldboot-uki
        '';

        # Create initramfs with makeInitrd
        buildInitramfs = kernel:
          pkgs.makeInitrd {
            name = "sandpolis-initramfs";

            # Use gzip compression (standard for initramfs)
            compressor = "gzip";

            contents = [
              # Include the init script at /init
              {
                object = initScript;
                symlink = "/init";
                mode = "0755";
              }

              # Include the goldboot binary
              {
                object = "${goldboot-uki}/bin/goldboot-uki";
                symlink = "/sbin/goldboot-uki";
                mode = "0755";
              }

              # Include busybox for shell utilities
              {
                object = "${pkgs.busybox}/bin/busybox";
                symlink = "/bin/busybox";
                mode = "0755";
              }
            ];
          };

        kernel = pkgs.linuxPackages_latest.kernel;

        initramfs = buildInitramfs kernel;

        # Build the UKI (Unified Kernel Image)
        goldboot-uki-image = pkgs.stdenv.mkDerivation {
          name = "sandpolis-uki";

          nativeBuildInputs = [
            pkgs.systemdUkify # provides ukify
            pkgs.binutils
          ];

          buildCommand = ''
            mkdir -p $out

            # List initramfs contents for debugging
            echo "Initramfs contents:"
            zcat ${initramfs}/initrd | ${pkgs.cpio}/bin/cpio -itv

            # Use ukify to create the UKI
            ${pkgs.systemdUkify}/bin/ukify build \
              --linux=${kernel}/bzImage \
              --initrd=${initramfs}/initrd \
              --os-release='NAME="Goldboot"
            ID=goldboot
            VERSION="0.1.0"' \
              --cmdline="console=ttyS0 console=tty0 quiet" \
              --output=$out/goldboot.efi

            echo "UKI created at $out/goldboot.efi"
          '';
        };

        # Run scripts for QEMU testing
        run-x86_64 = pkgs.writeShellScriptBin "run-x86_64" ''
          # Set up ESP directory structure in temp directory
          ESP_DIR=$(mktemp -d)
          mkdir -p $ESP_DIR/EFI/Boot
          cp result/sandpolis.efi $ESP_DIR/EFI/Boot/BootX64.efi

          qemu-system-x86_64 \
            -nodefaults --enable-kvm -m 256M -machine q35 -smp 4 \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd,readonly=on \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_VARS.fd,readonly=on \
            -drive format=raw,file=fat:rw:$ESP_DIR \
            -netdev user,id=user.0 -device rtl8139,netdev=user.0 \
            -serial stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04 -vga std

          rm -rf $ESP_DIR
        '';

        run-aarch64 = pkgs.writeShellScriptBin "run-aarch64" ''
          # Set up ESP directory structure in temp directory
          ESP_DIR=$(mktemp -d)
          mkdir -p $ESP_DIR/EFI/Boot
          cp result/sandpolis.efi $ESP_DIR/EFI/Boot/BootAA64.efi

          qemu-system-aarch64 \
            -nodefaults --enable-kvm -m 256M -machine virt -cpu cortex-a72 -smp 4 \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd,readonly=on \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_VARS.fd,readonly=on \
            -drive format=raw,file=fat:rw:$ESP_DIR \
            -netdev user,id=user.0 -device rtl8139,netdev=user.0 \
            -serial stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04 -vga std

          rm -rf $ESP_DIR
        '';

      in {
        packages = {
          default = goldboot-uki-image;
          goldboot-uki = goldboot-uki;
          goldboot-uki-image = goldboot-uki-image;
        };

        # Development shell
        devShells.default = pkgs.mkShell {
          buildInputs = [
            # Testing/debugging tools
            pkgs.qemu
            pkgs.OVMF

            # Run scripts
            run-x86_64
            run-aarch64

          ];

          shellHook = ''
            echo "Goldboot UKI Development Environment"
            echo "Rust version: $(rustc --version)"
            echo ""
            echo "Available commands:"
            echo "  nix build .#goldboot-uki        - Build just the binary"
            echo "  nix build .#goldboot-uki-image  - Build the complete UKI"
            echo "  nix build                       - Build default (UKI image)"
            echo ""
            echo "Test with QEMU:"
            echo "  run-x86_64   - Run with QEMU x86_64"
            echo "  run-aarch64  - Run with QEMU aarch64"
          '';
        };
      });
}
