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

/// A simple `no_std` compatible `Read` trait.
///
/// On `no_std`, it's implemented for `&[u8]`,
/// and for std it's implemented for `T: io::Read`,
/// which *also* includes `&[u8]`.
pub trait Read {
    /// Read the exact number of bytes required to fill `buf`.
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), ReadError>;
}

#[cfg(not(feature = "std"))]
impl Read for &[u8] {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), ReadError> {
        let len = core::cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(len);
        buf[..len].copy_from_slice(a);
        *self = b;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<T: std::io::Read> Read for T {
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), ReadError> {
        self.read_exact(buf).or(Err(ReadError))
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
    pub fn read<R: Read>(_source: R) -> Result<Self> {
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
