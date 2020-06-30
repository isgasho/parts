//! Gpt stuff
#![allow(missing_docs)]
use core::fmt;
use uuid::Uuid;

pub type Result<T, E = ()> = core::result::Result<T, E>;

/// A simple `no_std` error type for [`Read`].
#[derive(Debug)]
pub struct ReadError;

#[cfg(feature = "std")]
impl std::error::Error for ReadError {}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error reading")
    }
}

/// A GUID Partition Table
#[derive(Debug)]
pub struct Gpt {
    /// Disk UUID
    uuid: Uuid,

    /// *Used* partitions.
    partitions: Vec<()>,

    /// Disk size.
    disk_size: u64,

    /// Block size.
    block_size: u64,
}

impl Gpt {
    /// Create a new, empty, GUID Partition Table.
    pub fn new() -> Self {
        Self::with_uuid(Uuid::new_v4())
    }

    /// Create a new, empty, GUID Partition Table with the provided UUID.
    pub fn with_uuid(uuid: Uuid) -> Self {
        Self {
            uuid,
            partitions: Vec::new(),
            disk_size: 0,
            block_size: 0,
        }
    }

    /// Read a GUID Partition Table
    pub fn read<R>(_source: R) -> Result<Self> {
        todo!()
    }

    /// Read a GUID Partition Table using the function `F`.
    pub fn read_fn<F: FnMut(u64, &mut [u8]) -> Result<(), ReadError>>(_: F) -> Result<Self> {
        todo!()
    }

    /// Set GPT disk size
    pub fn set_size(&mut self, disk_size: u64) {
        self.disk_size = disk_size;
    }

    /// Set GPT block size
    pub fn set_block_size(&mut self, block_size: u64) {
        self.block_size = block_size;
    }
}

impl Default for Gpt {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read() {
        let source: &[u8] = &[1; 1];
        let f = std::fs::File::open("path").unwrap();
        let _ = Gpt::read(source);
        let _ = Gpt::read(f);
        let _ = Gpt::read_fn(|_, buf| {
            buf.copy_from_slice(source);
            Ok(())
        });
    }
}
