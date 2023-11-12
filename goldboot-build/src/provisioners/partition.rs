#[derive(Clone, Serialize, Deserialize, Validate, Debug)]
pub struct PartitionProvisioner {
    pub total_size: String,
    // TODO
}

impl PartitionProvisioner {
    pub fn storage_size_bytes(&self) -> u64 {
        self.total_size.parse::<ubyte::ByteUnit>().unwrap().as_u64()
    }
}
