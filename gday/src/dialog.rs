//! Helper functions for asking the user questions through
//! the command line.
use crate::{BOLD, GREEN, RED};
use gday_file_transfer::{
    FileOfferMsg, FileRequestsMsg, already_exists, detect_interrupted_download,
};
use indicatif::HumanBytes;
use std::{io::Write, path::Path};

/// Confirms that the user wants to send these files.
///
/// If not, returns false.
pub fn confirm_send(offer: &FileOfferMsg) -> std::io::Result<bool> {
    // print all the file names and sizes
    println!("{BOLD}Files to send:{BOLD:#}");
    for (path, meta) in &offer.offer {
        println!("{} ({})", path.display(), HumanBytes(meta.size));
    }
    println!();

    // print their total size
    let total_size: u64 = offer.get_total_offered_size();
    print!(
        "Would you like to send these {} files ({BOLD}{}{BOLD:#})? (y/n): ",
        offer.offer.len(),
        HumanBytes(total_size)
    );
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
) -> Result<FileRequestsMsg, gday_file_transfer::Error> {
    println!("{BOLD}Your mate wants to send you:{BOLD:#}");

    let mut interrupted_path = None;
    if let Some((path, start_offset)) = detect_interrupted_download(save_dir, offer) {
        let meta = &offer.offer[&path];
        println!(
            "{} {RED}(Interrupted. {} bytes remaining.){RED:#}",
            path.display(),
            HumanBytes(meta.size - start_offset)
        );
        interrupted_path = Some(path);
    }

    // Print all the offered files.
    for (path, meta) in &offer.offer {
        if Some(path) == interrupted_path.as_ref() {
            continue;
        }
        // print file metadata
        print!("{} ({})", path.display(), HumanBytes(meta.size));

        // file was already downloaded
        if already_exists(path, meta)? {
            print!(" {GREEN}ALREADY EXISTS{GREEN:#}");
        }
        println!();
    }

    println!();

    let new_files = FileRequestsMsg::accept_only_new_and_interrupted(offer, save_dir)?;
    let all_files = FileRequestsMsg::accept_all_files(offer);
    let no_files = FileRequestsMsg::reject_all_files();

    // If there are no existing/interrupted files,
    // send or quit.
    if new_files == all_files {
        print!(
            "Download all {} files ({})? {BOLD}(y/n){BOLD:#}: ",
            all_files.get_num_fully_accepted(),
            HumanBytes(offer.get_transfer_size(&all_files)?)
        );
        let input = get_lowercase_input()?;

        if "yes".starts_with(&input) {
            return Ok(all_files);
        } else {
            return Ok(no_files);
        }
    }

    println!(
        "1. Fully download all {} files ({}).",
        all_files.request.len(),
        HumanBytes(offer.get_transfer_size(&all_files)?)
    );

    if new_files.get_num_partially_accepted() == 0 {
        println!(
            "2. Only download the {} new files ({}).",
            new_files.get_num_fully_accepted(),
            HumanBytes(offer.get_transfer_size(&new_files)?)
        );
    } else if new_files.get_num_fully_accepted() == 0 {
        println!(
            "2. Only resume the {} interrupted downloads ({}).",
            new_files.get_num_partially_accepted(),
            HumanBytes(offer.get_transfer_size(&new_files)?)
        );
    } else {
        println!(
            "2. Only download the {} new files, and resume {} interrupted downloads ({}).",
            new_files.get_num_fully_accepted(),
            new_files.get_num_partially_accepted(),
            HumanBytes(offer.get_transfer_size(&new_files)?)
        );
    }

    println!("3. Cancel.");
    print!("{BOLD}Choose an option (1, 2, or 3):{BOLD:#} ");

    match get_lowercase_input()?.as_str() {
        // all files
        "1" => Ok(all_files),
        // new/interrupted files
        "2" => Ok(new_files),
        // cancel
        _ => Ok(no_files),
    }
}

/// Reads a trimmed ascii-lowercase line of input from the user.
fn get_lowercase_input() -> std::io::Result<String> {
    std::io::stdout().flush()?;
    let Some(response) = std::io::stdin().lines().next() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "Couldn't read user input.",
        ));
    };

    let response = response?.trim().to_ascii_lowercase();
    Ok(response)
}
