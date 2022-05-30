<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/logo-bg-256.png" />
</p>

![Build](https://github.com/goldboot/goldboot/workflows/.github/workflows/build.yml/badge.svg)

`goldboot` simplifies the process of building and deploying golden images to
bare-metal.

**Warning: this tool is totally unfinshed and should be used for testing only! Proceed
at your own risk!**

<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/build.gif" />
</p>

## Golden Images

Golden images contain your operating system(s), applications, software patches, and
configuration all rolled into one easily deployable package.

Keeping the configuration of a large number of servers consistent might be the
most obvious benefit to golden images, but they are useful for workstations
too. With golden images, you can boot a brand-new install of your favorite OS with all
applications and custom configuration already present!

### Reset Security

_Reset security_ is the concept that periodically rolling a machine's state back to a
"known clean" checkpoint is a beneficial security practice. Malware (excluding
firmware-level infections) cannot survive the rollback process and therefore can
be removed easily.

But how do you know the image you're applying is "known clean"? Supply chain attacks,
although difficult to pull off, can compromise golden images themseleves. The best
chance at catching this type of attack is automated testing/auditing which is still
an active area of development.

### Configuration Drift

Golden images provide a single source of truth for a machine's configuration. Over
time, it's common for configurations to change, new applications are installed,
junk files accumulate here and there, etc.

Like a ship's hull, computers (metaphorically) gather barnacles over time. There
are at least three solutions to this:

- Mount your filesystems as read-only
    - Making changes is now unwieldly, but the configuration can never drift
    - Not possible on many operating systems
- Sync your machine's configuration periodically after it has drifted
    - There are many excellent tools out there for this like: Ansible, Puppet, Chef, etc
    - The shortcoming of these tools is that they have limited scope
- Rollback your machine's configuration to a clean state periodically
    - Building that "clean state" and performing the rollback is what `goldboot` does

### Application Data

_Application data_ is everything that needs to survive a rollback. This data must
be synced/stored remotely with something like NFS, SMB, SSHFS, etc or it can reside
locally on another drive. The golden image can contain the configuration necessary
to mount the application data so it's immediately available.

### Downtime

A disadvantage to golden images is that applying them necessarily involves downtime.

The time it takes to apply an image is proportional to the total size of the image
and how far the machine's state has drifted. For this reason, the size of golden
images should be kept to a minimum. They are not ideal for storing _application
data_ like databases, archives, logs, backups, etc.

## Platform Support Matrix

The following table shows planned support (nothing here is fully complete yet).

| OS Name    | Testing         | Provisioners | Multiboot |
|------------|-----------------|--------------|-----------|
| ![Alpine](/.github/images/templates/AlpineLinux.png) Alpine Linux | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_alpine_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_alpine_aarch64.yml/badge.svg) | Yes | Yes |
| ![Arch Linux](/.github/images/templates/ArchLinux.png) Arch Linux | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_arch_linux_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_arch_linux_aarch64.yml/badge.svg) | Yes | Yes |
| ![Debian](/.github/images/templates/Debian.png) Debian | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_debian_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_debian_aarch64.yml/badge.svg) | Yes | Yes |
| ![macOS](/.github/images/templates/MacOs.png) macOS | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_mac_os_x86_64.yml/badge.svg) | Yes | No |
| ![Pop!_OS](/.github/images/templates/pop_os.png) Pop!\_OS | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_pop_os_x86_64.yml/badge.svg) | Yes | Yes |
| ![Steam Deck](/.github/images/templates/steam_deck.png) Steam Deck | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_steam_deck_x86_64.yml/badge.svg) | No | Yes |
| ![Steam OS](/.github/images/templates/steam_os.png) Steam OS | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_steam_os_x86_64.yml/badge.svg) | Yes | Yes |
| ![Windows 10](/.github/images/templates/Windows10.png) Windows 10 | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_windows_10_x86_64.yml/badge.svg) | Yes | No |

## Getting Started

Let's build a basic Arch Linux image for simplicity.

First, create a directory which can later be added to version control:
```sh
mkdir Test
cd Test
```

Initialize the directory and choose the `ArchLinux` base template to start with:
```sh
goldboot init --name Test --template ArchLinux
```

This will create `goldboot.json` which contains configuration options that can
be tweaked to suit your needs.

For example, we can create scripts to customize the image:

```sh
# Example provisioner script
echo 'pacman -Syu firefox' >configure.sh
```

And add it to the goldboot config:
```json
"provisioners": [
	{
		"type": "shell",
		"script": "configure.sh"
	}
]
```

Now, build the image:
```sh
goldboot build
```

Once the build succeeds, the image will be saved to the system's library directory. To deploy
it to a physical disk, you can use a bootable USB drive:

```sh
# THIS WILL OVERWRITE /dev/sdX!
goldboot make_usb --output /dev/sdX --include Test
```

Once the USB is created, you can use it to boot into the goldboot live environment
and select an image to write:

<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/select_image.png" />
</p>

Once the image has been applied, remove the bootable USB drive and reboot the machine.
