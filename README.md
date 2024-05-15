<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/logo-bg-256.png" />
</p>

![License](https://img.shields.io/github/license/goldboot/goldboot)
![build](https://github.com/goldboot/goldboot/actions/workflows/build.yml/badge.svg)
[![Discord](https://img.shields.io/discord/981695209492606986)](https://discord.gg/Vzr7gT5dsd)
![GitHub repo size](https://img.shields.io/github/repo-size/fossable/goldboot)
![Stars](https://img.shields.io/github/stars/goldboot/goldboot?style=social)
<hr>

People usually don't reinstall their OS from scratch very often. When they do,
chaos ensues the moment they reach that pristine desktop or terminal. Settings
get changed, applications are installed, bloatware is removed, files are downloaded
here and there. The system is generally altered from its original state into a new
"customized" state by a manual flurry of mouse clicks and key presses.

This standard approach is like _mutable infrastructure_, meaning you mutate the
state of your system repeatedly until it eventually suits your needs. And when
something goes awry, you have to make the necessary changes to get it back in line.

For most people, mutable infrastructure works out fine until something major
breaks or they have to migrate to a new computer altogether. In these cases,
they probably end up starting over from scratch and reapply their changes again
(and probably slightly differently this time).

Sophisticated computer elites probably practice _immutable infrastructure_. Meaning
that, every time they boot their system, its state begins almost identically to
the time before. Any changes that are made during the course of runtime vanish on
reboot. This approach has some real benefits, but requires quite a bit of effort
from the user.

`goldboot` is a tool that builds machine images for real hardware that can help
you achieve something close to immutable infrastructure without creating a lot
of extra work for yourself.

In the `goldboot` approach, you create a declarative configuration file for each
machine that you want to deploy. Using this configuration, `goldboot` builds an
image either on your local machine or on a CI platform like Github Actions. The
resulting image can be deployed to real hardware via a USB drive or through PXE boot.

**Warning: this tool is totally unfinshed and should be used for testing only!
Proceed at your own risk!**
<hr>
<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/overview.png" />
</p>

`goldboot` is approximately what you would get if [`docker`](https://www.docker.com) and [`packer`](https://www.packer.io)
were mixed together. Instead of building containers or virtual machines, `goldboot`
builds images for real hardware.

These machine images (also known as _golden images_) contain your operating
system(s), applications, software patches, and configuration all rolled into one
easily deployable package.

Like Docker images, your `goldboot` images can be stored in a registry and pulled
onto real hardware.

## Installation

<details>
<summary>Docker</summary>

![Docker Pulls](https://img.shields.io/docker/pulls/fossable/goldboot)
![Docker Image Size](https://img.shields.io/docker/image-size/fossable/goldboot)
![Docker Stars](https://img.shields.io/docker/stars/fossable/goldboot)

#### Install from DockerHub

```sh
alias goldboot="docker run --rm -v .:/root fossable/goldboot"
```
</details>

<details>
<summary>Crates.io</summary>

![Crates.io Total Downloads](https://img.shields.io/crates/d/goldboot)

#### Install from crates.io

```sh
cargo install goldboot
```
</details>

<details>
<summary>Arch Linux</summary>

![AUR Votes](https://img.shields.io/aur/votes/goldboot)
![AUR Version](https://img.shields.io/aur/version/goldboot)
![AUR Last Modified](https://img.shields.io/aur/last-modified/goldboot)

#### Install from the AUR

```sh
  cd /tmp
  curl https://aur.archlinux.org/cgit/aur.git/snapshot/goldboot.tar.gz | tar xf -
  makepkg -si
```
</details>

<details>
<summary>Github Releases</summary>

#### Install manually from Github releases

```sh
curl -o /usr/bin/goldboot https://github.com/fossable/goldboot/releases/download/v0.0.3/goldboot_<platform>
```

##### Dependencies
```sh
apt-get install -y libudev1 libgtk-4-1 libglib2.0-0
```
</details>

<details>
<summary>Github Actions</summary>

#### Running on Github actions

Building golden images with CI is common practice, so there's also a [Github
action](https://github.com/fossable/goldboot-action) to make it easy:

```yml
steps:
  - name: Checkout
    uses: actions/checkout@v4

  - name: Build goldboot image
    uses: fossable/goldboot-action@main
    with:
      config-path: goldboot.json
      output-path: image.gb

  - name: Save image artifact
    uses: actions/upload-artifact@v3
    with:
      name: my_image.gb
      path: image.gb
```
</details>

## Examples

The [goldboot-examples](https://github.com/fossable/goldboot-examples) repo contains example
configurations of all supported OS types and system architectures. They are built on a weekly
schedule against the latest version of `goldboot`.

| Linux | Windows | macos |
| ----- | ------- | ----- |
| ![Alpine](goldboot/src/foundry/molds/alpine/icon.png)         ![x86_64](https://github.com/fossable/goldboot-examples/workflows/Alpine/badge.svg)    | ![Windows 10](goldboot/src/foundry/molds/windows_10/icon.png) ![x86_64](https://github.com/fossable/goldboot-examples/workflows/Windows10/badge.svg) | ![macOS](goldboot/src/foundry/molds/arch_linux/mac_os.png) ![x86_64](https://github.com/fossable/goldboot-examples/workflows/Macos/badge.svg) |
| ![Arch Linux](goldboot/src/foundry/molds/arch_linux/icon.png) ![x86_64](https://github.com/fossable/goldboot-examples/workflows/ArchLinux/badge.svg) | |
| ![Debian](goldboot/src/foundry/molds/debian/icon.png)         ![x86_64](https://github.com/fossable/goldboot-examples/workflows/Debian/badge.svg)    | |
| ![Pop!_OS](goldboot/src/foundry/molds/pop_os/icon.png)        ![x86_64](https://github.com/fossable/goldboot-examples/workflows/Pop!_OS/badge.svg)   | |
| ![Steam Deck](goldboot/src/foundry/molds/steam_deck/icon.png) ![x86_64](https://github.com/fossable/goldboot-examples/workflows/SteamDeck/badge.svg) | |
| ![Steam OS](goldboot/src/foundry/molds/steam_os/icon.png)     ![x86_64](https://github.com/fossable/goldboot-examples/workflows/SteamOs/badge.svg)   | |

## Example walkthrough

Let's build a basic Arch Linux ![ArchLinux](goldboot/src/foundry/molds/arch_linux/icon.png)
image to prove we're _real_ Linux users.

First, create a directory to hold our configuration (which can later be tracked
in version control):

```sh
mkdir Test && cd Test
```

Initialize the directory and choose `ArchLinux` to start with:

```sh
goldboot init \
  --name Test \
  --mold ArchLinux \
  --size 10G \
  --format json
```

This will create `goldboot.json` which contains configuration options that can
be tweaked to suit your needs. For example:

```json
{
  "alloy": [
    {
      "mold": {
        "ArchLinux": {
          "hostname": "YeahIUseArch",
          "root_password": {
            "plaintext": "123456"
          }
        }
      },
      "source": {
        "Iso": {
          "url": "https://mirrors.edge.kernel.org/archlinux/iso/2024.01.01/archlinux-2024.01.01-x86_64.iso",
          "checksum": "sha256:12addd7d4154df1caf5f258b80ad72e7a724d33e75e6c2e6adc1475298d47155"
        }
      }
    }
  ],
  "arch": "Amd64",
  "name": "Test",
  "size": "10G"
}
```

There are many ways to customize the image, but for now just build it:

```sh
goldboot cast .
```

Once the build succeeds, the image will be saved to the system's library
directory. To deploy it to a physical disk, you can include the image on a
new bootable USB drive:

```sh
# THIS WILL OVERWRITE /dev/sdX!
goldboot liveusb --output /dev/sdX --include Test
```

Once the USB is created, you can use it to boot into the goldboot live
environment and select an image to deploy:

<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/select_image.png" />
</p>

Once the image has been deployed, remove the bootable USB drive and reboot the
machine.
