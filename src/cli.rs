use std::cmp::Reverse;
use std::io::{stdout, IsTerminal};
use std::path::{Path, PathBuf};

use clap::{builder::PossibleValuesParser, Arg, ArgAction, Command};
use humansize::{format_size, FormatSizeOptions, BINARY, DECIMAL};
use num_format::{Locale, ToFormattedString};

use crate::{CountType, Directories, DiskUsage, DiskUsageEntriesResult, DiskUsageResult, Error};

const BAR_WIDTH: u64 = 40;

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

fn display_path(path: &Path, roots: &[PathBuf]) -> String {
    for root in roots {
        if let Ok(relative) = path.strip_prefix(root) {
            if relative.as_os_str().is_empty() {
                return path.to_string_lossy().into_owned();
            }
            return format!("/{}", relative.to_string_lossy());
        }
    }

    path.to_string_lossy().into_owned()
}

fn format_bar(size: u64, max_size: u64) -> (String, f64) {
    if max_size == 0 {
        return (" ".repeat(BAR_WIDTH as usize), 0.0);
    }

    let mut bar_len = size.saturating_mul(BAR_WIDTH) / max_size;
    if bar_len == 0 && size > 0 {
        bar_len = 1;
    }

    let mut bar = "█".repeat(bar_len as usize);
    bar.push_str(&" ".repeat((BAR_WIDTH - bar_len) as usize));

    let percentage = (size as f64) * 100.0 / (max_size as f64);
    (bar, percentage)
}

fn print_entries(
    result: &DiskUsageEntriesResult,
    roots: &[PathBuf],
    size_format: FormatSizeOptions,
    verbose: bool,
) {
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

    let mut entries: Vec<_> = result.entries().iter().collect();
    entries.sort_by_key(|entry| Reverse(entry.result().ignore_errors().size_in_bytes()));

    let max_size = entries
        .first()
        .map(|entry| entry.result().ignore_errors().size_in_bytes())
        .unwrap_or(0);

    for entry in entries {
        let size = entry.result().ignore_errors().size_in_bytes();
        let (bar, percentage) = format_bar(size, max_size);
        println!(
            "{:>10}  {}  {:>6.1}%  {}",
            format_size(size, size_format),
            bar,
            percentage,
            display_path(entry.path(), roots)
        );
    }
}

fn build_command(name: &'static str, about: &'static str, include_list: bool) -> Command {
    let mut cmd = Command::new(name)
        .version(env!("CARGO_PKG_VERSION"))
        .about(about)
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

    if include_list {
        cmd = cmd.arg(
            Arg::new("list")
                .long("list")
                .short('l')
                .action(ArgAction::SetTrue)
                .help("Print a sorted direct-child size chart for the given directories"),
        );
    }

    #[cfg(not(windows))]
    {
        cmd = cmd.arg(
            Arg::new("apparent-size")
                .long("apparent-size")
                .short('b')
                .action(ArgAction::SetTrue)
                .help("Compute apparent size instead of disk usage"),
        );
    }

    cmd
}

fn run(include_list: bool, force_list: bool, name: &'static str, about: &'static str) {
    let matches = build_command(name, about, include_list).get_matches();

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

    let mut disk_usage = DiskUsage::new(&paths)
        .count_type(count_type)
        .directories(directories);
    if let Some(n) = num_workers {
        disk_usage = disk_usage.num_workers(n);
    }

    if force_list || (include_list && matches.get_flag("list")) {
        let result = disk_usage.count_direct_children();
        print_entries(&result, &paths, size_format, verbose);
    } else {
        let result = disk_usage.count();
        print_result(&result, size_format, verbose);
    }
}

pub fn run_diskus() {
    run(
        true,
        false,
        "diskus",
        "Compute disk usage for the given filesystem entries",
    );
}

pub fn run_ku() {
    run(
        false,
        true,
        "ku",
        "Print a sorted direct-child disk usage chart",
    );
}
