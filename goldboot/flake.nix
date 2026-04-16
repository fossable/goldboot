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
          buildInputs = builtins.filter (p:
            let name = p.pname or "";
            in !(pkgs.lib.hasPrefix "libx11" name)
            && !(pkgs.lib.hasPrefix "libxcb" name) && name != "xwayland")
            old.buildInputs;
          mesonFlags = [ "-Dxwayland=disabled" "-Dbackends=drm,libinput" ];
        });

        # cage without xwayland
        cage-no-xwayland = pkgs.cage.overrideAttrs (old: {
          buildInputs = builtins.map
            (p: if (p.pname or "") == "wlroots" then wlroots-no-xwayland else p)
            (builtins.filter (p:
              let name = p.pname or "";
              in !(pkgs.lib.hasPrefix "libx11" name)
              && !(pkgs.lib.hasPrefix "libxcb" name)) old.buildInputs);
          postFixup = "";
        });

        # Build the goldboot binary with UKI feature
        goldboot-bin = pkgs.rustPlatform.buildRustPackage {
          pname = "goldboot";
          version = "0.0.10";

          src = ../.;

          cargoLock = {
            lockFile = ../Cargo.lock;
            outputHashes = {
              "egui_term-0.1.0" =
                "sha256-M9/2tkNZUqrt7ca/l80xkE3BcPisy1SPXGnPaCoJCI4=";
            };
          };

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

          # Build goldboot UKI
          cargoBuildFlags =
            [ "-p" "goldboot" "--no-default-features" "--features" "uki" ];

          # Skip tests for UKI build
          doCheck = false;
        };

        # Minimal init script
        initScript = pkgs.writeScript "init" ''
          #!/bin/busybox sh

          set -e

          # Create busybox symlinks (but don't overwrite kmod's modprobe/insmod)
          /bin/busybox --install -s /bin

          # Set up library path for kmod
          export LD_LIBRARY_PATH=/lib/xz

          # Mount essential filesystems
          mount -t proc proc /proc
          mount -t sysfs sys /sys
          mount -t devtmpfs dev /dev

          # Mount /dev/shm for shared memory (required by wlroots for keymaps)
          mkdir -p /dev/shm
          mount -t tmpfs tmpfs /dev/shm

          # Mount /dev/pts for pseudo-terminals (required by debug shell)
          mkdir -p /dev/pts
          mount -t devpts devpts /dev/pts

          # Redirect all output to a log
          exec >/init.log 2>&1
          set -x

          # Create necessary directories
          mkdir -p /tmp /run /var /etc

          # Create minimal passwd for PTY allocation
          echo "root:x:0:0:root:/:/bin/sh" > /etc/passwd

          # Load all kernel modules upfront
          modprobe virtio_pci
          modprobe virtio_blk
          modprobe virtio_gpu
          modprobe virtio_input
          # Storage drivers
          modprobe ahci || true
          modprobe sd_mod || true
          modprobe nvme || true
          modprobe usb_storage || true
          # Filesystems
          modprobe nls_cp437
          modprobe nls_iso8859_1
          modprobe vfat
          modprobe ext4
          modprobe btrfs || true
          modprobe xfs || true
          modprobe ntfs3 || true
          modprobe iso9660 || true
          # USB host controllers
          modprobe xhci_pci
          modprobe ehci_pci
          # USB HID
          modprobe hid
          modprobe usbhid
          modprobe hid_generic
          # PS/2 keyboard (most laptop integrated keyboards)
          modprobe i8042 || true
          modprobe atkbd || true
          # I2C HID (newer laptops)
          modprobe i2c_hid || true
          modprobe i2c_hid_acpi || true
          # Input event interface
          modprobe evdev
          # Graphics
          modprobe drm
          modprobe drm_kms_helper
          # Networking
          modprobe r8169 || true

          # Set up udev directories
          mkdir -p /run/udev

          # Compile the udev hardware database (required for input device detection)
          udevadm hwdb --update

          # Start udev daemon
          udevd --daemon

          # Trigger udev to process devices and wait for them to settle
          udevadm trigger --action=add
          sleep 2
          udevadm settle --timeout=30

          # Mount goldboot images to /var/lib/goldboot/images
          mkdir -p /var/lib/goldboot/images
          ls /dev
          for dev in /dev/sd?* /dev/vd?* /dev/hd?* /dev/nvme*; do
            [ -e "$dev" ] || continue
            if mount -o ro "$dev" /var/lib/goldboot/images; then
              # Check for .gb files at root
              if ls /var/lib/goldboot/images/*.gb; then
                break
              fi
              umount /var/lib/goldboot/images
            fi
          done

          # Set up networking
          ip link set lo up
          # Find the first ethernet interface (not lo)
          for iface in /sys/class/net/*; do
            iface=$(basename "$iface")
            [ "$iface" = "lo" ] && continue
            ip link set "$iface" up
            udhcpc -i "$iface" -n -q || true
            break
          done

          export XDG_RUNTIME_DIR=/tmp/xdg-runtime
          mkdir -p "$XDG_RUNTIME_DIR"
          chmod 0700 "$XDG_RUNTIME_DIR"

          export WLR_BACKENDS=drm,libinput
          # Bypass udev enumeration — directly specify the DRM device
          export WLR_DRM_DEVICES=/dev/dri/card0
          # Use libseat embedded backend (root-capable, no daemon required)
          export LIBSEAT_BACKEND=builtin
          # Use pixman renderer for the compositor (no GL needed for cage itself)
          export WLR_RENDERER=pixman
          # virtio-gpu-pci has no hardware cursor planes
          export WLR_NO_HARDWARE_CURSORS=1
          # Allow starting without input devices (they may enumerate later)
          export WLR_LIBINPUT_NO_DEVICES=1

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

        # Combined udev rules with paths fixed for initramfs
        udevRules = pkgs.runCommand "udev-rules" { } ''
          mkdir -p $out/rules.d

          # Copy eudev rules
          cp ${pkgs.eudev}/var/lib/udev/rules.d/*.rules $out/rules.d/

          # Copy libinput rules but fix the hardcoded nix store paths
          for rule in ${pkgs.libinput.out}/lib/udev/rules.d/*.rules; do
            sed 's|${pkgs.libinput.out}/lib/udev/|/lib/udev/libinput/|g' "$rule" > "$out/rules.d/$(basename $rule)"
          done
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
            # USB HID
            "usbhid"
            "hid_generic"
            "ehci_pci"
            "xhci_pci"
            # PS/2 keyboard (most laptop integrated keyboards)
            "atkbd"
            "i8042"
            # I2C HID (newer laptops, ultrabooks)
            "i2c_hid"
            "i2c_hid_acpi"
            "hid"
            "r8169"
            "virtio_blk"
            "vfat"
            "ext4"
            "btrfs"
            "xfs"
            "ntfs3"
            "iso9660"
            "nls_cp437"
            "nls_iso8859_1"
            "evdev"
            # Real hardware storage drivers
            "ahci"
            "sd_mod"
            "nvme"
            "usb_storage"
          ];
        };

        # Create initramfs with makeInitrd
        buildInitramfs = kernel:
          pkgs.makeInitrd {
            name = "goldboot-initramfs";

            compressor = "${pkgs.zstd}/bin/zstd -19";

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
                object = "${pkgs.kmod}/bin/kmod";
                symlink = "/bin/kmod";
                mode = "0755";
              }
              {
                object = "${pkgs.kmod}/bin/kmod";
                symlink = "/bin/modprobe";
                mode = "0755";
              }
              {
                object = "${pkgs.kmod}/bin/kmod";
                symlink = "/bin/insmod";
                mode = "0755";
              }
              {
                object = "${pkgs.xz}/lib";
                symlink = "/lib/xz";
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

              # eudev for device property assignment (required by libinput)
              {
                object = "${pkgs.eudev}/bin/udevd";
                symlink = "/sbin/udevd";
              }
              {
                object = "${pkgs.eudev}/bin/udevadm";
                symlink = "/sbin/udevadm";
              }
              {
                object = "${pkgs.eudev}/var/lib/udev/hwdb.d";
                symlink = "/lib/udev/hwdb.d";
              }
              # Combined udev rules (eudev + libinput with fixed paths)
              # eudev looks for rules in /etc/udev/rules.d, NOT /lib/udev/rules.d
              {
                object = "${udevRules}/rules.d";
                symlink = "/etc/udev/rules.d";
              }
              # eudev helper binaries (ata_id, scsi_id, etc) - need to be at /lib/udev/
              {
                object = "${pkgs.eudev}/lib/udev/ata_id";
                symlink = "/lib/udev/ata_id";
              }
              {
                object = "${pkgs.eudev}/lib/udev/scsi_id";
                symlink = "/lib/udev/scsi_id";
              }
              {
                object = "${pkgs.eudev}/lib/udev/mtd_probe";
                symlink = "/lib/udev/mtd_probe";
              }

              # libinput library (for dynamic linking)
              {
                object = "${pkgs.libinput.out}/lib";
                symlink = "/lib/libinput";
              }
              # libinput udev helpers (device-group, fuzz-extract, etc)
              {
                object = "${pkgs.libinput.out}/lib/udev/libinput-device-group";
                symlink = "/lib/udev/libinput/libinput-device-group";
              }
              {
                object = "${pkgs.libinput.out}/lib/udev/libinput-fuzz-extract";
                symlink = "/lib/udev/libinput/libinput-fuzz-extract";
              }
              {
                object = "${pkgs.libinput.out}/lib/udev/libinput-fuzz-to-zero";
                symlink = "/lib/udev/libinput/libinput-fuzz-to-zero";
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
              --cmdline="quiet loglevel=0 rd.systemd.show_status=false rd.udev.log_level=0 vt.global_cursor_default=0" \
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

          # Create a FAT32 disk image from the images directory
          IMAGES_DIR=/var/lib/goldboot/images
          IMAGES_IMG=$(mktemp)
          IMAGES_SIZE=$(du -sb $IMAGES_DIR | cut -f1)
          IMAGES_SIZE=$((IMAGES_SIZE + 64*1024*1024))  # Add 64MB padding
          truncate -s $IMAGES_SIZE $IMAGES_IMG
          ${pkgs.dosfstools}/bin/mkfs.vfat -F 32 $IMAGES_IMG
          ${pkgs.mtools}/bin/mcopy -i $IMAGES_IMG -s $IMAGES_DIR/* ::

          qemu-system-x86_64 \
            -nodefaults --enable-kvm -m 8G -machine q35 -smp 4 \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_CODE.fd,readonly=on \
            -drive if=pflash,format=raw,file=${pkgs.OVMF.fd}/FV/OVMF_VARS.fd,readonly=on \
            -drive format=raw,file=fat:rw:$ESP_DIR \
            -drive format=raw,file=$IMAGES_IMG,id=images,if=none,readonly=on \
            -device virtio-blk-pci,drive=images \
            -netdev user,id=user.0 -device rtl8139,netdev=user.0 \
            -serial stdio \
            -device virtio-gpu-pci \
            -device usb-ehci \
            -device usb-kbd \
            -display gtk,gl=off

          rm -rf $ESP_DIR
          rm -f $IMAGES_IMG
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
