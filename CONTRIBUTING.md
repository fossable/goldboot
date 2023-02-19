# Contributing

Thanks for all of the interest in `goldboot`! This guide contains helpful information
for first time contributors.

## Crate Architecture

There are currently four crates to know about:

- `goldboot`
  - The primary CLI application for building goldboot images
  - Current estimated completion: 60%
- `goldboot-graphics`
  - Contains common graphical utilities
  - Current estimated completion: 100%
- `goldboot-linux`
  - Specialized Linux distribution for applying goldboot images
  - Current estimated completion: 50%
- `goldboot-registry`
  - Web service that hosts goldboot images
  - Current estimated completion: 10%

## Adding new templates

If `goldboot` doesn't already support your operating system, it should be possible
to add it relatively easily.

Start by finding a template similar to your operating system in the `goldboot::templates`
module.

TODO

## Template Maintenance

As a result of the nature of software, templates need constant maintenance as
new releases are made and old ones are retired. Typically this involves adding
new versions and marking old ones as deprecated, but occasionally upstream changes
may cause breakages for us.

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

In the future, we may designate "official maintainers" for templates that change
frequently to handle the burden.

## Testing
TODO
