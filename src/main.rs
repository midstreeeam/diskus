use std::io::{stdout, IsTerminal};
use std::path::PathBuf;

use clap::{builder::PossibleValuesParser, Arg, ArgAction, Command};
use humansize::{format_size, FormatSizeOptions, BINARY, DECIMAL};
use num_format::{Locale, ToFormattedString};

use diskus::{CountType, Directories, DiskUsage, DiskUsageEntriesResult, DiskUsageResult, Error};

fn print_error(err: &Error) {
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

fn print_result(result: &DiskUsageResult, size_format: FormatSizeOptions, verbose: bool) {
    if verbose {
        for err in result.errors() {
            print_error(err);
        }
    } else if !result.is_ok() {
        eprintln!(
            "[diskus warning] the results may be tainted. Re-run with -v/--verbose to print all errors."
        );
    }

    let size = result.ignore_errors().size_in_bytes();
    if stdout().is_terminal() {
        println!(
            "{} ({:} bytes)",
            format_size(size, size_format),
            size.to_formatted_string(&Locale::en)
        );
    } else {
        println!("{}", size);
    }
}

fn print_entries(result: &DiskUsageEntriesResult, size_format: FormatSizeOptions, verbose: bool) {
    if verbose {
        for err in result.errors() {
            print_error(err);
        }
        for entry in result.entries() {
            for err in entry.result().errors() {
                print_error(err);
            }
        }
    } else if !result.is_ok() {
        eprintln!(
            "[diskus warning] the results may be tainted. Re-run with -v/--verbose to print all errors."
        );
    }

    let is_terminal = stdout().is_terminal();
    for entry in result.entries() {
        let size = entry.result().ignore_errors().size_in_bytes();
        if is_terminal {
            println!(
                "{} ({:} bytes)\t{}",
                format_size(size, size_format),
                size.to_formatted_string(&Locale::en),
                entry.path().to_string_lossy()
            );
        } else {
            println!("{}\t{}", size, entry.path().to_string_lossy());
        }
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
            Arg::new("list")
                .long("list")
                .short('l')
                .action(ArgAction::SetTrue)
                .help("Print sizes for each direct child of the given directories"),
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

    let num_workers = matches
        .get_one::<String>("threads")
        .and_then(|t| t.parse().ok());

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

    let mut disk_usage = DiskUsage::new(paths)
        .count_type(count_type)
        .directories(directories);
    if let Some(n) = num_workers {
        disk_usage = disk_usage.num_workers(n);
    }
    if matches.get_flag("list") {
        let result = disk_usage.count_direct_children();
        print_entries(&result, size_format, verbose);
    } else {
        let result = disk_usage.count();
        print_result(&result, size_format, verbose);
    }
}
