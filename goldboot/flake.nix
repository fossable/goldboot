{
  description = "Goldboot - Immutable infrastructure for bare metal";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # wlroots without xwayland or x11 backend
        wlroots-no-xwayland = pkgs.wlroots_0_19.overrideAttrs (old: {
          buildInputs = builtins.filter
            (p: let name = p.pname or ""; in
              !(pkgs.lib.hasPrefix "libx11" name)
              && !(pkgs.lib.hasPrefix "libxcb" name)
              && name != "xwayland")
            old.buildInputs;
          mesonFlags = [
            "-Dxwayland=disabled"
            "-Dbackends=drm,libinput"
          ];
        });

        # cage without xwayland
        cage-no-xwayland = pkgs.cage.overrideAttrs (old: {
          buildInputs = builtins.map
            (p: if (p.pname or "") == "wlroots" then wlroots-no-xwayland else p)
            (builtins.filter
              (p: let name = p.pname or ""; in
                !(pkgs.lib.hasPrefix "libx11" name)
                && !(pkgs.lib.hasPrefix "libxcb" name))
              old.buildInputs);
          postFixup = "";
        });

        # Build the goldboot binary with UKI feature
        goldboot-bin = pkgs.rustPlatform.buildRustPackage {
          pname = "goldboot";
          version = "0.0.10";

          src = ../.;

          cargoLock = { lockFile = ../Cargo.lock; };

          nativeBuildInputs = with pkgs; [ pkg-config ];

          buildInputs = with pkgs; [
            openssl
            openssl.dev
            udev # Required by libudev-sys (block-utils dependency)
            # Required by winit/egui Wayland backend
            wayland
            wayland-protocols
            libxkbcommon
            # Required by glutin/glow (OpenGL)
            libGL
          ];

          # Tell winit to use the Wayland backend
          WINIT_UNIX_BACKEND = "wayland";

          # Build goldboot with UKI feature
          cargoBuildFlags =
            [ "-p" "goldboot" "--no-default-features" "--features" "uki" ];

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

          # Redirect all output to serial console now that /dev exists
          exec >/dev/ttyS0 2>&1
          set -x

          # Create necessary directories
          mkdir -p /tmp /run /var

          # Set up networking (lo interface)
          ip link set lo up

          # Load GPU and input kernel modules
          modprobe virtio_pci
          modprobe virtio_gpu
          modprobe virtio_input
          modprobe xhci_pci
          modprobe ehci_pci
          modprobe usbhid
          modprobe hid_generic


          export XDG_RUNTIME_DIR=/tmp/xdg-runtime
          mkdir -p "$XDG_RUNTIME_DIR"
          chmod 0700 "$XDG_RUNTIME_DIR"

          export WLR_BACKENDS=drm
          export WLR_LIBINPUT_NO_DEVICES=1
          # Bypass udev enumeration — directly specify the DRM device
          export WLR_DRM_DEVICES=/dev/dri/card0
          # Use libseat embedded backend (root-capable, no daemon required)
          export LIBSEAT_BACKEND=builtin
          # Use pixman renderer for the compositor (no GL needed for cage itself)
          export WLR_RENDERER=pixman
          # virtio-gpu-pci has no hardware cursor planes
          export WLR_NO_HARDWARE_CURSORS=1

          exec /sbin/cage -- /sbin/goldboot-launch
        '';

        # Wrapper that sets goldboot's client-side env without affecting cage
        goldbootLaunch = pkgs.writeScript "goldboot-launch" ''
          #!/bin/busybox sh
          # winit/glutin dlopen these at runtime; provide paths for dynamic loader
          export LD_LIBRARY_PATH=/lib/wayland:/lib/xkbcommon:/lib/libgl:/run/opengl-driver/lib
          # Point EGL/GL to mesa drivers in the initramfs
          export LIBGL_DRIVERS_PATH=/run/opengl-driver/lib/dri
          export EGL_DRIVERS_PATH=/run/opengl-driver/lib
          # Use swrast (supports EGL_PLATFORM_WAYLAND); kms_swrast does not
          export LIBGL_ALWAYS_SOFTWARE=1
          export MESA_LOADER_DRIVER_OVERRIDE=swrast
          export EGL_PLATFORM=wayland
          exec /sbin/goldboot
        '';

        modulesClosure = pkgs.makeModulesClosure {
          kernel = pkgs.linuxPackages_latest.kernel.modules;
          firmware = pkgs.linux-firmware;
          allowMissing = false;
          rootModules = [
            "virtio_gpu"
            "virtio_pci"
            "virtio_input"
            "drm"
            "drm_kms_helper"
            "usbhid"
            "hid_generic"
            "ehci_pci"
            "xhci_pci"
          ];
        };

        # Create initramfs with makeInitrd
        buildInitramfs = kernel:
          pkgs.makeInitrd {
            name = "goldboot-initramfs";

            compressor = "gzip";

            contents = [
              {
                object = initScript;
                symlink = "/init";
                mode = "0755";
              }

              {
                object = "${goldboot-bin}/bin/goldboot";
                symlink = "/sbin/goldboot";
                mode = "0755";
              }

              {
                object = "${pkgs.busybox}/bin/busybox";
                symlink = "/bin/busybox";
                mode = "0755";
              }
              {
                object = "${cage-no-xwayland}/bin/cage";
                symlink = "/sbin/cage";
                mode = "0755";
              }
              {
                object = goldbootLaunch;
                symlink = "/sbin/goldboot-launch";
                mode = "0755";
              }
              {
                object = "${pkgs.iproute2}/bin/ip";
                symlink = "/bin/ip";
              }
              {
                object = pkgs.mesa;
                symlink = "/run/opengl-driver";
              }
              {
                object = "${modulesClosure}/lib/modules";
                symlink = "/lib/modules";
              }
              {
                object = "${pkgs.wayland}/lib";
                symlink = "/lib/wayland";
              }
              {
                object = "${pkgs.libxkbcommon}/lib";
                symlink = "/lib/xkbcommon";
              }
              {
                object = "${pkgs.libGL}/lib";
                symlink = "/lib/libgl";
              }
            ];
          };

        kernel = pkgs.linuxPackages_latest.kernel;

        initramfs = buildInitramfs kernel;

        # Build the UKI (Unified Kernel Image)
        goldboot-uki = pkgs.stdenv.mkDerivation {
          name = "goldboot-uki";

          nativeBuildInputs = [
            pkgs.systemdUkify # provides ukify
            pkgs.binutils
          ];

          buildCommand = ''
            mkdir -p $out

            # List initramfs contents for debugging (initrd may be a concatenated cpio)
            echo "Initramfs contents:"
            ${pkgs.cpio}/bin/cpio -itv --quiet < ${initramfs}/initrd 2>/dev/null || true

            # Use ukify to create the UKI
            ${pkgs.systemdUkify}/bin/ukify build \
              --linux=${kernel}/bzImage \
              --initrd=${initramfs}/initrd \
              --os-release='NAME="Goldboot"
            ID=goldboot
            VERSION="0.1.0"' \
              --cmdline="console=ttyS0 console=tty0" \
              --output=$out/goldboot.efi

            echo "UKI created at $out/goldboot.efi"
          '';
        };

        # Create bootable ISO image with the UKI
        goldboot-iso = pkgs.stdenv.mkDerivation {
          name = "goldboot-iso";

          nativeBuildInputs = [ pkgs.xorriso pkgs.dosfstools pkgs.mtools ];

          buildCommand = ''
            mkdir -p iso/EFI/BOOT

            # Copy UKI to the ESP (EFI System Partition) location
            ${if system == "x86_64-linux" then ''
              cp ${goldboot-uki}/sandpolis.efi iso/EFI/BOOT/BOOTX64.EFI
            '' else ''
              cp ${goldboot-uki}/sandpolis.efi iso/EFI/BOOT/BOOTAA64.EFI
            ''}

            # Create the ISO image
            ${pkgs.xorriso}/bin/xorriso \
              -as mkisofs \
              -o $out/sandpolis.iso \
              -isohybrid-mbr ${pkgs.syslinux}/share/syslinux/isohdpfx.bin \
              -c boot.cat \
              -b EFI/BOOT/${
                if system == "x86_64-linux" then
                  "BOOTX64.EFI"
                else
                  "BOOTAA64.EFI"
              } \
              -no-emul-boot \
              -boot-load-size 4 \
              -boot-info-table \
              --efi-boot EFI/BOOT/${
                if system == "x86_64-linux" then
                  "BOOTX64.EFI"
                else
                  "BOOTAA64.EFI"
              } \
              -efi-boot-part \
              --efi-boot-image \
              --protective-msdos-label \
              iso

            echo "ISO created at $out/goldboot.iso"
          '';
        };

        # Run scripts for QEMU testing
        run-x86_64 = pkgs.writeShellScriptBin "run-x86_64" ''
          # Set up ESP directory structure in temp directory
          ESP_DIR=$(mktemp -d)
          mkdir -p $ESP_DIR/EFI/Boot
          cp result/goldboot.efi $ESP_DIR/EFI/Boot/BootX64.efi

          qemu-system-x86_64 \
            -nodefaults --enable-kvm -m 8G -machine q35 -smp 4 \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd,readonly=on \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_VARS.fd,readonly=on \
            -drive format=raw,file=fat:rw:$ESP_DIR \
            -netdev user,id=user.0 -device rtl8139,netdev=user.0 \
            -serial stdio \
            -device virtio-gpu-pci \
            -device virtio-keyboard-pci \
            -device virtio-mouse-pci \
            -display gtk,gl=off

          rm -rf $ESP_DIR
        '';

        run-aarch64 = pkgs.writeShellScriptBin "run-aarch64" ''
          # Set up ESP directory structure in temp directory
          ESP_DIR=$(mktemp -d)
          mkdir -p $ESP_DIR/EFI/Boot
          cp result/goldboot.efi $ESP_DIR/EFI/Boot/BootAA64.efi

          qemu-system-aarch64 \
            -nodefaults --enable-kvm -m 8G -machine virt -cpu cortex-a72 -smp 4 \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd,readonly=on \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_VARS.fd,readonly=on \
            -drive format=raw,file=fat:rw:$ESP_DIR \
            -netdev user,id=user.0 -device rtl8139,netdev=user.0 \
            -serial stdio -device isa-debug-exit,iobase=0xf4,iosize=0x04 -vga std

          rm -rf $ESP_DIR
        '';

      in {
        packages = {
          default = goldboot-uki;
          goldboot = goldboot-bin;
          goldboot-initramfs = initramfs;
          goldboot-uki = goldboot-uki;
          goldboot-iso = goldboot-iso;
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
            echo "  nix build .#goldboot            - Build just the binary"
            echo "  nix build .#goldboot-uki        - Build the complete UKI"
            echo "  nix build                       - Build default (UKI image)"
            echo ""
            echo "Test with QEMU:"
            echo "  run-x86_64   - Run with QEMU x86_64"
            echo "  run-aarch64  - Run with QEMU aarch64"
          '';
        };
      });
}
