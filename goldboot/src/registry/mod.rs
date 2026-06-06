pub mod api;
pub mod client;
pub mod protocol;

pub use client::{Client, registry_root};

// Re-export so callers needn't depend on goldboot_image directly.
pub use goldboot_image::{ImageRef, host_without_scheme};
