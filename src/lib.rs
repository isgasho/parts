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
//! # use std::fs::File;
//! # use std::error::Error;
//!
//! # fn main() -> Result<(), Box<dyn Error>> {
//! # let image = File::open("tests/data/test_parts_cf")?;
//! # let disk_size = ByteSize::from_bytes(10_485_760);
//! let mut gpt = Gpt::from_reader(image, BlockSize(512), disk_size)?;
//! for part in gpt.partitions() {
//!     println!("Partition UUID: {}", part.uuid());
//! }
//! #
//! # Ok(())
//! # }
//! ```
#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![deny(missing_docs)]
#![cfg_attr(feature = "nightly", feature(doc_cfg))]
#![allow(dead_code)]

#[cfg(test)]
mod util;

mod gpt;
mod mbr;
mod partitions;
pub mod types;

pub use gpt::*;
pub use partitions::*;

// Re-exports
#[doc(inline)]
pub use generic_array::typenum;
#[doc(inline)]
pub use uuid;
