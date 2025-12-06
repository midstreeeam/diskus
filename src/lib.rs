//! # Basic usage
//!
//! ```
//! use std::path::PathBuf;
//! use diskus::{DiskUsage, CountType};
//!
//! let paths = vec![PathBuf::from(".")];
//! let (size_in_bytes, errors) = DiskUsage::new(&paths)
//!     .num_workers(4)
//!     .count_type(CountType::DiskUsage)
//!     .count();
//! ```

mod filesize;
mod unique_id;
pub mod walk;

pub use crate::filesize::CountType;
pub use crate::walk::{Directories, DiskUsage, Error};
