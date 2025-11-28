## goldboot-uki

This crate is loaded into a Unified Kernel Image (UKI) used to apply `goldboot` images.

### What is a UKI?

A Unified Kernel Image (UKI) is a single EFI executable that contains:
- Linux kernel
- initramfs (with goldboot-uki application and all dependencies)
- Kernel command line
- OS metadata

This allows the entire goldboot deployment environment to be a single bootable `.efi` file.

### Architecture

```
UEFI Firmware → goldboot.efi → goldboot-uki GUI → reboot
```

The UKI boots directly into the goldboot-uki application running from initramfs, with:
- GTK4 graphical interface
- Direct framebuffer rendering (DRM/KMS)
- Block device detection and management
- Automatic system reboot on exit

### Building

The UKI is built using Nix for reproducibility:

```bash
# Build just the goldboot-uki binary
nix build .#goldboot-uki

# Build the complete UKI image
nix build .#goldboot-uki-image

# The resulting UKI will be at: result/goldboot.efi
```

### Development

Enter the development shell:

```bash
nix develop

# Then you can use standard Rust tools:
cargo build -p goldboot-uki
cargo test -p goldboot-uki
```

### Deployment

The resulting `goldboot.efi` can be:
1. Copied to a USB drive's EFI System Partition (ESP)
2. Booted directly via UEFI firmware
3. Added to a bootloader menu
4. Network booted via iPXE/UEFI HTTP boot

Example ESP layout:
```
/EFI/
  └── BOOT/
      └── BOOTX64.EFI  (goldboot.efi renamed)
```

### Components

- **goldboot-uki binary**: Rust GTK4 application (this crate)
- **dracut module** (`goldboot/src/builder/os/goldboot/dracut/99goldboot/`): Bundles the application into initramfs
- **Nix derivation** (`flake.nix`): Orchestrates the UKI build

### Size

The UKI is approximately 300-500MB:
- Kernel: ~70MB
- initramfs (with GTK4 + goldboot-uki): ~200-400MB
- Overhead: ~50MB
