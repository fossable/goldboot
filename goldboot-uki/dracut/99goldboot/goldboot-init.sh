#!/bin/sh

# Goldboot UKI initialization script
# This runs early in the initramfs boot process

# Set up minimal environment
export PATH=/usr/bin:/usr/sbin:/bin:/sbin
export HOME=/root
export TERM=linux

# Ensure DRM/KMS devices are available
udevadm trigger --action=add --subsystem-match=drm
udevadm settle

# Wait for block devices to be ready
udevadm trigger --action=add --subsystem-match=block
udevadm settle

# Create necessary directories
mkdir -p /run /tmp /var/tmp /var/lib/goldboot/images

# Set up framebuffer console
if [ -e /dev/fb0 ]; then
    echo "Framebuffer detected at /dev/fb0"
fi

# The actual goldboot-uki application will be started by the systemd service
