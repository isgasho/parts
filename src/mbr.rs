//! MBR definitions
use crate::{gpt::error::*, types::*};
use core::convert::{TryFrom, TryInto};

/// Hard-coded legacy MBR size.
pub const MBR_SIZE: usize = 512;

/// Legacy MBR boot code.
///
/// Required because a bare array wouldn't be `Copy`.
#[derive(Copy, Clone)]
#[repr(transparent)]
struct BootCode([u8; 440]);

impl PartialEq for BootCode {
    fn eq(&self, other: &Self) -> bool {
        self.0[..] == other.0[..]
    }
}

impl Default for BootCode {
    fn default() -> Self {
        BootCode([0; 440])
    }
}

/// GPT Protective MBR
#[cfg_attr(test, derive(PartialEq))]
#[repr(C)]
pub struct ProtectiveMbr {
    /// Bios boot code. Unused by GPT.
    boot_code: BootCode,

    /// A unique signature. Unused by GPT.
    /// Hard-coded to 0.
    unique_signature: [u8; 4],

    /// Hard-coded to 0.
    unknown: [u8; 2],

    /// Hard-coded to one partition, covering the entire device.
    partitions: [MbrPart; 4],

    /// Hard-coded to 0xAA55-LE.
    // Array instead of u16 for alignment purposes
    signature: [u8; 2],
}

// Crate public
impl ProtectiveMbr {
    /// Creates a new Protective MBR
    ///
    /// `last_lba`, the last usable logical block address on the device.
    pub fn new(last_lba: Block) -> Self {
        Self {
            boot_code: Default::default(),
            unique_signature: [0u8; 4],
            unknown: [0u8; 2],
            partitions: [
                MbrPart {
                    boot: 0,
                    start_chs: 0x000200u32.to_le_bytes()[..3].try_into().unwrap(),
                    os_type: 0xEE,
                    // Technically incorrect?, but
                    // Existing implementations seem to do the same thing here.
                    end_chs: 0xFFFFFFu32.to_le_bytes()[..3].try_into().unwrap(),
                    start_lba: 1u32.to_le_bytes(),
                    size_lba: u32::try_from(last_lba.0)
                        .unwrap_or(u32::max_value())
                        .to_le_bytes(),
                },
                MbrPart::default(),
                MbrPart::default(),
                MbrPart::default(),
            ],
            signature: 0xAA55u16.to_le_bytes(),
        }
    }

    /// Zero-copy `ProtectiveMbr` from byte slice.
    ///
    /// # Panics
    ///
    /// - If `source` is not [`MBR_SIZE`].
    ///
    /// # Errors
    ///
    /// - [`Error::Mbr`] If the MBR is invalid
    pub fn from_bytes(source: &[u8]) -> Result<&Self> {
        assert_eq!(source.len(), MBR_SIZE, "BUG: Source must be MBR_SIZE bytes");
        // SAFETY:
        // - `ProtectiveMbr` has alignment of 1.
        // - `size_of::<ProtectiveMbr>` is MBR_SIZE.
        // - `source` is valid for `MBR_SIZE`
        let mbr = unsafe { &*(source.as_ptr() as *const ProtectiveMbr) };
        mbr.validate()?;
        Ok(mbr)
    }

    /// Write a GPT Protective MBR to `dest`
    ///
    /// # Panics
    ///
    /// - If `dest` is not [`MBR_SIZE`] bytes.
    pub fn to_bytes(&self, dest: &mut [u8]) {
        assert_eq!(dest.len(), MBR_SIZE, "BUG: Dest must be MBR_SIZE bytes");
        // SAFETY:
        // - `self` is valid and aligned.
        // - `size_of::<ProtectiveMbr>` is MBR_SIZE.
        let raw = unsafe {
            let ptr = self as *const ProtectiveMbr as *const u8;
            core::slice::from_raw_parts(ptr, MBR_SIZE)
        };
        dest.copy_from_slice(raw);
    }
}

// Private
impl ProtectiveMbr {
    /// Validate the Protective MBR.
    ///
    /// # Errors
    ///
    /// - [`Error::Mbr`] The MBR is considered invalid if any of it's fields
    ///   differ from expectations.
    fn validate(&self) -> Result<()> {
        if self.unique_signature != [0; 4] {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        if self.unknown != [0; 2] {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        let part: &MbrPart = &self.partitions[0];
        if part.boot != 0 {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        if part.start_chs != 0x000200u32.to_le_bytes()[..3] {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        if part.os_type != 0xEE {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        if part.end_chs != 0xFFFFFFu32.to_le_bytes()[..3] {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        if part.start_lba != 1u32.to_le_bytes() {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        // TODO: Actual size check
        if part.size_lba == 0u32.to_le_bytes() {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }
        for part in &self.partitions[1..] {
            if *part != MbrPart::default() {
                return Err(Error::Mbr("Invalid Protective MBR"));
            }
        }
        if self.signature != 0xAA55u16.to_le_bytes() {
            return Err(Error::Mbr("Invalid Protective MBR"));
        }

        Ok(())
    }
}

/// Smaller Debug output.
impl core::fmt::Debug for ProtectiveMbr {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        fmt.debug_struct("ProtectiveMbr")
            .field("partition 0", &{ self.partitions[0] })
            .finish()
    }
}

#[derive(Debug, Default, PartialEq, Copy, Clone)]
#[repr(C)]
struct MbrPart {
    /// Whether the partition is "bootable". Unused by GPT.
    /// Hard-coded to 0.
    boot: u8,

    /// Cylinder, Head, Sector. Unused by GPT.
    /// Hard-coded to 0x000200.
    start_chs: [u8; 3],

    /// Hard-coded to 0xEE, GPT Protective.
    os_type: u8,

    /// Cylinder, Head, Sector. Unused by GPT.
    /// De facto Hard-coded to 0xFFFFFF.
    end_chs: [u8; 3],

    /// Hard-coded to 1, the start of the GPT Header.
    // Array instead of u32 for alignment purposes, without having to be
    // `repr(packed)`.
    start_lba: [u8; 4],

    /// Size of the disk, in LBA, minus one, or 0xFFFFFFFF if the size is too
    /// large.
    // Array instead of u32 for alignment purposes, without having to be
    // `repr(packed)`.
    size_lba: [u8; 4],
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::util::Result;
    use static_assertions::*;

    assert_eq_size!(MbrPart, [u8; 16]);
    assert_eq_size!(ProtectiveMbr, [u8; MBR_SIZE]);
    assert_eq_align!(ProtectiveMbr, MbrPart, u8);
    assert_eq_align!(MbrPart, u8);
    assert_eq_align!(BootCode, u8);

    static DATA: &[u8] = include_bytes!("../tests/data/test_parts_cf");

    /// Basic reading should work and validate correctly.
    #[test]
    fn read_test() -> Result {
        let _mbr = ProtectiveMbr::from_bytes(&DATA[..MBR_SIZE])?;
        Ok(())
    }

    /// A correct Protective MBR can be read and written out without changing.
    #[test]
    fn roundtrip() -> Result {
        let source_mbr = &DATA[..MBR_SIZE];
        let parsed_source_mbr = ProtectiveMbr::from_bytes(source_mbr)?;
        let mut raw_my_mbr = [0u8; MBR_SIZE];
        parsed_source_mbr.to_bytes(&mut raw_my_mbr);

        assert_eq!(
            &raw_my_mbr[..],
            &source_mbr[..],
            "Written MBR did not match read MBR"
        );
        let parsed_raw_my_mbr = ProtectiveMbr::from_bytes(&raw_my_mbr)?;
        assert_eq!(
            parsed_raw_my_mbr, parsed_source_mbr,
            "MBR was not read the same as it was written?"
        );

        Ok(())
    }
}
