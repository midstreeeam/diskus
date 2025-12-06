use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::thread;

use crossbeam_channel as channel;

use rayon::{self, prelude::*};

use crate::filesize::CountType;
use crate::unique_id::{generate_unique_id, UniqueID};

/// Specifies whether directory sizes should be counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Directories {
    /// Automatically match `du` behavior based on the type of the size being counted:
    /// directories are included for disk usage, but excluded for apparent size.
    #[default]
    Auto,
    /// Count both files and directories (matches `du -s` behavior).
    Included,
    /// Count only files, not directories (matches `du -sb` behavior).
    Excluded,
}

pub enum Error {
    NoMetadataForPath(PathBuf),
    CouldNotReadDir(PathBuf),
}

enum Message {
    SizeEntry(Option<UniqueID>, u64),
    Error { error: Error },
}

fn walk(
    tx: channel::Sender<Message>,
    entries: &[PathBuf],
    filesize_type: CountType,
    directories: Directories,
) {
    entries.into_par_iter().for_each_with(tx, |tx_ref, entry| {
        if let Ok(metadata) = entry.symlink_metadata() {
            let is_dir = metadata.is_dir();

            let should_count = match directories {
                Directories::Included => true,
                Directories::Excluded => !is_dir,
                Directories::Auto => {
                    // Auto mode matches `du` behavior: directories are included for
                    // disk usage but excluded for apparent size.
                    !is_dir || filesize_type == CountType::DiskUsage
                }
            };

            if should_count {
                let unique_id = generate_unique_id(&metadata);
                let size = filesize_type.size(&metadata);
                tx_ref.send(Message::SizeEntry(unique_id, size)).unwrap();
            }

            if is_dir {
                let mut children = vec![];
                match fs::read_dir(entry) {
                    Ok(child_entries) => {
                        for child_entry in child_entries.flatten() {
                            children.push(child_entry.path());
                        }
                    }
                    Err(_) => {
                        tx_ref
                            .send(Message::Error {
                                error: Error::CouldNotReadDir(entry.clone()),
                            })
                            .unwrap();
                    }
                }

                walk(tx_ref.clone(), &children[..], filesize_type, directories);
            };
        } else {
            tx_ref
                .send(Message::Error {
                    error: Error::NoMetadataForPath(entry.clone()),
                })
                .unwrap();
        };
    });
}

/// Configure and run a parallel directory walk to file system usage.
pub struct DiskUsage<'a> {
    root_directories: &'a [PathBuf],
    num_workers: usize,
    count_type: CountType,
    directories: Directories,
}

impl<'a> DiskUsage<'a> {
    /// Create a new DiskUsage builder for the given root directories.
    pub fn new(root_directories: &'a [PathBuf]) -> DiskUsage<'a> {
        DiskUsage {
            root_directories,
            num_workers: 1,
            count_type: CountType::default(),
            directories: Directories::default(),
        }
    }

    /// Set the number of workers to use for parallel traversal.
    pub fn num_workers(mut self, num_workers: usize) -> Self {
        self.num_workers = num_workers;
        self
    }

    /// Specify the type of the count (disk usage or apparent size).
    pub fn count_type(mut self, count_type: CountType) -> Self {
        self.count_type = count_type;
        self
    }

    /// Set whether to count directory sizes.
    pub fn directories(mut self, directories: Directories) -> Self {
        self.directories = directories;
        self
    }

    /// Run the count and return the total size in bytes, and any errors encountered.
    pub fn count(&self) -> (u64, Vec<Error>) {
        let (tx, rx) = channel::unbounded();

        let receiver_thread = thread::spawn(move || {
            let mut total = 0;
            let mut ids = HashSet::new();
            let mut error_messages: Vec<Error> = Vec::new();
            for msg in rx {
                match msg {
                    Message::SizeEntry(unique_id, size) => {
                        if let Some(unique_id) = unique_id {
                            // Only count this entry if the ID has not been seen
                            if ids.insert(unique_id) {
                                total += size;
                            }
                        } else {
                            total += size;
                        }
                    }
                    Message::Error { error } => {
                        error_messages.push(error);
                    }
                }
            }
            (total, error_messages)
        });

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.num_workers)
            .build()
            .unwrap();
        pool.install(|| walk(tx, self.root_directories, self.count_type, self.directories));

        receiver_thread.join().unwrap()
    }
}
