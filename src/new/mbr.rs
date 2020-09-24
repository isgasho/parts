//! Legacy Master Boot Record(MBR)
use core::convert::TryFrom;
use displaydoc::Display;
use thiserror::Error;

/// Invalid MBR
#[derive(Debug, Display, Error)]
pub enum Error {
    /// unsupported legacy MBR
    NotProtective,

    /// unsupported legacy MBR with UEFI system partition
    Unsupported,

    /// MBR had unexpected or invalid values
    Corrupt(&'static str),
}

type Result<T> = core::result::Result<T, Error>;

/// Hard-coded legacy MBR size.
pub const MBR_SIZE: usize = 512;

/// GPT Protective OS Type
const GPT_PROTECTIVE: u8 = 0xEE;

/// UEFI System Partition OS Type
const UEFI_SYSTEM: u8 = 0xEF;

/// Starting MBR CHS
// 0x000200u32
const START_CHS: [u8; 3] = [0, 2, 0];

/// Ending MBR CHS
// Technically incorrect, maybe?
// Existing implementations seem to do the same thing here, though?
// 0xFFFFFFu32
const END_CHS: [u8; 3] = [0xFF, 0xFF, 0xFF];

/// Signature, 0xAA55u16.
const SIGNATURE: [u8; 2] = [0x55, 0xAA];

/// Legacy MBR boot code.
///
/// Required because a bare array doesn't implement traits we need.
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

impl core::fmt::Debug for BootCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BootCode").finish()
    }
}

/// GPT Protective MBR
#[cfg_attr(test, derive(Debug, PartialEq))]
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

impl ProtectiveMbr {
    /// Creates a new Protective MBR
    ///
    /// `last_lba`, the last usable logical block address on the device.
    pub fn new(last_lba: u64) -> Self {
        Self {
            boot_code: Default::default(),
            unique_signature: [0u8; 4],
            unknown: [0u8; 2],
            partitions: [
                MbrPart {
                    bootable: 0,
                    start_chs: START_CHS,
                    os_type: GPT_PROTECTIVE,
                    end_chs: END_CHS,
                    start_lba: 1u32.to_le_bytes(),
                    size_lba: u32::try_from(last_lba)
                        .unwrap_or(u32::max_value())
                        .to_le_bytes(),
                },
                MbrPart::default(),
                MbrPart::default(),
                MbrPart::default(),
            ],
            signature: SIGNATURE,
        }
    }

    /// Read the MBR from `source`.
    ///
    /// # Errors
    ///
    /// - If the MBR has any other partitions.
    ///
    /// # Panics
    ///
    /// - If `source` is not [`MBR_SIZE`] bytes.
    pub fn read(source: &[u8], _block_size: u64) -> Result<&Self> {
        assert_eq!(source.len(), MBR_SIZE, "BUG: Source must be MBR_SIZE bytes");
        // SAFETY:
        // - `ProtectiveMbr` has alignment of 1.
        // - `size_of::<ProtectiveMbr>` is `MBR_SIZE`.
        // - `source` is valid for `MBR_SIZE`.
        let mbr = unsafe { &*(source.as_ptr() as *const ProtectiveMbr) };
        // NOTE: Too strict, should ignore? Spec says gotta check?
        // Unclear if it means only for `UEFI_SYSTEM` types.
        if mbr.signature != SIGNATURE {
            return Err(Error::Corrupt("invalid signature"));
        }
        for part in &mbr.partitions {
            match part.os_type {
                UEFI_SYSTEM => return Err(Error::Unsupported),
                GPT_PROTECTIVE => (),
                0 => (),
                _ => return Err(Error::NotProtective),
            }
        }
        for part in &mbr.partitions[1..] {
            if part != &MbrPart::default() {
                return Err(Error::NotProtective);
            }
        }
        Ok(mbr)
    }

    /// Write a GPT Protective MBR to `dest`
    ///
    /// # Panics
    ///
    /// - If `dest` is not [`MBR_SIZE`] bytes.
    pub fn write(&self, dest: &mut [u8], _block_size: u64) {
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

/// Protective MBR Partition
#[derive(Debug, Default, PartialEq, Copy, Clone)]
#[repr(C)]
struct MbrPart {
    /// Whether the partition is "bootable". Unused by GPT.
    ///
    /// Hard-coded to 0.
    bootable: u8,

    /// Cylinder, Head, Sector. Unused by GPT.
    ///
    /// Hard-coded to 0x000200.
    start_chs: [u8; 3],

    /// Hard-coded to 0xEE, GPT Protective.
    os_type: u8,

    /// Cylinder, Head, Sector. Unused by GPT.
    ///
    /// De-facto Hard-coded to 0xFFFFFF.
    end_chs: [u8; 3],

    /// Hard-coded to 1, the start of the GPT Header.
    ///
    /// Array instead of u32 for alignment purposes, without having to be
    /// `repr(packed)`.
    start_lba: [u8; 4],

    /// Size of the disk, in LBA, minus one, or 0xFFFFFFFF if the size is too
    /// large.
    ///
    /// Array instead of u32 for alignment purposes, without having to be
    /// `repr(packed)`.
    size_lba: [u8; 4],
}

#[cfg(test)]
mod tests {
    use super::{super::util::TEST_DATA, *};
    use static_assertions::*;

    assert_eq_size!(MbrPart, [u8; 16]);
    assert_eq_size!(ProtectiveMbr, [u8; MBR_SIZE]);
    assert_eq_align!(ProtectiveMbr, MbrPart, BootCode, u8);

    /// Basic reading should work and validate correctly.
    #[test]
    fn read_test() -> Result<()> {
        for data in TEST_DATA {
            let _mbr = ProtectiveMbr::read(&data.bytes[..MBR_SIZE], data.block_size)?;
        }
        Ok(())
    }

    /// Create protective a MBR that matches existing.
    #[test]
    fn new() -> Result<()> {
        for data in TEST_DATA {
            let last_lba = (data.bytes.len() / data.block_size as usize) - 1;
            let new = &ProtectiveMbr::new(last_lba as u64);
            let source_mbr = ProtectiveMbr::read(&data.bytes[..MBR_SIZE], data.block_size)?;
            let part: &MbrPart = &source_mbr.partitions[0];
            // NOTE: GNU Parted is buggy.
            // Remove this when it stops being buggy.
            if part.start_chs == [0, 1, 0] && part.end_chs == [254, 255, 255] {
                continue;
            }
            assert_eq!(new, source_mbr);
        }
        Ok(())
    }

    /// A correct Protective MBR can be read and written out without changes.
    #[test]
    fn roundtrip() -> Result<()> {
        for data in TEST_DATA {
            let source_mbr_bytes = &data.bytes[..MBR_SIZE];
            let source_mbr = ProtectiveMbr::read(source_mbr_bytes, data.block_size)?;
            let mut mbr_bytes = [0u8; MBR_SIZE];
            source_mbr.write(&mut mbr_bytes, data.block_size);
            assert_eq!(
                &mbr_bytes[..],
                &source_mbr_bytes[..],
                "Written MBR did not match read MBR"
            );
            let mbr = ProtectiveMbr::read(&mbr_bytes, 0)?;
            assert_eq!(
                mbr, source_mbr,
                "MBR was not read the same as it was written?"
            );
        }
        Ok(())
    }
}
