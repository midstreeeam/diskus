use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
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

#[derive(Debug)]
pub enum Error {
    NoMetadataForPath(PathBuf),
    CouldNotReadDir(PathBuf),
}

/// The result of a disk usage computation.
#[derive(Debug)]
pub struct DiskUsageResult {
    size_in_bytes: u64,
    errors: Vec<Error>,
}

impl DiskUsageResult {
    /// Returns the total size in bytes, or the errors if any occurred.
    pub fn size_in_bytes(&self) -> Result<u64, &[Error]> {
        if self.errors.is_empty() {
            Ok(self.size_in_bytes)
        } else {
            Err(&self.errors)
        }
    }

    /// Ignore any errors and return a result that provides direct access to the size.
    pub fn ignore_errors(&self) -> UncheckedDiskUsageResult {
        UncheckedDiskUsageResult {
            size_in_bytes: self.size_in_bytes,
        }
    }

    /// Returns any errors encountered during traversal.
    pub fn errors(&self) -> &[Error] {
        &self.errors
    }

    /// Returns `true` if no errors occurred during traversal.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// The size result for one filesystem entry.
#[derive(Debug)]
pub struct DiskUsageEntry {
    path: PathBuf,
    result: DiskUsageResult,
}

impl DiskUsageEntry {
    /// Returns the path this result belongs to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the disk usage result for this path.
    pub fn result(&self) -> &DiskUsageResult {
        &self.result
    }
}

/// The result of a direct-entry disk usage computation.
#[derive(Debug)]
pub struct DiskUsageEntriesResult {
    entries: Vec<DiskUsageEntry>,
    errors: Vec<Error>,
}

impl DiskUsageEntriesResult {
    /// Returns the results for direct entries below the requested root paths.
    pub fn entries(&self) -> &[DiskUsageEntry] {
        &self.entries
    }

    /// Returns errors encountered while resolving or reading the requested root paths.
    pub fn errors(&self) -> &[Error] {
        &self.errors
    }

    /// Returns `true` if no root or entry errors occurred during traversal.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty() && self.entries.iter().all(|entry| entry.result.is_ok())
    }
}

/// A disk usage result where errors have been explicitly ignored.
#[derive(Debug)]
pub struct UncheckedDiskUsageResult {
    size_in_bytes: u64,
}

impl UncheckedDiskUsageResult {
    /// Returns the total size in bytes.
    pub fn size_in_bytes(&self) -> u64 {
        self.size_in_bytes
    }
}

enum Message {
    SizeEntry(Option<UniqueID>, u64),
    Error { error: Error },
}

enum EntryMessage {
    SizeEntry {
        index: usize,
        unique_id: Option<UniqueID>,
        size: u64,
    },
    Error {
        index: usize,
        error: Error,
    },
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

fn walk_entry(
    tx: channel::Sender<EntryMessage>,
    index: usize,
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
                Directories::Auto => !is_dir || filesize_type == CountType::DiskUsage,
            };

            if should_count {
                let unique_id = generate_unique_id(&metadata);
                let size = filesize_type.size(&metadata);
                tx_ref
                    .send(EntryMessage::SizeEntry {
                        index,
                        unique_id,
                        size,
                    })
                    .unwrap();
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
                            .send(EntryMessage::Error {
                                index,
                                error: Error::CouldNotReadDir(entry.clone()),
                            })
                            .unwrap();
                    }
                }

                walk_entry(
                    tx_ref.clone(),
                    index,
                    &children[..],
                    filesize_type,
                    directories,
                );
            };
        } else {
            tx_ref
                .send(EntryMessage::Error {
                    index,
                    error: Error::NoMetadataForPath(entry.clone()),
                })
                .unwrap();
        };
    });
}

/// Configure and run a parallel directory walk to compute file system usage.
pub struct DiskUsage {
    root_directories: Vec<PathBuf>,
    num_workers: usize,
    count_type: CountType,
    directories: Directories,
}

impl DiskUsage {
    /// Create a new DiskUsage builder for the given root directories.
    pub fn new<P: AsRef<Path>>(root_directories: impl IntoIterator<Item = P>) -> DiskUsage {
        DiskUsage {
            root_directories: root_directories
                .into_iter()
                .map(|p| p.as_ref().to_path_buf())
                .collect(),
            num_workers: Self::default_num_workers(),
            count_type: CountType::default(),
            directories: Directories::default(),
        }
    }

    /// Returns the default number of workers (3 × number of CPU cores).
    ///
    /// This is a good tradeoff between cold-cache and warm-cache performance.
    /// For cold disk caches, more threads help the IO scheduler plan ahead.
    /// For warm caches, too many threads add synchronization overhead.
    ///
    /// <https://github.com/sharkdp/diskus/issues/38#issuecomment-612772867>
    fn default_num_workers() -> usize {
        3 * std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            // To limit startup overhead on massively parallel machines, don't use more than 64 threads
            .min(64)
    }

    /// Set the number of workers to use for parallel traversal.
    ///
    /// By default, this is set to three times the number of CPU cores, which
    /// results in a good performance tradeoff for both cold and warm disk caches.
    pub fn num_workers(mut self, num_workers: usize) -> Self {
        self.num_workers = num_workers;
        self
    }

    /// Specify the type of the count (disk usage or apparent size).
    pub fn count_type(mut self, count_type: CountType) -> Self {
        self.count_type = count_type;
        self
    }

    /// Configure the count to use apparent size instead of disk usage.
    pub fn apparent_size(mut self) -> Self {
        self.count_type = CountType::ApparentSize;
        self
    }

    /// Set whether to count directory sizes.
    pub fn directories(mut self, directories: Directories) -> Self {
        self.directories = directories;
        self
    }

    /// Run the count and return the result.
    pub fn count(&self) -> DiskUsageResult {
        let (size_in_bytes, errors) = self.count_inner();
        DiskUsageResult {
            size_in_bytes,
            errors,
        }
    }

    /// Run the count and return only the size, ignoring any errors.
    pub fn count_ignoring_errors(&self) -> u64 {
        self.count_inner().0
    }

    /// Run the count for each direct child of the configured root directories.
    ///
    /// This only changes which entries are reported: each direct child is still
    /// traversed recursively so directories report their full size. The regular
    /// [`DiskUsage::count`] path is separate and keeps its original fast total-only
    /// aggregation.
    pub fn count_direct_children(&self) -> DiskUsageEntriesResult {
        let (paths, errors) = self.direct_child_paths();
        let results = self.count_entries(paths);
        DiskUsageEntriesResult {
            entries: results,
            errors,
        }
    }

    fn direct_child_paths(&self) -> (Vec<PathBuf>, Vec<Error>) {
        let mut paths = Vec::new();
        let mut errors = Vec::new();

        for root in &self.root_directories {
            match root.symlink_metadata() {
                Ok(metadata) if metadata.is_dir() => match fs::read_dir(root) {
                    Ok(child_entries) => {
                        for child_entry in child_entries.flatten() {
                            paths.push(child_entry.path());
                        }
                    }
                    Err(_) => errors.push(Error::CouldNotReadDir(root.clone())),
                },
                Ok(_) => paths.push(root.clone()),
                Err(_) => errors.push(Error::NoMetadataForPath(root.clone())),
            }
        }

        (paths, errors)
    }

    fn count_entries(&self, paths: Vec<PathBuf>) -> Vec<DiskUsageEntry> {
        if paths.is_empty() {
            return Vec::new();
        }

        let (tx, rx) = channel::unbounded();
        let entry_count = paths.len();

        let receiver_thread = thread::spawn(move || {
            let mut totals = vec![0; entry_count];
            let mut errors: Vec<Vec<Error>> = (0..entry_count).map(|_| Vec::new()).collect();
            let mut ids = HashSet::new();

            for msg in rx {
                match msg {
                    EntryMessage::SizeEntry {
                        index,
                        unique_id,
                        size,
                    } => {
                        if let Some(unique_id) = unique_id {
                            if ids.insert(unique_id) {
                                totals[index] += size;
                            }
                        } else {
                            totals[index] += size;
                        }
                    }
                    EntryMessage::Error { index, error } => {
                        errors[index].push(error);
                    }
                }
            }

            (totals, errors)
        });

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.num_workers)
            .build()
            .unwrap();
        pool.install(|| {
            paths
                .par_iter()
                .enumerate()
                .for_each_with(tx, |tx_ref, (index, path)| {
                    walk_entry(
                        tx_ref.clone(),
                        index,
                        std::slice::from_ref(path),
                        self.count_type,
                        self.directories,
                    );
                })
        });

        let (totals, errors) = receiver_thread.join().unwrap();

        paths
            .into_iter()
            .zip(totals)
            .zip(errors)
            .map(|((path, size_in_bytes), errors)| DiskUsageEntry {
                path,
                result: DiskUsageResult {
                    size_in_bytes,
                    errors,
                },
            })
            .collect()
    }

    fn count_inner(&self) -> (u64, Vec<Error>) {
        let (tx, rx) = channel::unbounded();

        let receiver_thread = thread::spawn(move || {
            let mut total = 0;
            let mut ids = HashSet::new();
            let mut errors: Vec<Error> = Vec::new();
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
                        errors.push(error);
                    }
                }
            }
            (total, errors)
        });

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.num_workers)
            .build()
            .unwrap();
        pool.install(|| {
            walk(
                tx,
                &self.root_directories,
                self.count_type,
                self.directories,
            )
        });

        receiver_thread.join().unwrap()
    }
}
