//! A Pure Rust library for working with GPT partition tables.
//!
//! The primary type for interacting the a GPT table is [`Gpt`].
//! From here you can add and remove partitions.
//!
//! # Usage
//!
//! List all partitions
//!
//! ```rust
//! # use parts::{Gpt, types::*};
//! # #[cfg(feature = "std")]
//! # use std::fs::File;
//! # #[cfg(feature = "std")]
//! # use std::error::Error;
//!
//! # #[cfg(not(feature = "std"))]
//! fn main() {}
//! # #[cfg(feature = "std")]
//! # fn main() -> Result<(), Box<dyn Error>> {
//! # let image = File::open("tests/data/test_parts_cf")?;
//! # let disk_size = Size::from_bytes(10_485_760);
//! let mut gpt = Gpt::from_reader(image, BlockSize(512), disk_size)?;
//! for part in gpt.partitions() {
//!     println!("Partition Name: {}", part.name());
//!     println!("Partition Type: {}", part.partition_type());
//!     println!("Partition UUID: {}", part.uuid());
//! }
//! #
//! # Ok(())
//! # }
//! ```
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(feature = "nightly", feature(external_doc))]
#![deny(missing_docs)]

#[cfg(test)]
mod util;

mod gpt;
mod mbr;
#[allow(dead_code)]
mod partitions;
pub mod types;

pub use gpt::{
    error::Error,
    partition::{Partition, PartitionBuilder},
    Gpt,
    GptC,
};
pub use partitions::*;

// Re-exports
pub use arrayvec;
#[doc(inline)]
pub use generic_array::typenum;
#[doc(inline)]
pub use uuid;

#[cfg(all(doctest, feature = "nightly"))]
#[doc(include = "../README.md")]
pub struct ReadmeDocTests;
