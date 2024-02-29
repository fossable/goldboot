#!/bin/bash -e

exec 1>&2

# Synchronize time
timedatectl set-ntp true

# Display current time. If this is wrong, pacman keys might fail to import
date

# Configure Pacman mirrors
if [ ${#GB_MIRRORLIST} -gt 0 ]; then
	echo "${GB_MIRRORLIST}" >/etc/pacman.d/mirrorlist
fi

# Create partitions
parted --script -a optimal -- /dev/vda \
	mklabel gpt \
	mkpart primary 1MiB 256MiB \
	set 1 esp on \
	mkpart primary 256MiB 100%

# Format boot partition
mkfs.vfat /dev/vda1

if [ "${GB_LUKS_PASSPHRASE}" != "" ]; then

	# TODO configure parameters
	echo -n "${GB_LUKS_PASSPHRASE}" | cryptsetup -v luksFormat /dev/vda2 -
	echo -n "${GB_LUKS_PASSPHRASE}" | cryptsetup open /dev/vda2 root -
	history -cw

	# Format root
	mkfs.ext4 /dev/mapper/root

	# Mount root
	mount /dev/mapper/root /mnt
else
	# Format root
	mkfs.ext4 /dev/vda2

	# Mount root
	mount /dev/vda2 /mnt
fi

# Mount boot partition
mount --mkdir /dev/vda1 /mnt/boot

# Display mounts before install
mount

# Bootstrap filesystem
pacstrap /mnt base linux linux-firmware efibootmgr grub dhcpcd ${GB_PACKAGES}

# Generate fstab
genfstab -U /mnt >/mnt/etc/fstab

if [ -e /dev/mapper/root ]; then
	cat <<-EOF >>/mnt/etc/default/grub
		GRUB_CMDLINE_LINUX="cryptdevice=UUID=$(blkid -s UUID -o value /dev/vda2):root root=/dev/mapper/root"
	EOF

	# Update initramfs
	echo 'HOOKS=(base udev autodetect keyboard keymap consolefont modconf block encrypt filesystems fsck)' >/mnt/etc/mkinitcpio.conf
	arch-chroot /mnt mkinitcpio -P
else
	cat <<-EOF >>/mnt/etc/default/grub
		GRUB_CMDLINE_LINUX="root=UUID=$(blkid -s UUID -o value /dev/vda2)"
	EOF
fi

# Install bootloader
arch-chroot /mnt grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB
arch-chroot /mnt grub-mkconfig -o /boot/grub/grub.cfg

# Enable dhcpcd
systemctl enable dhcpcd.service --root /mnt

# Set root password
echo "root:${GB_ROOT_PASSWORD:?}" | chpasswd --root /mnt

# Complete
# reboot
