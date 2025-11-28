#!/bin/bash

# called by dracut
check() {
    # Only include this module if explicitly requested
    return 255
}

# called by dracut
depends() {
    # Require DRM/KMS for framebuffer, systemd for init
    echo "drm systemd"
    return 0
}

# called by dracut
install() {
    # Install the goldboot-uki binary and let dracut resolve its dependencies automatically
    inst_binary "$goldboot_uki_path" "/usr/bin/goldboot-uki"

    # Dracut's inst_binary will automatically handle shared library dependencies via ldd
    # This works correctly in both Debian and Nix environments

    # Install block device utilities
    inst_multiple \
        lsblk \
        blkid \
        blockdev

    # Install our custom init script
    inst_hook pre-mount 99 "$moddir/goldboot-init.sh"

    # Create systemd service to run goldboot-uki
    $SYSTEMCTL -q --root "$initdir" add-wants initrd.target goldboot-uki.service

    inst_simple "$moddir/goldboot-uki.service" "$systemdsystemunitdir/goldboot-uki.service"
}

# called by dracut
installkernel() {
    # Install DRM/KMS kernel modules for GPU support
    instmods =drivers/gpu/drm
    instmods =drivers/video
}
