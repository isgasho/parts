//! Raw Gpt stuff
use self::{error::*, header::*, partition::*};
use crate::{
    mbr::{ProtectiveMbr, MBR_SIZE},
    types::*,
};
use arrayvec::{Array, ArrayVec};
use core::{convert::TryInto, fmt};
use crc::{crc32, Hasher32};
use generic_array::{
    sequence::GenericSequence,
    typenum::{Unsigned, U128},
    ArrayLength,
    GenericArray,
};
#[cfg(feature = "std")]
use std::io::{prelude::*, SeekFrom};
use uuid::Uuid;

pub mod error;
mod header;
pub mod partition;

fn validate<F: FnMut(Offset, &mut [u8]) -> Result<()>, CB: FnMut(usize, &[u8])>(
    primary: &Header,
    alt: &Header,
    mut func: F,
    block_size: BlockSize,
    disk_size: Size,
    mut cb: CB,
) -> Result<()> {
    if primary.this != Block::new(1, block_size) {
        return Err(Error::Invalid("Corrupt Primary GPT Header"));
    }
    let crc = calculate_part_crc(
        &mut func,
        primary.partitions as u64,
        primary.array.into_offset(),
        &mut cb,
    )?;
    if crc != primary.partitions_crc32 {
        return Err(Error::Invalid("Primary Partition Array CRC32 mismatch"));
    }
    let last_lba = (disk_size / block_size) - 1;
    if primary.alt != last_lba {
        return Err(Error::Invalid("Corrupt Primary GPT Header"));
    }
    //
    if alt.this != last_lba {
        return Err(Error::Invalid("Corrupt Backup GPT Header"));
    }
    if alt.alt != Block::new(1, block_size) {
        return Err(Error::Invalid("Corrupt Backup GPT Header"));
    }
    let crc = calculate_part_crc(
        &mut func,
        alt.partitions as u64,
        alt.array.into_offset(),
        &mut cb,
    )?;
    if crc != alt.partitions_crc32 {
        return Err(Error::Invalid("Backup Partition Array CRC32 mismatch"));
    }
    //
    Ok(())
}

fn default_partitions<N>() -> GenericArray<Partition, N>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    GenericArray::<Partition, N>::generate(|_| Partition::new())
}

trait _GptHelper<C> {
    fn new() -> C;

    fn as_slice(&self) -> &[Partition];

    fn as_mut_slice(&mut self) -> &mut [Partition];

    fn push(&mut self, part: Partition) -> Result<()>;

    fn remove(&mut self, index: usize) -> Partition;
}

impl<N: Array<Item = Partition>> _GptHelper<ArrayVec<N>> for ArrayVec<N> {
    fn new() -> ArrayVec<N> {
        ArrayVec::new()
    }

    fn as_slice(&self) -> &[Partition] {
        self.as_slice()
    }

    fn as_mut_slice(&mut self) -> &mut [Partition] {
        self.as_mut_slice()
    }

    fn push(&mut self, part: Partition) -> Result<()> {
        self.try_push(part).map_err(|_| Error::Overlap)
    }

    fn remove(&mut self, index: usize) -> Partition {
        self.remove(index)
    }
}

#[cfg(feature = "std")]
impl _GptHelper<Vec<Partition>> for Vec<Partition> {
    fn new() -> Vec<Partition> {
        // TODO: Const
        Vec::with_capacity(128)
    }

    fn as_slice(&self) -> &[Partition] {
        self.as_slice()
    }

    fn as_mut_slice(&mut self) -> &mut [Partition] {
        self.as_mut_slice()
    }

    fn push(&mut self, part: Partition) -> Result<()> {
        self.push(part);
        Ok(())
    }

    fn remove(&mut self, index: usize) -> Partition {
        self.remove(index)
    }
}

#[derive(Debug, PartialEq)]
struct _GptC<C> {
    uuid: Uuid,
    partitions: C,
}

#[cfg(not(feature = "std"))]
type _Gpt<C = ArrayVec<[Partition; 128]>> = _GptC<C>;

#[cfg(feature = "std")]
type _Gpt<C = std::vec::Vec<Partition>> = _GptC<C>;

#[allow(dead_code)]
impl<C: _GptHelper<C>> _Gpt<C> {
    /// New empty Gpt
    ///
    /// WARNING: `uuid` must be unique, such as from [`Uuid::new_v4`].
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            partitions: C::new(),
        }
    }

    pub fn from_bytes(source: &[u8], block_size: BlockSize, disk_size: Size) -> Result<Self> {
        let b_size = block_size.0 as usize;
        let d_size = disk_size.as_bytes() as usize;
        let primary = &source[..b_size * 2];
        let alt = &source[d_size - b_size..];
        _GptC::from_bytes_with_func(
            primary,
            alt,
            |i, buf| {
                let i = i.0 as usize;
                let size = buf.len();
                buf.copy_from_slice(&source[i..][..size]);
                Ok(())
            },
            block_size,
            disk_size,
        )
    }

    pub fn from_bytes_with_func<F: FnMut(Offset, &mut [u8]) -> Result<()>>(
        primary: &[u8],
        alt: &[u8],
        mut func: F,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<Self> {
        let b_size = block_size.0 as usize;
        assert_eq!(primary.len(), b_size * 2, "Invalid primary");
        assert_eq!(alt.len(), b_size, "Invalid alt");
        let _mbr = ProtectiveMbr::from_bytes(&primary[..MBR_SIZE])
            .map_err(|_| Error::Invalid("Invalid Protective MBR"))?;
        let primary = Header::from_bytes(&primary[MBR_SIZE..], block_size)?;
        let alt = Header::from_bytes(alt, block_size)?;
        //
        let mut partitions = C::new();
        validate(
            &primary,
            &alt,
            &mut func,
            block_size,
            disk_size,
            |_, source| {
                let _ = partitions.push(Partition::from_bytes(source, block_size));
            },
        )?;

        Ok(_GptC {
            uuid: primary.uuid,
            partitions,
        })
    }

    #[cfg(feature = "std")]
    pub fn from_reader<RS: Read + Seek>(
        mut source: RS,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<Self> {
        let last_lba = (disk_size / block_size) - 1;
        let mut primary = vec![0; (block_size.0 * 2) as usize];
        let mut alt = vec![0; block_size.0 as usize];
        source.seek(SeekFrom::Start(0))?;
        source.read_exact(&mut primary)?;
        source.seek(SeekFrom::Start(last_lba.into_offset().0))?;
        source.read_exact(&mut alt)?;
        let gpt = _GptC::from_bytes_with_func(
            &primary,
            &alt,
            |i, buf| {
                source.seek(SeekFrom::Start(i.0))?;
                source.read_exact(buf)?;
                Ok(())
            },
            block_size,
            disk_size,
        )?;
        Ok(gpt)
    }

    pub fn to_bytes(&self, dest: &mut [u8], block_size: BlockSize, disk_size: Size) -> Result<()> {
        self.to_bytes_with_func(
            |i, buf| {
                let i = i.0 as usize;
                dest[i..][..buf.len()].copy_from_slice(buf);
                Ok(())
            },
            block_size,
            disk_size,
        )
    }

    pub fn to_bytes_with_func<F: FnMut(Offset, &[u8]) -> Result<()>>(
        &self,
        mut func: F,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<()> {
        let last_lba = (disk_size / block_size) - 1;
        let mbr = ProtectiveMbr::new(last_lba);
        let mut mbr_buf = [0; MBR_SIZE];
        mbr.to_bytes(&mut mbr_buf);
        func(Size::from_bytes(0).into(), &mbr_buf)?;
        //
        let partition_len = self.partitions.as_slice().len().try_into().unwrap();
        let mut partition_buf = [0; PARTITION_ENTRY_SIZE as usize];
        let mut digest = crc32::Digest::new(crc32::IEEE);
        // FIXME: Invalid for N lower than 128?
        for part in self.partitions.as_slice() {
            part.to_bytes(&mut partition_buf, block_size);
            digest.write(&partition_buf);
        }
        let parts_crc = digest.sum32();
        let disk_uuid = self.uuid;

        let alt = Header::new(
            last_lba,
            Block::new(1, block_size),
            partition_len,
            parts_crc,
            disk_uuid,
            block_size,
            disk_size,
        );
        // Verify all partitions are within bounds
        for part in self.partitions() {
            let a = part.start() / block_size;
            let b = part.end() / block_size;
            if (a < alt.first_usable) || (b > alt.last_usable) {
                return Err(Error::NotEnough);
            }
        }
        //
        self.write_header_array(&mut func, alt, last_lba, block_size)?;
        //
        let primary = Header::new(
            Block::new(1, block_size),
            last_lba,
            partition_len,
            parts_crc,
            disk_uuid,
            block_size,
            disk_size,
        );
        self.write_header_array(func, primary, Block::new(1, block_size), block_size)?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn to_writer<WS: Write + Seek>(
        &self,
        mut dest: WS,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<()> {
        self.to_bytes_with_func(
            |i, buf| {
                dest.seek(SeekFrom::Start(i.0))?;
                dest.write_all(buf)?;
                Ok(())
            },
            block_size,
            disk_size,
        )?;
        Ok(())
    }

    /// Unique Disk UUID
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Slice of in-use partitions
    pub fn partitions(&self) -> &[Partition] {
        self.partitions.as_slice()
    }

    /// Mutable slice of in-use partitions.
    pub fn partitions_mut(&mut self) -> &mut [Partition] {
        self.partitions.as_mut_slice()
    }

    /// Add a partition.
    ///
    /// # Errors
    ///
    /// - If `part` overlaps with existing partitions
    /// - In `no_std`, if `part` would overflow `C`.
    pub fn add_partition(&mut self, part: Partition) -> Result<()> {
        self.check_overlap(&part)?;
        self.partitions.push(part)?;
        self.partitions_mut().sort_unstable_by_key(|p| p.start());
        Ok(())
    }

    /// Remove the partition at `index`.
    // FIXME: Where is the index supposed to come from, exactly?
    pub fn remove_partition(&mut self, index: usize) -> Partition {
        self.partitions.remove(index)
    }

    /// Set the disk UUID.
    ///
    /// WARNING: Gpt UUID's MUST be unique.
    /// Only use this if you know what you're doing.
    pub fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }
}

/// Private APIs
#[allow(dead_code)]
impl<C: _GptHelper<C>> _GptC<C> {
    fn check_overlap(&mut self, part: &Partition) -> Result<()> {
        for existing in self.partitions() {
            if part.start() >= existing.start() && part.start() <= existing.end() {
                return Err(Error::Overlap);
            }
        }
        Ok(())
    }

    fn write_header_array<F: FnMut(Offset, &[u8]) -> Result<()>>(
        &self,
        mut func: F,
        header: Header,
        last_lba: Block,
        block_size: BlockSize,
    ) -> Result<()> {
        let mut header_buf = [0; HEADER_SIZE as usize];
        let mut partition_buf = [0; PARTITION_ENTRY_SIZE as usize];
        //
        header.to_bytes(&mut header_buf);
        func(last_lba.into_offset(), &header_buf)?;
        for (i, part) in self.partitions.as_slice().iter().enumerate() {
            part.to_bytes(&mut partition_buf, block_size);
            let b =
                Offset(header.array.into_offset().0 + ((PARTITION_ENTRY_SIZE as u64) * i as u64));
            func(b, &partition_buf)?;
        }
        //
        Ok(())
    }
}

#[cfg(test)]
mod test_new {
    use super::*;

    #[test]
    fn feature() {
        let gpt: _Gpt = _Gpt::new(Uuid::new_v4());
        dbg!(gpt);
        panic!();
    }
}

/// Represents a GUID Partition Table
///
/// Note that all modifications are done in-memory
/// and *only* effect partition entries, not the data in them.
///
/// # Memory
///
/// By default this takes `16KiB + 24 bytes` of space.
/// The UEFI spec requires a minimum of 16KiB reserved for partitions,
/// which is 128 partitions if each entry is 128 bytes,
/// which we assume by default.
///
/// If you are particularly space-constrained, or need to support more,
/// you can use [`crate::typenum`] to set the `N` generic parameter to
/// the number of partitions you want to support.
///
/// When doing this unsupported partitions will ***not*** be preserved,
/// and depending on `N`, and what you do, may be overwritten by
/// newly added partitions.
///
/// Regardless of `N`, when reading the full partition array *is* still
/// validated.
///
/// **Warning:** Using values of `N` less than `128` may cause the Gpt
/// to be considered invalid by other tools or hardware.
// TODO: No writing for any N?
#[derive(Copy, Clone, PartialEq)]
pub struct Gpt<N = U128>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    uuid: Uuid,
    partitions: GenericArray<Partition, N>,
    /// Real size of the partitions array.
    /// u64, not u32, for alignment reasons.
    partitions_len: u64,
}

impl Gpt {
    /// Read the GPT from a byte slice
    ///
    /// See [`Gpt::from_bytes_with_size`] if getting a slice of
    /// the entire disk isn't possible.
    pub fn from_bytes(source: &[u8], block_size: BlockSize, disk_size: Size) -> Result<Self> {
        let b_size = block_size.0 as usize;
        let d_size = disk_size.as_bytes() as usize;
        let primary = &source[..b_size * 2];
        let alt = &source[d_size - b_size..];
        Gpt::from_bytes_with_size(
            primary,
            alt,
            |i, buf| {
                let i = i.0 as usize;
                let size = buf.len();
                buf.copy_from_slice(&source[i..][..size]);
                Ok(())
            },
            block_size,
            disk_size,
        )
    }

    /// Write the full GPT to a byte slice
    ///
    /// See [`Gpt::to_bytes_with_size`] if getting a slice of
    /// the entire disk isn't possible,
    /// and for details of what gets written and in what order.
    pub fn to_bytes(&self, dest: &mut [u8], block_size: BlockSize, disk_size: Size) -> Result<()> {
        self.to_bytes_with_size(
            |i, buf| {
                let i = i.0 as usize;
                dest[i..][..buf.len()].copy_from_slice(buf);
                Ok(())
            },
            block_size,
            disk_size,
        )
    }
}

#[cfg(feature = "std")]
impl Gpt {
    /// Read the GPT from a [`Read`]er.
    ///
    /// See [`Gpt::from_bytes`] for more details.
    pub fn from_reader<RS: Read + Seek>(
        source: RS,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<Self> {
        Gpt::from_reader_with_size(source, block_size, disk_size)
    }

    /// Write the GPT to a [`Write`]r.
    ///
    /// See [`Gpt::to_bytes`] for more details.
    pub fn to_writer<WS: Write + Seek>(
        &self,
        dest: WS,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<()> {
        self.to_writer_with_size(dest, block_size, disk_size)
    }
}

impl<N> Gpt<N>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    /// Like [`Gpt::from_bytes`] but stores `N` partitions
    /// instead of the minimum reserved amount.
    ///
    /// You probably don't want this method, but it can be useful
    /// if you're fine with only supporting a few partitions.
    ///
    /// `primary` must contain LBA0 and LBA1. That is, `block_size * 2` bytes.
    ///
    /// `alt` must be the last LBA. That is, `block_size` bytes.
    ///
    /// `func` is called to read data. Errors are propagated.
    /// It's arguments are a byte offset and a buffer to read into.
    pub fn from_bytes_with_size<F: FnMut(Offset, &mut [u8]) -> Result<()>>(
        primary: &[u8],
        alt: &[u8],
        mut func: F,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<Self> {
        let b_size = block_size.0 as usize;
        assert_eq!(primary.len(), b_size * 2, "Invalid primary");
        assert_eq!(alt.len(), b_size, "Invalid alt");
        let _mbr = ProtectiveMbr::from_bytes(&primary[..MBR_SIZE])
            .map_err(|_| Error::Invalid("Invalid Protective MBR"))?;
        let primary = Header::from_bytes(&primary[MBR_SIZE..], block_size)?;
        let alt = Header::from_bytes(alt, block_size)?;
        //
        let mut partitions = default_partitions();
        validate(
            &primary,
            &alt,
            &mut func,
            block_size,
            disk_size,
            |i, source| {
                if i < partitions.len() {
                    partitions[i] = Partition::from_bytes(source, block_size);
                }
            },
        )?;

        Ok(Gpt {
            uuid: primary.uuid,
            // Only count used partitions.
            partitions_len: partitions
                .iter()
                .filter(|p| **p != Partition::new())
                .count() as u64,
            partitions,
        })
    }

    /// Like [`Gpt::to_bytes`] but stores `N` partitions.
    ///
    /// `disk_size` must be the size of the device.
    ///
    /// `func` will be called to write data.
    ///
    /// As an argument it receives the byte offset to write at, and
    /// a buffer of data to write.
    ///
    /// It returns a `Result<()>`, and errors are propagated.
    ///
    /// # Errors
    ///
    /// If all partitions do not fit within the usable blocks
    ///
    /// # Details
    ///
    /// This will tell `func` to write, in order:
    ///
    /// - The Protective MBR
    /// - The backup header
    /// - The backup header array
    /// - The primary header
    /// - The primary header array
    pub fn to_bytes_with_size<F: FnMut(Offset, &[u8]) -> Result<()>>(
        &self,
        mut func: F,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<()> {
        let last_lba = (disk_size / block_size) - 1;
        let mbr = ProtectiveMbr::new(last_lba);
        let mut mbr_buf = [0; MBR_SIZE];
        mbr.to_bytes(&mut mbr_buf);
        func(Size::from_bytes(0).into(), &mbr_buf)?;
        //
        let mut partition_buf = [0; PARTITION_ENTRY_SIZE as usize];
        let mut digest = crc32::Digest::new(crc32::IEEE);
        // FIXME: Invalid for N lower than 128?
        for part in self.partitions {
            part.to_bytes(&mut partition_buf, block_size);
            digest.write(&partition_buf);
        }
        let parts_crc = digest.sum32();
        let disk_uuid = self.uuid;

        let alt = Header::new(
            last_lba,
            Block::new(1, block_size),
            self.partitions.len() as u32,
            parts_crc,
            disk_uuid,
            block_size,
            disk_size,
        );
        // Verify all partitions are within bounds
        for part in self.partitions() {
            let a = part.start() / block_size;
            let b = part.end() / block_size;
            if (a < alt.first_usable) || (b > alt.last_usable) {
                return Err(Error::NotEnough);
            }
        }
        //
        self.write_header_array(&mut func, alt, last_lba, block_size)?;
        //
        let primary = Header::new(
            Block::new(1, block_size),
            last_lba,
            self.partitions.len() as u32,
            parts_crc,
            disk_uuid,
            block_size,
            disk_size,
        );
        self.write_header_array(func, primary, Block::new(1, block_size), block_size)?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<N> Gpt<N>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    /// Create a new GPT Table with no partitions
    ///
    /// This requires the `std` feature to generate a unique Uuid.
    ///
    /// See [`Gpt::with_uuid`] for a `no_std` solution.
    // Gpt can't be `Default`, this is std only.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Gpt {
            uuid: Uuid::new_v4(),
            partitions: default_partitions(),
            partitions_len: 0,
        }
    }

    /// Like [`Gpt::from_bytes_with_size`] but for readers
    pub fn from_reader_with_size<RS: Read + Seek>(
        mut source: RS,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<Self> {
        let last_lba = (disk_size / block_size) - 1;
        let mut primary = vec![0; (block_size.0 * 2) as usize];
        let mut alt = vec![0; block_size.0 as usize];
        source.seek(SeekFrom::Start(0))?;
        source.read_exact(&mut primary)?;
        source.seek(SeekFrom::Start(last_lba.into_offset().0))?;
        source.read_exact(&mut alt)?;
        let gpt = Gpt::from_bytes_with_size(
            &primary,
            &alt,
            |i, buf| {
                source.seek(SeekFrom::Start(i.0))?;
                source.read_exact(buf)?;
                Ok(())
            },
            block_size,
            disk_size,
        )?;
        Ok(gpt)
    }

    /// Like [`Gpt::to_bytes_with_size`] but for writers
    pub fn to_writer_with_size<WS: Write + Seek>(
        &self,
        mut dest: WS,
        block_size: BlockSize,
        disk_size: Size,
    ) -> Result<()> {
        self.to_bytes_with_size(
            |i, buf| {
                dest.seek(SeekFrom::Start(i.0))?;
                dest.write_all(buf)?;
                Ok(())
            },
            block_size,
            disk_size,
        )?;
        Ok(())
    }
}

// Public APIs
impl<N> Gpt<N>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    /// Create a new GPT Table
    ///
    /// See [`Gpt::new`] for details.
    // TODO: Take random data instead?
    // Don't want people using a duplicate Uuid from Gpt::uuid?
    // Plus need to create Uuid's for partitions? Problem for the Builder?
    pub fn with_uuid(uuid: Uuid) -> Self {
        Gpt {
            uuid,
            partitions: default_partitions(),
            partitions_len: 0,
        }
    }

    /// Disk UUID
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Slice of in-use partitions
    pub fn partitions(&self) -> &[Partition] {
        let len = core::cmp::min(self.partitions_len as usize, self.partitions.len());
        &self.partitions[..len]
    }

    /// Mutable slice of in-use partitions.
    pub fn partitions_mut(&mut self) -> &mut [Partition] {
        let len = core::cmp::min(self.partitions_len as usize, self.partitions.len());
        &mut self.partitions[..len]
    }

    /// Add a partition
    pub fn add_partition(&mut self, part: Partition) -> Result<()> {
        self.check_overlap(&part)?;
        let len = core::cmp::min(self.partitions_len as usize, self.partitions.len());
        self.partitions[len] = part;
        self.partitions_len += 1;
        self.partitions_mut().sort_unstable_by_key(|p| p.start());
        Ok(())
    }

    /// Remove the partition at `index`.
    // FIXME: Where is the index supposed to come from, exactly?
    pub fn remove_partition(&mut self, index: usize) {
        self.partitions_mut()[index] = Partition::new();
        self.partitions_len -= 1;
        self.partitions_mut().sort_unstable_by_key(|p| p.start());
    }

    /// Set the disk UUID.
    ///
    /// Be careful with this, as UUID's MUST be unique.
    pub fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }

    // TODO: First/last usable block getters, and remaining. Store disk/block size?
}

// Private APIs
impl<N> Gpt<N>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    fn check_overlap(&mut self, part: &Partition) -> Result<()> {
        for existing in self.partitions() {
            if part.start() >= existing.start() && part.start() <= existing.end() {
                return Err(Error::Overlap);
            }
        }
        Ok(())
    }

    fn write_header_array<F: FnMut(Offset, &[u8]) -> Result<()>>(
        &self,
        mut func: F,
        header: Header,
        last_lba: Block,
        block_size: BlockSize,
    ) -> Result<()> {
        let mut header_buf = [0; HEADER_SIZE as usize];
        let mut partition_buf = [0; PARTITION_ENTRY_SIZE as usize];
        //
        header.to_bytes(&mut header_buf);
        func(last_lba.into_offset(), &header_buf)?;
        for (i, part) in self.partitions.iter().enumerate() {
            part.to_bytes(&mut partition_buf, block_size);
            let b =
                Offset(header.array.into_offset().0 + ((PARTITION_ENTRY_SIZE as u64) * i as u64));
            func(b, &partition_buf)?;
        }
        //
        Ok(())
    }
}

impl<N> fmt::Debug for Gpt<N>
where
    N: ArrayLength<Partition> + Unsigned,
    N::ArrayType: Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gpt")
            .field("uuid", &self.uuid)
            .field("partitions", &self.partitions())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        partitions::PartitionType,
        util::{Result, *},
    };
    use core::mem;
    use generic_array::{
        typenum::{Unsigned, U0, U128, U256, U64},
        ArrayLength,
    };
    use static_assertions::*;
    use std::io;

    assert_eq_size!(
        Gpt,
        [u8; MIN_PARTITIONS_BYTES as usize + mem::size_of::<[u8; 16]>() + mem::size_of::<u64>()]
    );

    fn read_gpt_size<N>(raw: &[u8]) -> Result<Gpt<N>>
    where
        N: Unsigned,
        N: ArrayLength<Partition>,
        N::ArrayType: Copy,
    {
        let block = BLOCK_SIZE.0 as usize;

        let primary = &raw[..block * 2];
        let alt = &raw[raw.len() - block..];
        let gpt: Gpt<N> = Gpt::from_bytes_with_size(
            &primary,
            &alt,
            |i, buf| {
                let i = i.0 as usize;
                let size = buf.len();
                buf.copy_from_slice(&raw[i..][..size]);
                Ok(())
            },
            BLOCK_SIZE,
            Size::from_bytes(TEN_MIB_BYTES as u64),
        )
        .map_err(anyhow::Error::msg)?;
        //
        Ok(gpt)
    }

    /// Confirm the GPT has what we expect
    fn expected_gpt(gpt: Gpt) {
        assert_eq!(gpt.partitions().len(), 1);
        assert_eq!(
            gpt.partitions()[0].uuid(),
            Uuid::parse_str(CF_PART_GUID).unwrap()
        );
        assert_eq!(gpt.uuid, Uuid::parse_str(CF_DISK_GUID).unwrap());
    }

    /// Test that we can read a GPT from another tool
    #[test]
    fn read_gpt() -> Result {
        let raw = data()?;
        let gpt = read_gpt_size::<U128>(&raw)?;
        read_gpt_size::<U0>(&raw)?;
        read_gpt_size::<U64>(&raw)?;
        read_gpt_size::<U256>(&raw)?;
        Gpt::from_bytes(&raw, BLOCK_SIZE, Size::from_bytes(TEN_MIB_BYTES as u64))?;
        //
        expected_gpt(gpt);
        //
        Ok(())
    }

    //
    #[test]
    fn gpt_roundtrip() -> Result {
        let mut dest = vec![0; TEN_MIB_BYTES];
        let gpt = read_gpt_size::<U128>(&data()?)?;
        gpt.to_bytes_with_size(
            |i, buf| {
                let i = i.0 as usize;
                dest[i..][..buf.len()].copy_from_slice(buf);
                Ok(())
            },
            BLOCK_SIZE,
            Size::from_bytes(TEN_MIB_BYTES as u64),
        )
        .map_err(anyhow::Error::msg)?;
        let new_gpt = read_gpt_size::<U128>(&dest)?;
        assert_eq!(new_gpt, gpt);
        //
        gpt.to_bytes(
            &mut dest,
            BLOCK_SIZE,
            Size::from_bytes(TEN_MIB_BYTES as u64),
        )?;
        let new_gpt = read_gpt_size::<U128>(&dest)?;
        assert_eq!(new_gpt, gpt);
        Ok(())
    }

    #[test]
    #[should_panic = "Invalid Protective MBR"]
    fn missing_mbr_test() {
        let raw = [0; 1024];
        let _gpt = read_gpt_size::<U128>(&raw).unwrap();
    }

    #[test]
    #[should_panic = "Invalid Signature"]
    fn missing_gpt_test() {
        let mut raw = data().unwrap();
        raw[512..][..512].copy_from_slice(&[0; 512]);
        let _gpt = read_gpt_size::<U128>(&raw).unwrap();
    }

    /// Test that the from_reader/to_writer methods work correctly
    #[test]
    #[cfg(feature = "std")]
    fn std_gpt_test() -> Result {
        let raw = data()?;
        let raw = std::io::Cursor::new(raw);
        let gpt = Gpt::from_reader(raw, BLOCK_SIZE, Size::from_bytes(TEN_MIB_BYTES as u64))?;
        expected_gpt(gpt);
        //
        Ok(())
    }

    /// Don't panic on slice indexing if given an empty slice
    #[test]
    // FIXME: from_bytes_with_size takes `primary` and `alt`,
    // and panics if they're not block_size.
    #[ignore]
    fn empty_bytes_regress() {
        let raw = &[];
        let gpt = Gpt::<U128>::from_bytes_with_size(
            raw,
            raw,
            |i, buf| {
                let i = i.0 as usize;
                let size = buf.len();
                buf.copy_from_slice(&raw[i..][..size]);
                Ok(())
            },
            BLOCK_SIZE,
            Size::from_bytes(TEN_MIB_BYTES as u64),
        );
        let e = gpt.unwrap_err();
        if let Error::NotEnough = e {
            panic!("Wrong error");
        }
    }

    /// Make sure that if a `Gpt<U0>` is written to a device with 1 partition,
    /// that the partition destroyed/ignored.
    /// Basically it should output a valid GPT Header
    #[test]
    fn destroy_unsupported_partitions() -> Result {
        let mut raw = data()?;
        let zero_gpt = read_gpt_size::<U0>(&raw)?;
        zero_gpt
            .to_bytes_with_size(
                |i, buf| {
                    let i = i.0 as usize;
                    raw[i..][..buf.len()].copy_from_slice(buf);
                    Ok(())
                },
                BLOCK_SIZE,
                Size::from_bytes(TEN_MIB_BYTES as u64),
            )
            .map_err(anyhow::Error::msg)?;
        let gpt = read_gpt_size::<U128>(&raw)?;
        assert_eq!(gpt.partitions().len(), 0);
        //
        Ok(())
    }

    /// Test that add_partition actually works
    ///
    /// Was first implemented using `partitions_mut` so index would
    /// always be out of bounds. Oops.
    #[test]
    fn add_partition_regress() -> Result {
        let raw = data()?;
        let mut gpt = read_gpt_size::<U128>(&raw)?;
        let part = gpt.partitions()[0];
        // Just test that it doesn't panic
        let _ = gpt.add_partition(part);
        Ok(())
    }

    /// Prevent adding overlapping partitions
    #[test]
    #[should_panic(expected = "Attempted to add overlapping partitions")]
    fn invalid_partitions() {
        let raw = data().unwrap();
        let mut gpt = read_gpt_size::<U128>(&raw).unwrap();
        let part = gpt.partitions()[0];
        let e = gpt.add_partition(part).unwrap_err();
        panic!(e.to_string());
    }

    /// Prevent adding invalid partitions, outside the usable lba range.
    #[test]
    #[ignore]
    #[should_panic(expected = "Attempted to add overlapping partitions")]
    fn invalid_range_partitions() {
        let raw = data().unwrap();
        let mut gpt = read_gpt_size::<U128>(&raw).unwrap();
        let part = PartitionBuilder::new(Uuid::new_v4())
            .start(Size::from_mib(1).into())
            .size(Size::from_mib(1))
            .finish(BLOCK_SIZE);
        let e = gpt.add_partition(part).unwrap_err();
        panic!(e.to_string());
    }

    /// Create a GPT label more-or-less identical to our test data
    #[test]
    fn create_test_parts() -> Result {
        let test_data = data()?;
        let test_gpt = read_gpt_size::<U128>(&test_data)?;
        //
        let mut gpt: Gpt<U128> = Gpt::new();
        gpt.set_uuid(Uuid::parse_str(CF_DISK_GUID)?);
        let part = PartitionBuilder::new(Uuid::parse_str(CF_PART_GUID)?)
            .start(Size::from_mib(1).into())
            .size(Size::from_mib(8))
            .partition_type(PartitionType::LinuxFilesystemData);
        gpt.add_partition(part.finish(BLOCK_SIZE))?;
        assert_eq!(gpt, test_gpt);
        Ok(())
    }

    /// Test that a newly created Gpt has no partitions
    #[test]
    fn empty_partitions() -> Result {
        let mut data = vec![0; TEN_MIB_BYTES];
        let size = Size::from_bytes(TEN_MIB_BYTES as u64);
        let gpt = Gpt::new();
        gpt.to_bytes(&mut data, BLOCK_SIZE, size)?;
        let gpt = Gpt::from_bytes(&data, BLOCK_SIZE, size)?;
        assert_eq!(gpt.partitions().len(), 0);
        //
        Ok(())
    }

    /// Test that a newly created Gpt has no partitions
    #[test]
    fn empty_partitions_std() -> Result {
        let data = vec![0; TEN_MIB_BYTES];
        let mut data = io::Cursor::new(data);
        let size = Size::from_bytes(TEN_MIB_BYTES as u64);
        let gpt = Gpt::new();
        gpt.to_writer(&mut data, BLOCK_SIZE, size)?;
        let gpt = Gpt::from_reader(&mut data, BLOCK_SIZE, size)?;
        assert_eq!(gpt.partitions().len(), 0);
        //
        Ok(())
    }
}
