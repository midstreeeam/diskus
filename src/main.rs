use std::path::PathBuf;

use clap::{builder::PossibleValuesParser, Arg, ArgAction, Command};
use humansize::file_size_opts::{self, FileSizeOpts};
use humansize::FileSize;
use num_format::{Locale, ToFormattedString};

use diskus::{Error, FilesizeType, Walk};

fn print_result(size: u64, errors: &[Error], size_format: &FileSizeOpts, verbose: bool) {
    if verbose {
        for err in errors {
            match err {
                Error::NoMetadataForPath(path) => {
                    eprintln!(
                        "diskus: could not retrieve metadata for path '{}'",
                        path.to_string_lossy()
                    );
                }
                Error::CouldNotReadDir(path) => {
                    eprintln!(
                        "diskus: could not read contents of directory '{}'",
                        path.to_string_lossy()
                    );
                }
            }
        }
    } else if !errors.is_empty() {
        eprintln!(
            "[diskus warning] the results may be tainted. Re-run with -v/--verbose to print all errors."
        );
    }

    if atty::is(atty::Stream::Stdout) {
        println!(
            "{} ({:} bytes)",
            size.file_size(size_format).unwrap(),
            size.to_formatted_string(&Locale::en)
        );
    } else {
        println!("{}", size);
    }
}

fn main() {
    let cmd = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("Compute disk usage for the given filesystem entries")
        .arg(
            Arg::new("path")
                .action(ArgAction::Append)
                .help("List of filesystem paths"),
        )
        .arg(
            Arg::new("threads")
                .long("threads")
                .short('j')
                .value_name("N")
                .help("Set the number of threads (default: 3 x num cores)"),
        )
        .arg(
            Arg::new("size-format")
                .long("size-format")
                .value_name("type")
                .value_parser(PossibleValuesParser::new(["decimal", "binary"]))
                .default_value("decimal")
                .help("Output format for file sizes (decimal: MB, binary: MiB)"),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .action(ArgAction::SetTrue)
                .help("Do not hide filesystem errors"),
        );

    #[cfg(not(windows))]
    let cmd = cmd.arg(
        Arg::new("apparent-size")
            .long("apparent-size")
            .short('b')
            .action(ArgAction::SetTrue)
            .help("Compute apparent size instead of disk usage"),
    );

    let matches = cmd.get_matches();

    // Setting the number of threads to 3x the number of cores is a good tradeoff between
    // cold-cache and warm-cache runs. For a cold disk cache, we are limited by disk IO and
    // therefore want the number of threads to be rather large in order for the IO scheduler to
    // plan ahead. On the other hand, the number of threads shouldn't be too high for warm disk
    // caches where we would otherwise pay a higher synchronization overhead.
    let num_threads = matches
        .get_one::<String>("threads")
        .and_then(|t| t.parse().ok())
        .unwrap_or(3 * num_cpus::get());

    let paths: Vec<PathBuf> = matches
        .get_many::<String>("path")
        .map(|paths| paths.map(PathBuf::from).collect())
        .unwrap_or_else(|| vec![PathBuf::from(".")]);

    #[cfg(not(windows))]
    let filesize_type = if matches.get_flag("apparent-size") {
        FilesizeType::ApparentSize
    } else {
        FilesizeType::DiskUsage
    };

    #[cfg(windows)]
    let filesize_type = FilesizeType::DiskUsage;

    let size_format = match matches.get_one::<String>("size-format").map(|s| s.as_str()) {
        Some("decimal") => file_size_opts::DECIMAL,
        _ => file_size_opts::BINARY,
    };

    let verbose = matches.get_flag("verbose");

    let walk = Walk::new(&paths, num_threads, filesize_type);
    let (size, errors) = walk.run();
    print_result(size, &errors, &size_format, verbose);
}
