use crate::Qcow2;
use std::io::Seek;
use std::io::Write;

pub struct Writer<'qcow, 'writer, W>
where
    W: Write + Seek,
{
    qcow: &'qcow Qcow2,

    writer: &'writer mut W,

    /// current position of the writer within the guest
    pos: u64,
}
