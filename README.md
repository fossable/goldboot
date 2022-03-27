<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/logo-bg-256.png" />
</p>

[![Build](https://github.com/goldboot/goldboot/workflows/.github/workflows/test.yml/badge.svg)](https://github.com/goldboot/goldboot/actions?query=workflow%3A.github%2Fworkflows%2Ftest.yml)

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

Reset security is the concept that periodically rolling back a machine's state to a
"known clean" checkpoint is a beneficial security practice. Most malware (excepting
firmware-level infections) cannot survive the reimaging process.

So the idea is, if an attacker is able to infiltrate a machine, at least they can't stick around for long.

### Downtime

A disadvantage to golden images is that applying them involves downtime which is
proportional to the size of the image and how far a particular machine's state
has drifted from the golden image.

For this reason, the size of golden images should be kept to a minimum. They are
therefore not ideal for storing large databases, archives, logs, etc.

## Operating System Compatibility Matrix

| OS Name    | Architectures   | Provisioners | Multiboot |
|------------|-----------------|--------------|-----------|
| ![Arch Linux](/.github/images/platforms/arch_linux.png) Arch Linux | x86_64, aarch64 | Yes | Yes |

## Getting Started

Let's build a Windows 10 image since Windows known for its laborious install process.

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