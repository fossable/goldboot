use crate::builder::Builder;
use crate::cli::prompt::Prompt;
use anyhow::Result;
use goldboot_image::ImageArch;
use serde::{ser::Serializer, Serialize};

pub mod alpine_linux;
pub mod arch_linux;
pub mod debian;
// pub mod goldboot;
pub mod nix;
pub mod pop_os;
pub mod ubuntu;
pub mod windows_10;
pub mod windows_11;

/// "Building" is the process of generating an immutable goldboot image from raw
/// configuration data.
pub trait BuildImage {
    /// Build an image.
    fn build(&self, builder: &Builder) -> Result<()>;
}

/// Combined trait for OS types that can be used as image elements.
pub trait OsTrait: BuildImage + Prompt + Send + Sync {
    fn os_name(&self) -> &'static str;
    fn os_architectures(&self) -> &'static [ImageArch];
    fn os_alloy(&self) -> bool {
        false
    }
    fn os_size(&self) -> u64;
    fn os_arch(&self) -> ImageArch;
    fn serialize_ron(&self, config: &ron::ser::PrettyConfig) -> anyhow::Result<String>;
}

/// Descriptor registered at link time via `inventory` for each OS type.
pub struct OsDescriptor {
    pub name: &'static str,
    pub architectures: &'static [ImageArch],
    pub default: fn() -> Box<dyn OsTrait>,
    /// Deserialize from a raw RON string (the full `TypeName(...)` form).
    pub deserialize_ron: fn(&str) -> anyhow::Result<Box<dyn OsTrait>>,
}

inventory::collect!(OsDescriptor);

/// Iterate over all registered OS descriptors.
pub fn os_iter() -> impl Iterator<Item = &'static OsDescriptor> {
    inventory::iter::<OsDescriptor>()
}

/// Parse the leading struct-name identifier from a RON string.
/// Returns the identifier before the first `(`, trimmed.
pub fn ron_struct_name(s: &str) -> Option<&str> {
    let name = s.trim().split('(').next()?.trim();
    if name.is_empty() || name.starts_with('{') || name.starts_with('[') {
        None
    } else {
        Some(name)
    }
}

/// Deserialize an `OsConfig` from a raw RON string.
pub fn os_config_from_ron(s: &str) -> anyhow::Result<OsConfig> {
    let name = ron_struct_name(s)
        .ok_or_else(|| anyhow::anyhow!("RON string has no leading struct name"))?;

    let descriptor = os_iter()
        .find(|d| d.name == name)
        .ok_or_else(|| anyhow::anyhow!("unknown OS type: {name}"))?;

    let inner = (descriptor.deserialize_ron)(s)?;
    Ok(OsConfig(inner))
}

/// A boxed OS configuration element (replaces `Os` enum).
pub struct OsConfig(pub Box<dyn OsTrait>);

impl std::fmt::Debug for OsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OsConfig({})", self.0.os_name())
    }
}

impl Serialize for OsConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let ron_config = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .enumerate_arrays(false)
            .compact_arrays(false);

        let s = self
            .0
            .serialize_ron(&ron_config)
            .map_err(serde::ser::Error::custom)?;

        // Re-parse into a ron::Value and serialize through the serializer.
        // Note: this loses struct-name info inside ron::Value, but for serialization
        // we only need the data — the struct name is prepended by serialize_ron.
        let value: ron::Value = ron::from_str(&s).map_err(serde::ser::Error::custom)?;
        value.serialize(serializer)
    }
}

// OsConfig deserialization is handled via os_config_from_ron() in ConfigPath::load,
// not via the serde Deserialize trait, because ron::Value loses struct-name information.
