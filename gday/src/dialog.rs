//! Helper functions for asking the user questions through
//! the command line.
use gday_file_transfer::{FileOfferMsg, FileResponseMsg};
use indicatif::HumanBytes;
use owo_colors::OwoColorize;
use std::{
    io::{BufRead, Write},
    path::Path,
};

/// Confirms that the user wants to send these `files``.
///
/// If not, returns false.
pub fn confirm_send(files: &FileOfferMsg) -> std::io::Result<bool> {
    // print all the file names and sizes
    println!("{}", "Files to send:".bold());
    for file in &files.files {
        println!("{} ({})", file.short_path.display(), HumanBytes(file.len));
    }
    println!();

    // print their total size
    let total_size: u64 = files.get_total_offered_size();
    print!(
        "Would you like to send these {} files ({})? (y/n): ",
        files.files.len(),
        HumanBytes(total_size).bold()
    );
    std::io::stdout().flush()?;
    let input = get_lowercase_input()?;

    // act on user choice
    if "yes".starts_with(&input) {
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Asks the user which of the files in `offer` to accept.
///
/// `save_dir` is the directory where the files will later be saved.
pub fn ask_receive(
    offer: &FileOfferMsg,
    save_dir: &Path,
) -> Result<FileResponseMsg, gday_file_transfer::Error> {
    println!("{}", "Your mate wants to send you:".bold());

    // Print all the offered files.
    for file in &offer.files {
        // print file metadata
        print!("{} ({})", file.short_path.display(), HumanBytes(file.len));

        // an interrupted download exists
        if let Some(local_len) = file.partial_download_exists(save_dir)? {
            let remaining_len = file.len - local_len;

            print!(
                " {} {} {}",
                "CAN RESUME DOWNLOAD.".red().bold(),
                HumanBytes(remaining_len).red().bold(),
                "REMAINING".red().bold()
            );

        // file was already downloaded
        } else if file.already_exists(save_dir)? {
            print!(" {}", "ALREADY EXISTS".green().bold());
        }
        println!();
    }

    println!();

    let new_files = FileResponseMsg::accept_only_new_and_interrupted(offer, save_dir)?;
    let all_files = FileResponseMsg::accept_all_files(offer);
    let no_files = FileResponseMsg::reject_all_files(offer);

    // If there are no existing/interrupted files,
    // send or quit.
    if new_files == all_files {
        print!(
            "Download all {} files ({})? (y/n): ",
            all_files.get_num_fully_accepted(),
            HumanBytes(offer.get_transfer_size(&all_files)?).bold()
        );
        std::io::stdout().flush()?;
        let input = get_lowercase_input()?;

        if "yes".starts_with(&input) {
            return Ok(all_files);
        } else {
            return Ok(no_files);
        }
    }

    println!(
        "1. Fully download all {} files ({}).",
        all_files.response.len(),
        HumanBytes(offer.get_transfer_size(&all_files)?).bold()
    );

    if new_files.get_num_partially_accepted() == 0 {
        println!(
            "2. Only download the {} new files ({}).",
            new_files.get_num_fully_accepted(),
            HumanBytes(offer.get_transfer_size(&new_files)?).bold()
        );
    } else if new_files.get_num_fully_accepted() == 0 {
        println!(
            "2. Only resume the {} interrupted downloads ({}).",
            new_files.get_num_partially_accepted(),
            HumanBytes(offer.get_transfer_size(&new_files)?).bold()
        );
    } else {
        println!(
            "2. Only download the {} new files, and resume {} interrupted downloads ({}).",
            new_files.get_num_fully_accepted(),
            new_files.get_num_partially_accepted(),
            HumanBytes(offer.get_transfer_size(&new_files)?).bold()
        );
    }

    println!("3. Cancel.");
    print!("{} ", "Choose an option (1, 2, or 3):".bold());
    std::io::stdout().flush()?;

    match get_lowercase_input()?.as_str() {
        // all files
        "1" => Ok(all_files),
        // new/interrupted files
        "2" => Ok(new_files),
        // cancel
        _ => Ok(FileResponseMsg::reject_all_files(offer)),
    }
}

/// Reads a trimmed ascii-lowercase line of input from the user.
fn get_lowercase_input() -> std::io::Result<String> {
    let Some(response) = std::io::BufReader::new(std::io::stdin()).lines().next() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "Couldn't read user input.",
        ));
    };

    let response = response?.trim().to_ascii_lowercase();
    Ok(response)
}
