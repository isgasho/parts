//! GPT Header stuff
use core::convert::TryFrom;
use crc::{crc32, Hasher32};
use displaydoc::Display;
use thiserror::Error;
use uuid::Uuid;

/// Invalid MBR: {0}
#[derive(Debug, Display, Error)]
pub struct Invalid(&'static str);

// type Result<T> = core::result::Result<T, InvalidMbr>;

/// Hard coded EFI magic
const SIGNATURE: [u8; 8] = *b"EFI PART";

/// Current/supported GPT Header revision
const _REVISION: u32 = 0x0001_0000;

/// Minimum header size. Anything smaller than this is invalid.
pub const MIN_HEADER_SIZE: u32 = 92;

/// Start of the GPT Header Structure
///
/// This is only the first 3 fields,
/// used to determine what kind of header to actually parse.
#[repr(C, packed)]
pub struct HeaderPre {
    /// Hard-coded to [`SIGNATURE`]
    signature: u64,

    /// Hard-coded to [`REVISION`]
    revision: u32,

    /// Soft-coded to [`MIN_HEADER_SIZE`]
    ///
    /// Must be header_size >= [`MIN_HEADER_SIZE`] and header_size <=
    /// logical block size.
    header_size: u32,
}

impl HeaderPre {
    pub fn read(source: &[u8], block_size: u64) -> &Self {
        assert!(
            source.len() >= MIN_HEADER_SIZE as usize
                && source.len()
                    <= usize::try_from(block_size)
                        .expect("Block size *way* too large, would overflow usize"),
            "BUG: Source must be between MIN_HEADER_SIZE and block_size bytes"
        );
        // SAFETY:
        // - `HeaderPre` has alignment of 1.
        // - `size_of::<HeaderPre>` is 16.
        // - `source` is valid for `16`.
        unsafe { &*(source.as_ptr() as *const HeaderPre) }
    }
}

/// GPT Header, revision 1.0
#[repr(C, packed)]
pub struct Header {
    /// Hard-coded to [`SIGNATURE`]
    signature: [u8; 8],

    /// Hard-coded to [`REVISION`]
    revision: u32,

    /// Soft-coded to [`MIN_HEADER_SIZE`]
    ///
    /// Must be header_size >= [`MIN_HEADER_SIZE`] and header_size <=
    /// logical block size.
    header_size: u32,

    /// CRC32(bytes[0..header_size])
    ///
    /// Set to zero when computing CRC
    header_crc32: u32,

    /// Must be zero
    _reserved: u32,

    /// The logical block address we reside in
    this_lba: u64,

    /// The logical block address the backup header is in
    alt_lba: u64,

    /// Where partitions can start
    first_usable_lba: u64,

    /// Where partitions must end
    last_usable_lba: u64,

    /// Disk GUID.
    disk_guid: Uuid,

    /// Where our partition array starts on disk.
    partition_array_start: u64,

    /// Number of partitions
    partitions: u32,

    /// Size of each partition entry structure.
    /// Must be 128 * 2^n, where n >= 0
    partition_size: u32,

    /// CRC32 of the partition array
    partitions_crc32: u32,
}

impl Header {
    /// Read the GPT Header from `source`,
    /// located at the logical block address `lba`.
    ///
    /// # Panics
    ///
    /// - if `source` is not between [`MIN_HEADER_SIZE`] and `block_size`,
    ///   inclusive.
    ///
    /// # Errors
    ///
    /// - If the header is invalid. See [`Header::validate`] for details.
    pub fn read(source: &[u8], block_size: u64, lba: u64) -> &Self {
        assert!(
            source.len() >= MIN_HEADER_SIZE as usize
                && source.len()
                    <= usize::try_from(block_size)
                        .expect("Block size *way* too large, would overflow usize"),
            "BUG: Source must be between MIN_HEADER_SIZE and block_size bytes"
        );
        // SAFETY:
        // - `Header` has alignment of 1.
        // - `Header` is `repr(C)`.
        // - `size_of::<Header>` is `MIN_HEADER_SIZE`.
        // - `source` is valid for `MIN_HEADER_SIZE`.
        let header = unsafe { &*(source.as_ptr() as *const Header) };
        header.validate(source, lba);
        header
    }

    /// UEFI UUID's are stored in a mixed endian format,
    /// so this will correct it.
    pub fn uuid(&self) -> Uuid {
        Uuid::from_bytes_me(self.disk_guid.to_bytes())
    }
}

impl Header {
    /// Validate the header.
    ///
    /// See Section 5.3.2 of the UEFI specification for exact details.
    ///
    /// # Validity
    ///
    /// The header is considered invalid if:
    ///
    /// - The signature is invalid
    /// - The header CRC is invalid
    /// - [`Header::this_lba`] is equal to `this_lba`.
    /// - The partition array CRC is invalid
    fn validate(&self, source: &[u8], this_lba: u64) {
        if self.signature != SIGNATURE {
            panic!("Invalid Signature");
        }
        // Up to `self.header_size` bytes
        let source =
            &source[..usize::try_from(self.header_size).expect("Header size would overflow usize")];
        let mut digest = crc32::Digest::new(crc32::IEEE);
        // Header bytes up to the CRC field, which starts at offset 16.
        digest.write(&source[..16]);
        // Write 4 zeros for the CRC.
        digest.write(&0u32.to_ne_bytes());
        // Continue for the rest of the slice.
        digest.write(&source[20..]);
        let crc = digest.sum32();
        if self.header_crc32 != crc {
            let e = self.header_crc32;
            panic!("Invalid CRC: Got {}, expected {}", crc, e);
        }
        if self.this_lba != this_lba {
            panic!("Invalid this_lba");
        }
        // TODO: Partition array
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::{
            mbr::MBR_SIZE,
            util::{Result, TEST_DATA},
        },
        *,
    };
    use static_assertions::*;

    assert_eq_size!(HeaderPre, [u8; 16]);
    assert_eq_align!(HeaderPre, u8);

    /// Basic reading should work and validate correctly.
    #[test]
    fn read_test() -> Result<()> {
        for data in TEST_DATA {
            // Skip the MBR, limit to block size.
            let bytes = &data.bytes[MBR_SIZE..][..data.block_size as usize];
            let header = Header::read(bytes, data.block_size, 1);
            assert_eq!(
                header.uuid(),
                Uuid::parse(data.disk).unwrap(),
                "UUID didn't match test data"
            );
            assert_eq!(header.uuid().variant(), uuid::Variant::Rfc4122);
        }
        Ok(())
    }
}
