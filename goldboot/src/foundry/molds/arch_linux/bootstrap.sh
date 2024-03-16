#!/bin/bash

set -e
set -x

exec 1>&2

# Don't block forever if we don't have enough entropy
ln -sf /dev/urandom /dev/random

# Synchronize time
timedatectl set-ntp true

# Wait for time sync
while [ $(timedatectl show --property=NTPSynchronized --value) != "yes" ]; do
	sleep 5
done

# Wait for pacman-init to complete
while ! systemctl show pacman-init.service | grep SubState=exited; do
    sleep 5
done

# Use a wrapper to run the install
pacman -Sy --noconfirm --noprogressbar archinstall curl
archinstall --debug --config <(curl "http://${GB_HTTP_HOST:?}:${GB_HTTP_PORT:?}/config.json") --creds <(curl "http://${GB_HTTP_HOST:?}:${GB_HTTP_PORT:?}/creds.json")

exit

# Create partitions
parted --script -a optimal -- /dev/vda \
	mklabel gpt \
	mkpart primary 1MiB 256MiB \
	set 1 esp on \
	mkpart primary 256MiB 100%

# Format boot partition
mkfs.vfat /dev/vda1

# Bootstrap filesystem
pacstrap -K -M /mnt base linux linux-firmware efibootmgr grub dhcpcd ${GB_PACKAGES}

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

