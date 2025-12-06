use std::path::PathBuf;

use clap::{builder::PossibleValuesParser, Arg, ArgAction, Command};
use humansize::{format_size, FormatSizeOptions, BINARY, DECIMAL};
use num_format::{Locale, ToFormattedString};

use diskus::{CountType, Directories, DiskUsage, DiskUsageResult, Error};

fn print_result(result: &DiskUsageResult, size_format: FormatSizeOptions, verbose: bool) {
    if verbose {
        for err in result.errors() {
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
    } else if !result.is_ok() {
        eprintln!(
            "[diskus warning] the results may be tainted. Re-run with -v/--verbose to print all errors."
        );
    }

    let size = result.ignore_errors().size_in_bytes();
    if atty::is(atty::Stream::Stdout) {
        println!(
            "{} ({:} bytes)",
            format_size(size, size_format),
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
        )
        .arg(
            Arg::new("directories")
                .long("directories")
                .value_name("mode")
                .value_parser(PossibleValuesParser::new(["auto", "included", "excluded"]))
                .default_value("auto")
                .help("Whether to count directory sizes (auto: included for disk usage, excluded for apparent size)"),
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
    let count_type = if matches.get_flag("apparent-size") {
        CountType::ApparentSize
    } else {
        CountType::DiskUsage
    };

    #[cfg(windows)]
    let count_type = CountType::DiskUsage;

    let size_format = match matches.get_one::<String>("size-format").map(|s| s.as_str()) {
        Some("decimal") => DECIMAL,
        _ => BINARY,
    };

    let verbose = matches.get_flag("verbose");

    let directories = match matches.get_one::<String>("directories").map(|s| s.as_str()) {
        Some("included") => Directories::Included,
        Some("excluded") => Directories::Excluded,
        _ => Directories::Auto,
    };

    let result = DiskUsage::new(paths)
        .num_workers(num_threads)
        .count_type(count_type)
        .directories(directories)
        .count();
    print_result(&result, size_format, verbose);
}
