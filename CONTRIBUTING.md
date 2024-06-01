# Contributing

This guide contains helpful information for first time contributors.

## Crate architecture

There are currently four crates to know about:

- `goldboot`
  - The primary CLI application for building goldboot images
- `goldboot-image`
  - Implements the goldboot image format
- `goldboot-macros`
  - Procedural macros
- `goldboot-registry`
  - Web service that hosts goldboot images

## The metallurgy metaphor

Although end-users could probably ignore it, the internals of `goldboot` use vocabulary
taken from the field of metallurgy.

#### Foundry

An image foundry is a configuration object that knows how to build goldboot images.

#### OS

An image mold takes an image source and refines it according to built-in rules.
For example, the `ArchLinux` mold knows how to take Arch Linux install media (in
the form of an ISO) and install it in an automated manner.

#### Casting

Casting (or building) is the process that takes image sources and produces
a final goldboot image containing all customizations.

Under the hood, foundries cast images by spawning a Qemu virtual machine and
running one or more image molds against it (via SSH or VNC). Once the virtual
machine is fully provisioned, its shutdown and the underlying storage (.qcow2)
is converted into a final goldboot image (.gb).

#### Alloys

An alloy is a multi-boot image.

#### Fabricators

Operates on images at the end of the casting process. For example, the shell
fabricator runs shell commands on the image which can be useful in many cases.

## Adding new operating systems

If `goldboot` doesn't already support your operating system, it should be possible
to add it relatively easily.

Start by finding an OS similar to yours in the `goldboot::foundry::os` module.

TODO

## OS maintenance

OS support often need constant maintenance as new upstream releases are made and old
ones are retired. Typically this involves adding new versions and marking some
as deprecated, but occasionally upstream changes may cause breakages for us.

For example, we have a struct that tracks Alpine releases which needs to be
updated about twice per year:

```rust
#[derive(Clone, Serialize, Deserialize, Debug, EnumIter)]
pub enum AlpineRelease {
    Edge,
    #[serde(rename = "v3.17")]
    V3_17,
    #[serde(rename = "v3.16")]
    V3_16,
    #[serde(rename = "v3.15")]
    V3_15,
    #[deprecated]
    #[serde(rename = "v3.14")]
    V3_14,
}
```

## Testing
TODO
