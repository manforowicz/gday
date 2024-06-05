//! Helper functions for asking the user questions through
//! the command line.
use gday_file_transfer::{FileMeta, FileMetaLocal};
use indicatif::HumanBytes;
use owo_colors::OwoColorize;
use std::{
    io::Write,
    path::PathBuf,
};

/// Asks the user which of these offered `files` to accept.
/// Returns a `Vec<Option<u64>>`, where each
/// - `None` represents rejecting the file at this index,
/// - `Some(0)` represents fully accepting the file at this index,
/// - `Some(x)` represents resuming with the first `x` bytes skipped.
pub fn ask_receive(files: &[FileMeta]) -> Result<Vec<Option<u64>>, gday_file_transfer::Error> {
    let mut new_files = Vec::<Option<u64>>::with_capacity(files.len());
    let mut new_size = 0;
    let mut total_size = 0;

    println!(
        "{} {} {}",
        "Your mate wants to send you".bold(),
        files.len().bold(),
        "files:".bold()
    );
    for file in files {
        // print file metadata
        print!("{} ({})", file.short_path.display(), HumanBytes(file.len));

        // file was already downloaded
        if file.already_exists()? {
            print!(" {}", "ALREADY EXISTS".green().bold());
            new_files.push(None);

        // an interrupted download exists
        } else if let Some(local_len) = file.partial_download_exists()? {
            let remaining_len = file.len - local_len;

            print!(
                " {} {} {}",
                "PARTIALLY DOWNLOADED.".red().bold(),
                HumanBytes(remaining_len).red().bold(),
                "REMAINING".red().bold()
            );
            new_size += remaining_len;
            new_files.push(Some(local_len));

        // this file does not exist
        } else {
            new_size += file.len;
            new_files.push(Some(0));
        }

        total_size += file.len;
        println!();
    }

    println!();
    println!(
        "Size of all offered files: {}",
        HumanBytes(total_size).bold()
    );
    println!(
        "Size of files that have a new/changed path or size or were interrupted: {}",
        HumanBytes(new_size).bold()
    );
    println!("{}", "Options:".bold());
    println!(
        "{}",
        "1. Download only files with new path or size. Resume any interrupted downloads.".bold()
    );
    println!("{}", "2. Fully download all files.".bold());
    println!("{}", "3. Cancel.".bold());
    println!(
        "Note: gday won't overwrite existing files (it suffixes any new files with the same name)."
    );
    print!("{} ", "Choose an option (1, 2, or 3):".bold());
    std::io::stdout().flush()?;

    // act on user choice
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    match response.trim() {
        // new files
        "1" => Ok(new_files),
        // all files
        "2" => Ok(vec![Some(0); files.len()]),
        // cancel
        _ => Ok(vec![None; files.len()]),
    }
}

/// Recursively finds all the files at the provided paths and
/// asks the user to confirm they want to send them, otherwise exits.
/// Returns the list of files these paths lead to.
pub fn ask_send(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
    // get the file metadatas for all these paths
    let files = gday_file_transfer::get_paths_metadatas(paths)?;

    // print all the file names and sizes
    println!("{} {}", files.len().bold(), "files to send:".bold());
    for file in &files {
        println!("{} ({})", file.short_path.display(), HumanBytes(file.len));
    }

    // print their total size
    let total_size: u64 = files.iter().map(|file| file.len).sum();
    println!(
        "\n{} {}",
        "Total size: ".bold(),
        HumanBytes(total_size).bold()
    );
    print!("Would you like to send these? (y/n): ");
    std::io::stdout().flush()?;

    // act on user choice
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    if "yes".starts_with(response.trim()) {
        Ok(files)
    } else {
        println!("Cancelled.");
        std::process::exit(0);
    }
}
