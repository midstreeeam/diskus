//! # Basic usage
//!
//! ```
//! use std::path::PathBuf;
//! use diskus::{Walk, FilesizeType, Directories};
//!
//! let num_threads = 4;
//! let root_directories = &[PathBuf::from(".")];
//! let walk = Walk::new(root_directories, num_threads, FilesizeType::DiskUsage, Directories::Auto);
//! let (size_in_bytes, errors) = walk.run();
//! ```

mod filesize;
mod unique_id;
pub mod walk;

pub use crate::filesize::FilesizeType;
pub use crate::walk::{Directories, Error, Walk};
