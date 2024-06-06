//! Helper functions for asking the user questions through
//! the command line.
use gday_file_transfer::{FileMeta, FileMetaLocal};
use indicatif::HumanBytes;
use owo_colors::OwoColorize;
use std::{io::Write, path::PathBuf};

/// Recursively finds all the files at the provided paths and
/// asks the user to confirm they want to send them, otherwise exits.
/// Returns the list of files these paths lead to.
pub fn ask_send(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
    // get the file metadatas for all these paths
    let files = gday_file_transfer::get_file_metas(paths)?;

    // print all the file names and sizes
    println!("{} {}", files.len().bold(), "files to send:".bold());
    for file in &files {
        println!("{} ({})", file.short_path.display(), HumanBytes(file.len));
    }
    println!();

    // print their total size
    let total_size: u64 = files.iter().map(|file| file.len).sum();
    print!(
        "Would you like to send these ({})? (y/n): ",
        HumanBytes(total_size).bold()
    );
    std::io::stdout().flush()?;
    let input = get_lowercase_input()?;

    // act on user choice
    if "yes".starts_with(&input) {
        Ok(files)
    } else {
        println!("Cancelled.");
        std::process::exit(0);
    }
}

/// Asks the user which of these offered `files` to accept.
/// Returns a `Vec<Option<u64>>`, where each
/// - `None` represents rejecting the file at this index,
/// - `Some(0)` represents fully accepting the file at this index,
/// - `Some(x)` represents resuming with the first `x` bytes skipped.
pub fn ask_receive(files: &[FileMeta]) -> Result<Vec<Option<u64>>, gday_file_transfer::Error> {
    println!(
        "{} {} {}",
        "Your mate wants to send you".bold(),
        files.len().bold(),
        "files:".bold()
    );

    // A response accepting files that are not already fully saved
    let mut new_files = Vec::<Option<u64>>::with_capacity(files.len());
    // The size of files in the offer not already saved
    let mut new_size = 0;

    // Print all the offered files.
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
        println!();
    }

    println!();

    // The total size of all the files
    let total_size = files.iter().map(|f| f.len).sum();

    // If there are no existing/interrupted files,
    // send or quit.
    if new_size == total_size {
        print!("Download all ({})? (y/n): ", HumanBytes(total_size).bold());
        std::io::stdout().flush()?;
        let input = get_lowercase_input()?;

        if "yes".starts_with(&input) {
            return Ok(vec![Some(0); files.len()]);
        } else {
            println!("Cancelled.");
            std::process::exit(0);
        }
    }

    println!(
        "1. Fully download all files ({}).",
        HumanBytes(total_size).bold()
    );
    println!(
        "2. Download only new files, resuming any interrupted downloads ({}).",
        HumanBytes(new_size).bold()
    );
    println!("3. Cancel.");
    print!("{} ", "Choose an option (1, 2, or 3):".bold());
    std::io::stdout().flush()?;
    let input = get_lowercase_input()?;

    match input.as_str() {
        // all files
        "1" => Ok(vec![Some(0); files.len()]),
        // new/interrupted files
        "2" => Ok(new_files),
        // cancel
        _ => Ok(vec![None; files.len()]),
    }
}

/// Reads a trimmed ascii-lowercase line of input from the user
fn get_lowercase_input() -> std::io::Result<String> {
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    let response = response.trim().to_ascii_lowercase();
    Ok(response)
}
