use std::error::Error;

use dialoguer::theme::ColorfulTheme;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{build::BuildConfig, PromptMut};

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct TimezoneProvisioner {
    // TODO
}

impl PromptMut for TimezoneProvisioner {
    fn prompt(
        &mut self,
        config: &BuildConfig,
        theme: &ColorfulTheme,
    ) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}
