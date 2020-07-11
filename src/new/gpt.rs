//! Gpt stuff
#![allow(missing_docs)]
use super::mbr::{ProtectiveMbr, MBR_SIZE};
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
    pub fn new(block_size: u64, disk_size: u64) -> Self {
        Self::with_uuid(block_size, disk_size, Uuid::new_v4())
    }

    /// Create a new, empty, GUID Partition Table with the provided UUID.
    pub fn with_uuid(block_size: u64, disk_size: u64, uuid: Uuid) -> Self {
        Self {
            uuid,
            partitions: Vec::new(),
            disk_size,
            block_size,
        }
    }

    /// Read a GUID Partition Table from `source`.
    ///
    /// # Errors
    ///
    /// - If `source` does.
    /// - If the GUID Partition Table is invalid.
    #[cfg(feature = "std")]
    pub fn read<R: std::io::Read + std::io::Seek>(mut source: R, block_size: u64) -> Result<Self> {
        let disk_size = source.seek(std::io::SeekFrom::End(0)).or(Err(()))?;
        Self::read_fn(
            |offset, buf| {
                source
                    .seek(std::io::SeekFrom::Start(offset))
                    .or(Err(ReadError))?;
                source.read_exact(buf).or(Err(ReadError))?;
                Ok(())
            },
            block_size,
            disk_size,
        )
    }

    /// Read a GUID Partition Table from `source`.
    ///
    /// # Errors
    ///
    /// - If the GUID Partition Table is invalid.
    pub fn read_bytes(source: &[u8], block_size: u64) -> Result<Self> {
        Self::read_fn(
            |offset, buf| {
                let offset = usize::try_from(offset).or(Err(ReadError))?;
                buf.copy_from_slice(
                    source
                        .get(offset..)
                        .ok_or(ReadError)?
                        .get(..buf.len())
                        .ok_or(ReadError)?,
                );
                Ok(())
            },
            block_size,
            source.len() as u64,
        )
    }

    /// Read a GUID Partition Table using the function `F`.
    ///
    /// `F` is a function or closure taking an offset and a buffer to write in
    /// to. The buffer must be fully filled.
    ///
    /// This is useful in `no_std` environments, where `std::io::Read`
    /// doesn't exist, and it would be impractical to read the entire disk into
    /// a slice.
    ///
    /// # Errors
    ///
    /// - If `F` does.
    /// - If the GUID Partition Table is invalid.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use parts::new_gpt::Gpt;
    /// # fn no_std_read_at(_: u64, _:&mut [u8]) {}
    /// # let disk_size = 0;
    /// # let block_size = 0;
    ///
    /// Gpt::read_fn(
    ///     |offset, buf| {
    ///         no_std_read_at(offset, buf);
    ///         Ok(())
    ///     },
    ///     block_size,
    ///     disk_size,
    /// )
    /// .unwrap();
    /// ```
    pub fn read_fn<F: FnMut(u64, &mut [u8]) -> Result<(), ReadError>>(
        mut fun: F,
        block_size: u64,
        _disk_size: u64,
    ) -> Result<Self> {
        let mut mbr = [0; MBR_SIZE];
        fun(0, &mut mbr).or(Err(()))?;
        let _ = ProtectiveMbr::read(&mbr, block_size);
        todo!()
    }

    /// Write the GUID Partition Table to `dest`.
    ///
    /// # Errors
    ///
    /// - If `dest` does.
    #[cfg(feature = "std")]
    pub fn write<WS: std::io::Write + std::io::Seek>(&self, mut dest: WS) -> Result<()> {
        self.write_fn(|offset, buf| {
            dest.seek(std::io::SeekFrom::Start(offset))
                .or(Err(ReadError))?;
            dest.write_all(buf).or(Err(ReadError))?;
            Ok(())
        })
    }

    /// Write the GUID Partition Table to `dest`.
    ///
    /// # Errors
    ///
    /// - If `dest` is too small.
    pub fn write_bytes(&self, dest: &mut [u8]) -> Result<()> {
        self.write_fn(|offset, buf| {
            let offset = usize::try_from(offset).or(Err(ReadError))?;
            dest.get_mut(offset..)
                .ok_or(ReadError)?
                .get_mut(..buf.len())
                .ok_or(ReadError)?
                .copy_from_slice(buf);
            Ok(())
        })
    }

    /// Write the GUID Partition Table using the function `F`.
    ///
    /// `F` is a function or closure taking an offset and a buffer to write
    /// from.
    ///
    /// This is useful in `no_std` environments, where `std::io::Write`
    /// doesn't exist, and it would be impractical to use a slice.
    ///
    /// # Errors
    ///
    /// - If `F` does.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use parts::new_gpt::Gpt;
    /// # fn no_std_write_at(_: u64, _: &[u8]) {}
    /// # let gpt = Gpt::new(0, 0);
    ///
    /// gpt.write_fn(|offset, buf| {
    ///     no_std_write_at(offset, buf);
    ///     Ok(())
    /// }).unwrap();
    /// ```
    pub fn write_fn<F: FnMut(u64, &[u8]) -> Result<(), ReadError>>(&self, _: F) -> Result<()> {
        todo!()
    }
}
