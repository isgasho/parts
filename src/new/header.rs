//! GPT Header stuff
use core::{convert::TryFrom, num::NonZeroU32};

/// Hard coded EFI magic
const SIGNATURE: [u8; 8] = *b"EFI PART";

/// Current/supported GPT Header revision
const REVISION: u32 = 0x0001_0000;

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

#[repr(C, packed)]
pub struct Header {
    /// Hard-coded to [`SIGNATURE`]
    signature: u64,

    /// Hard-coded to [`REVISION`]
    revision: u32,

    /// Soft-coded to [`MIN_HEADER_SIZE`]
    ///
    /// Must be header_size >= [`MIN_HEADER_SIZE`] and header_size <=
    /// logical block size.
    header_size: u32,
    header_crc32: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use static_assertions::*;

    assert_eq_size!(HeaderPre, [u8; 16]);

    assert_eq_align!(HeaderPre, u8);
}
