//! # Basic usage
//!
//! ```
//! use std::path::PathBuf;
//! use diskus::{DiskUsage, CountType};
//!
//! let result = DiskUsage::new(&["."])
//!     .num_workers(4)
//!     .count();
//! let size_in_bytes = result.ignore_errors().size_in_bytes();
//! ```

mod filesize;
mod unique_id;
pub mod walk;

pub use crate::filesize::CountType;
pub use crate::walk::{Directories, DiskUsage, DiskUsageResult, Error};
