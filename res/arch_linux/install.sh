#!/bin/bash
## Perform a basic Arch Linux install.
set -e -u

# Synchronize time
timedatectl set-ntp true

# Configure Pacman mirrors
echo "${GB_MIRRORLIST:?}" >/etc/pacman.d/mirrorlist

# Create partitions
parted --script -a optimal -- /dev/vda \
	mklabel gpt \
	mkpart primary 1MiB 256MiB \
	mkpart primary 256MiB 100%

mkfs.vfat /dev/vda1
mkfs.ext4 /dev/vda2

# Mount partitions
mount /dev/vda2 /mnt
mkdir -p /mnt/boot && mount /dev/vda1 /mnt/boot

# Bootstrap filesystem
pacstrap /mnt base linux linux-firmware efibootmgr grub dhcpcd openssh python python-pip

# Generate fstab
genfstab -U /mnt >/mnt/etc/fstab

# Install grub
cat <<-EOF >>/mnt/etc/default/grub
	GRUB_CMDLINE_LINUX="root=/dev/vda2"
EOF
arch-chroot /mnt grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB
arch-chroot /mnt grub-mkconfig -o /boot/grub/grub.cfg

# Enable sshd
systemctl enable sshd.service --root /mnt

# Enable dhcpcd
systemctl enable dhcpcd.service --root /mnt

# Set root password
echo "root:${GB_ROOT_PASSWORD:?}" | chpasswd --root /mnt

# Allow root login for subsequent provisioning
cat <<-EOF >>/mnt/etc/ssh/sshd_config
	PermitRootLogin yes
EOF

# Complete
reboot
