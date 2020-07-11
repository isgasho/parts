//! Test utility stuff

/// cfdisk test data
static TEST_PARTS_CF: &[u8] = include_bytes!("../../tests/data/test_parts_cf");

/// gnu parted test data
static TEST_PARTS: &[u8] = include_bytes!("../../tests/data/test_parts");

/// Test data structure
pub struct Data {
    pub bytes: &'static [u8],
    pub block_size: u64,
    pub disk: &'static str,
    pub part: &'static str,
}

impl Data {
    const fn new(
        bytes: &'static [u8],
        block_size: u64,
        disk: &'static str,
        part: &'static str,
    ) -> Self {
        Self {
            bytes,
            block_size,
            disk,
            part,
        }
    }
}

/// Test data
pub static TEST_DATA: &[Data] = &[
    //
    Data::new(
        TEST_PARTS_CF,
        512,
        "A17875FB-1D86-EE4D-8DFE-E3E8ABBCD364",
        "97954376-2BB6-534B-A015-DF434A94ABA2",
    ),
    Data::new(
        TEST_PARTS,
        512,
        "062946B9-3113-4CC0-98DD-94649773E536",
        "F3099835-0F4A-4D49-B012-7078CF1B4045",
    ),
];

/// Result type.
pub type Result<T = ()> = core::result::Result<T, ()>;
