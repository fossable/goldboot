#!/bin/bash
## Perform a basic Arch Linux install.
set -e -u

# Synchronize time
timedatectl set-ntp true

# Configure Pacman mirrors
cat <<-EOF >/etc/pacman.d/mirrorlist
	Server = https://dfw.mirror.rackspace.com/archlinux/\$repo/os/\$arch
	Server = https://mirrors.kernel.org/archlinux/\$repo/os/\$arch
EOF

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
pacstrap /mnt base linux linux-firmware efibootmgr grub dhcpcd openssh python python-pip sudo

# Generate fstab
genfstab -U /mnt >/mnt/etc/fstab

# Install grub
cat <<-EOF >>/mnt/etc/default/grub
	GRUB_CMDLINE_LINUX="root=/dev/nvme0n1p2"
EOF
arch-chroot /mnt grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB
arch-chroot /mnt grub-mkconfig -o /boot/grub/grub.cfg

# Configure timezone
arch-chroot /mnt ln -sf /usr/share/zoneinfo/America/Chicago /etc/localtime

# Configure hardware clock
arch-chroot /mnt hwclock --systohc

# Generate locale
cat <<-EOF >/mnt/etc/locale.gen
	en_US.UTF-8 UTF-8
EOF
cat <<-EOF >/mnt/etc/locale.conf
	LANG=en_US.UTF-8
EOF
arch-chroot /mnt locale-gen

# Initial ssh configuration
cat <<-EOF >>/mnt/etc/ssh/sshd_config
	PasswordAuthentication no
EOF

# Enable sshd
arch-chroot /mnt systemctl enable sshd.service

# Enable dhcpcd
arch-chroot /mnt systemctl enable dhcpcd.service
