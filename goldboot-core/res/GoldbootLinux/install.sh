#!/bin/bash -x
## Perform a Goldboot Linux install.
set -e

# Synchronize time
timedatectl set-ntp true

# Create partitions
parted --script -a optimal -- /dev/vda \
	mklabel gpt \
	mkpart primary 1MiB 256MiB \
	set 1 esp on \
	mkpart primary 256MiB 100%

# Format boot partition
mkfs.vfat /dev/vda1

# Format root
mkfs.ext4 /dev/vda2

# Mount root
mount /dev/vda2 /mnt

# Mount boot partition
mount --mkdir /dev/vda1 /mnt/boot

# Print mounts (debugging)
mount

# Bootstrap filesystem
pacstrap /mnt base linux efibootmgr e2fsprogs grub dhcpcd xorg-server xorg-xinit gtk4 tpm2-tools

# Generate fstab
genfstab -U /mnt >/mnt/etc/fstab

cat <<-EOF >>/mnt/etc/default/grub
	GRUB_CMDLINE_LINUX_DEFAULT="root=UUID=$(blkid -s UUID -o value /dev/vda2) quiet loglevel=3 rd.systemd.show_status=auto rd.udev.log_level=3"
	GRUB_DEFAULT=0
	GRUB_TIMEOUT=0
	GRUB_TIMEOUT_STYLE=hidden
	GRUB_HIDDEN_TIMEOUT=0
	GRUB_HIDDEN_TIMEOUT_QUIET=true
	GRUB_DISABLE_OS_PROBER=true
	GRUB_RECORDFAIL_TIMEOUT=0
EOF

# Install bootloader
arch-chroot /mnt grub-install --target=x86_64-efi --efi-directory=/boot --bootloader-id=GRUB --removable
arch-chroot /mnt grub-mkconfig -o /boot/grub/grub.cfg

# Enable dhcpcd
systemctl enable dhcpcd.service --root /mnt

# Set root password
echo "root:${GB_ROOT_PASSWORD:?}" | chpasswd --root /mnt

# Install latest goldboot
latest=$(curl -L -s -H 'Accept: application/json' 'https://github.com/goldboot/goldboot/releases/latest' | sed 's/^.*"tag_name":"//;s/".*$//')
case "$(uname -m)" in
x86_64)
	curl -L -s "https://github.com/goldboot/goldboot/releases/download/${latest}/goldboot-gui-x86_64-unknown-linux-gnu" -o /mnt/usr/bin/goldboot-gui
	;;
aarch64)
	;;
esac
chmod +x /mnt/usr/bin/goldboot-gui

# Root autologin
#sed -i 's/^ExecStart.*$/ExecStart=-\/sbin\/agetty -a root %I $TERM/' /mnt/lib/systemd/system/getty@.service
mkdir -p /mnt/etc/systemd/system/getty@tty1.service.d
cat <<-EOF >/mnt/etc/systemd/system/getty@tty1.service.d/skip-prompt.conf
	[Service]
	ExecStart=
	ExecStart=-/usr/bin/agetty --skip-login --nonewline --noissue --autologin root --noclear %I \$TERM
EOF

touch /mnt/root/.hushlogin

# Autostart GUI
cat <<-EOF >/mnt/root/.profile
	exec startx &>/dev/null
EOF
cat <<-EOF >/mnt/root/.xinitrc
	exec goldboot-gui
EOF
