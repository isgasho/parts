//! Gpt stuff
#![allow(missing_docs)]
use core::{convert::TryFrom, fmt};
use uuid::Uuid;

pub type Result<T, E = ()> = core::result::Result<T, E>;

/// A simple `no_std` error type.
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
    #[cfg(feature = "std")]
    pub fn read<R: std::io::Read + std::io::Seek>(mut source: R) -> Result<Self> {
        Self::read_fn(|offset, buf| {
            source
                .seek(std::io::SeekFrom::Start(offset))
                .or(Err(ReadError))?;
            source.read_exact(buf).or(Err(ReadError))?;
            Ok(())
        })
    }

    /// Read a GUID Partition Table
    pub fn read_bytes(source: &[u8]) -> Result<Self> {
        Self::read_fn(|offset, buf| {
            let offset = usize::try_from(offset).or(Err(ReadError))?;
            buf.copy_from_slice(
                source
                    .get(offset..)
                    .ok_or(ReadError)?
                    .get(..buf.len())
                    .ok_or(ReadError)?,
            );
            Ok(())
        })
    }

    /// Read a GUID Partition Table using the function `F`.
    ///
    /// `F` is a function or closure taking an offset and a buffer to write in
    /// to.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use parts::new_gpt::Gpt;
    /// # fn no_std_read_at(_:u64, _:&mut [u8])
    ///
    /// Gpt::read_fn(|offset, buf| {
    ///     no_std_read_at(offset, buf);
    ///     Ok(())
    /// }).unwrap();
    /// ```
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
