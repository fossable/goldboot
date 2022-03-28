<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/logo-bg-256.png" />
</p>

![Build](https://github.com/goldboot/goldboot/workflows/.github/workflows/build.yml/badge.svg)

`goldboot` simplifies the process of building and deploying golden images to
bare-metal.

**Warning: this tool is totally unfinshed and should be used for testing only! Proceed
at your own risk!**

## Golden Images

Golden images contain your operating system, applications, software patches, and
configuration all rolled into one easily deployable package.

Keeping a large number of servers consistent might be the most obvious benefit to
golden images, but `goldboot` can also build images for desktop workstations too.
Imagine booting a brand-new install of your favorite OS with all applications and
custom configuration already present!

### Reset Security

_Reset security_ is the concept that periodically rolling back a machine's state to a
"known clean" checkpoint is a beneficial security practice. Most malware (excepting
firmware-level infections) cannot survive the reimaging process and therefore can
be removed easily.

So the benefit is, if an attacker is able to infiltrate a machine, at least they
can't stick around for long and removing the compromise is very easy to do. Of
course, an attacker might not need much time, so _reset security_ isn't a replacement
for normal security (if you're here looking to replace normal security, then I
have some bitter news for you).

### Configuration Drift

Another nice trait of golden images is they provide a single source of truth for
your machine's configuration. Over time, it's very common for config files to
change, new applications are installed, junk files accumulate here and there, etc.

Like a ship's hull, computers (metaphorically) gather barnacles over time. There
are at least three solutions to this:

- Mount your root filesystem as read-only
    - Of course making changes is now unwieldly, but the configuration can never drift
    - Not possible on many operating systems
- Sync your machine's configuration after it has drifted
    - There are many excellent tools out there for this like: Ansible, Puppet, Chef, etc
    - The shortcoming of these tools is that they have limited scope
- Rollback your machine's configuration to a clean state after a while
    - Building that "clean state" and deploying it is what `goldboot` does

All three of these solutions are compatible with `goldboot` (although the first may
not make sense since there's nothing to change on rollback).

The trick to a successful rollback is not to lose any important data. That means:

- Any config changes that should be persistent must be made in the `goldboot` image
- Any important data needs to be stored remotely (NFS, SMB, etc...) so it doesn't get wiped out

### Downtime

A disadvantage to golden images is that applying them involves downtime which is
proportional to the size of the image and how far a particular machine's state
has drifted from the golden image.

For this reason, the size of golden images should be kept to a minimum. They are
therefore not ideal for storing large databases, archives, logs, etc.

## Platform Support Matrix

The following table shows planned support (nothing here is fully complete yet).

| OS Name    | Testing         | Provisioners | Multiboot |
|------------|-----------------|--------------|-----------|
| ![Alpine](/.github/images/platforms/alpine.png) Alpine Linux | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_alpine_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_alpine_aarch64.yml/badge.svg) | Yes | Yes |
| ![Arch Linux](/.github/images/platforms/arch.png) Arch Linux | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_arch_linux_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_arch_linux_aarch64.yml/badge.svg) | Yes | Yes |
| ![Debian](/.github/images/platforms/debian.png) Debian | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_debian_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_debian_aarch64.yml/badge.svg) | Yes | Yes |
| ![macOS](/.github/images/platforms/mac_os.png) macOS | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_mac_os_x86_64.yml/badge.svg) | Yes | No |
| ![Pop!_OS](/.github/images/platforms/pop_os.png) Pop!\_OS | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_pop_os_x86_64.yml/badge.svg) | Yes | Yes |
| ![Windows 10](/.github/images/platforms/windows_10.png) Windows 10 | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_windows_10_x86_64.yml/badge.svg) | Yes | No |
| ![Steam Deck](/.github/images/platforms/steam_deck.png) Steam Deck | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_steam_deck_x86_64.yml/badge.svg) | No | Yes |
| ![Steam OS](/.github/images/platforms/steam_os.png) Steam OS | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_steam_os_x86_64.yml/badge.svg) | Yes | Yes |

## Getting Started

Let's build a Windows 10 image since Windows known for its long install process.

First, create a directory which can later be added to version control:
```sh
mkdir WindowsMachine
cd WindowsMachine
```

Initialize the directory and choose a base profile to start with:
```sh
goldboot init --profile Windows10
```

This will create `goldboot.json` which contains configuration options that will
need to be tweaked. For example, you'll need to supply your own install media for
a Windows install (thanks Microsoft):

```json
"iso_url": "Win10_1803_English_x64.iso",
"iso_checksum": "sha1:08fbb24627fa768f869c09f44c5d6c1e53a57a6f"
```

Next, create some scripts to provision the install:

```sh
# Example provisioner script
echo 'Set-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\Power\ -name HibernateEnabled -value 0' >disable_hibernate.ps1
```

And add it to the goldboot config in the order they should be executed:
```json
"provisioners": [
	{
		"type": "shell",
		"scripts": ["disable_hibernate.ps1"]
	}
]
```

Now, build the image:
```sh
goldboot build
```

And finally the last step is to deploy it to a physical disk. There are two alternative
ways to do this.

#### Option 1: Apply the image to an existing disk

```sh
# THIS WILL OVERWRITE /dev/sdX! TAKE A BACKUP FIRST!
goldboot image write WindowsMachine /dev/sdX
```

#### Option 2: Create a bootable USB containing the image

```sh
# THIS WILL OVERWRITE /dev/sdX! TAKE A BACKUP FIRST!
goldboot make_usb --disk /dev/sdX --include WindowsMachine
```

Once the USB is created, you can use it to boot into the goldboot live environment
and select an image to write.