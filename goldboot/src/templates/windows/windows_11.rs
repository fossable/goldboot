use crate::provisioners::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use validator::Validate;

#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct Windows11Template {
	pub id: TemplateId,

	pub iso: IsoSource,
	pub ansible: Option<Vec<AnsibleProvisioner>>,
}
