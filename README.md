<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/logo-bg-256.png" />
</p>
<hr>

Normal people don't reinstall their OS from scratch very often. When they do,
the moment they reach that pristine desktop or terminal after a clean
installation, all hell breaks loose. Settings get changed, applications are
installed, bloatware is removed, files get downloaded here and there. The system
is generally altered from its original state into a new "customized" state by a
manual flurry of mouse clicks and key presses.

If you think about your system like a server, this approach is called _mutable
infrastructure_, meaning you mutate the state of your system repeatedly until it
eventually suits your needs. And when something goes awry, you have to make the
necessary changes to get it back in line.

For normal people, mutable infrastructure works out fine until something major
breaks or they have to migrate to a new computer altogether. In these cases,
they probably end up starting over from scratch and have to reapply their
changes again (and probably differently this time).

Slightly less normal people might have scripts or even use a configuration
management tool like Ansible or Puppet to automate all of those customizations.
This is great, but you can't start at a boot prompt and immediately run an
Ansible playbook. Something (or someone) has to install the OS before the
automation can be "kicked off". Also, configuration management tools have
limited scope.

Truly sophisticated computer elites practice _immutable infrastructure_. Meaning
that, every time they boot their system, its state begins identically to the
time before. Any changes that are made during the course of runtime vanish on
reboot. This approach has some real benefits, but requires quite a bit of effort
from the user.

If you're looking to achieve something close to immutable infrastructure without
creating a lot of extra work for yourself, you've come to the right place.

In the `goldboot` approach, you choose a starting template containing an
absolutely minimal install of your favorite OS. Then you create _provisioners_
which are the scripts that add all of your customizations on top of the
template. From these pieces, `goldboot` builds a machine image ready to be
deployed to real hardware.

**Warning: this tool is totally unfinshed and should be used for testing only!
Proceed at your own risk!**

![License](https://img.shields.io/github/license/goldboot/goldboot)
![build](https://github.com/goldboot/goldboot/actions/workflows/build.yml/badge.svg)
[![Discord](https://img.shields.io/discord/981695209492606986)](https://discord.gg/Vzr7gT5dsd)
![Lines of code](https://img.shields.io/tokei/lines/github/fossable/goldboot)
![Stars](https://img.shields.io/github/stars/goldboot/goldboot?style=social)

## `goldboot`

`goldboot` is a command-line utility similar in nature to
[Packer](https://github.com/hashicorp/packer) that builds machine images for
both servers and workstations alike.

These machine images (also known as _golden images_) contain your operating
system(s), applications, software patches, and configuration all rolled into one
easily deployable package.

The CLI is designed to run locally or in the cloud from a CI pipeline.

## `goldboot-linux`

The golden images that `goldboot` produces can be deployed through a bootable
Linux USB stick with a
[graphical user interface](https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/select_image.png).

The `goldboot` command can create a bootable USB stick and include images on it.

## `goldboot-registry`

There's also an optional HTTP server that hosts goldboot images over the
network. `goldboot-linux` is capable of downloading images from a registry and
applying it to the local machine.

## Platform Support Matrix

The following table shows planned support (nothing here is fully complete yet).

| OS Name                                                            | Testing                                                                                                                                                                                                                             | Provisioners | Multiboot |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | --------- |
| ![Alpine](/.github/images/templates/AlpineLinux.png) Alpine Linux  | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_alpine_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_alpine_aarch64.yml/badge.svg)         | Yes          | Yes       |
| ![Arch Linux](/.github/images/templates/ArchLinux.png) Arch Linux  | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_arch_linux_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_arch_linux_aarch64.yml/badge.svg) | Yes          | Yes       |
| ![Debian](/.github/images/templates/Debian.png) Debian             | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_debian_x86_64.yml/badge.svg) ![aarch64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_debian_aarch64.yml/badge.svg)         | Yes          | Yes       |
| ![macOS](/.github/images/templates/MacOs.png) macOS                | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_mac_os_x86_64.yml/badge.svg)                                                                                                                        | Yes          | No        |
| ![Pop!_OS](/.github/images/templates/pop_os.png) Pop!\_OS          | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_pop_os_x86_64.yml/badge.svg)                                                                                                                        | Yes          | Yes       |
| ![Steam Deck](/.github/images/templates/steam_deck.png) Steam Deck | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_steam_deck_x86_64.yml/badge.svg)                                                                                                                    | No           | Yes       |
| ![Steam OS](/.github/images/templates/steam_os.png) Steam OS       | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_steam_os_x86_64.yml/badge.svg)                                                                                                                      | Yes          | Yes       |
| ![Windows 10](/.github/images/templates/Windows10.png) Windows 10  | ![x86_64](https://github.com/goldboot/goldboot/workflows/.github/workflows/test_windows_10_x86_64.yml/badge.svg)                                                                                                                    | Yes          | No        |

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

Once the build succeeds, the image will be saved to the system's library
directory. To deploy it to a physical disk, you can use a bootable USB drive:

```sh
# THIS WILL OVERWRITE /dev/sdX!
goldboot make_usb --output /dev/sdX --include Test
```

Once the USB is created, you can use it to boot into the goldboot live
environment and select an image to write:

<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/select_image.png" />
</p>

Once the image has been applied, remove the bootable USB drive and reboot the
machine.
