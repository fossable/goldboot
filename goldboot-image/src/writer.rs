use crate::GoldbootImage;
use std::io::{Seek, Write};

pub struct Writer<'qcow, 'writer, W>
where
	W: Write + Seek,
{
	qcow: &'qcow GoldbootImage,

	writer: &'writer mut W,

	/// current position of the writer within the guest
	pos: u64,
}
