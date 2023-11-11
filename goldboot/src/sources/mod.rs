use std::error::Error;

///! Contains general-purpose sources for use in templates.
pub mod iso;

/// All builds start with a single `Source` which provides the initial image
/// to be subjected to further customizations.
pub trait Source {
    fn load(&self) -> Result<String, Box<dyn Error>>;
}
