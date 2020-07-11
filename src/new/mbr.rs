//! Legacy Master Boot Record(MBR)
use core::convert::TryFrom;
use displaydoc::Display;
use thiserror::Error;

/// Invalid MBR: {0}
#[derive(Debug, Display, Error)]
pub struct InvalidMbr(&'static str);

type Result<T> = core::result::Result<T, InvalidMbr>;

/// Hard-coded legacy MBR size.
pub const MBR_SIZE: usize = 512;

/// GPT Protective OS Type
const GPT_PROTECTIVE: u8 = 0xEE;

/// Starting MBR CHS
// 0x000200u32
const START_CHS: [u8; 3] = [0, 2, 0];

/// Ending MBR CHS
// Technically incorrect, maybe?
// Existing implementations seem to do the same thing here, though?
// 0xFFFFFFu32
const END_CHS: [u8; 3] = [255, 255, 255];

/// Signature
// 0xAA55u16
const SIGNATURE: [u8; 2] = [85, 170];

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
        for part in &mbr.partitions[1..] {
            if part != &MbrPart::default() {
                return Err(InvalidMbr(
                    "Protective MBR has non-empty partitions. Probably not GPT formatted.",
                ));
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

    // TODO: Roundtrip test
}
