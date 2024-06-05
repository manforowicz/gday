use gday_file_transfer::{FileMeta, FileMetaLocal};
use indicatif::HumanBytes;
use owo_colors::OwoColorize;
use std::{
    collections::HashSet,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
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
        } else if let Some(local_len) = interrupted_exists(file)? {
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
    let files = get_paths_metadatas(paths)?;

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

/// Checks if there exists a file like `meta`
/// whose download was interrupted.
/// Iff there is an interrupted file, returns Some(size of interrupted file)
fn interrupted_exists(meta: &FileMeta) -> Result<Option<u64>, gday_file_transfer::Error> {
    let local_path = meta.get_partial_download_path()?;

    // check if the file can be opened
    if let Ok(file) = File::open(local_path) {
        // check if its length is less than the meta length
        if let Ok(local_meta) = file.metadata() {
            let local_len = local_meta.len();
            if local_len < meta.len {
                return Ok(Some(local_len));
            }
        }
    }
    Ok(None)
}

/// Takes a set of `paths`, each of which may be a directory or file.
/// Returns the [`FileMetaLocal`] of each file, including those in the given directories.
fn get_paths_metadatas(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
    // using a set to prevent duplicates
    let mut files = HashSet::new();

    for path in paths {
        // normalize and remove symlinks
        let path = path.canonicalize()?;

        // get the parent path
        let top_path = &path.parent().unwrap_or(Path::new(""));

        // add all files in this path to the files set
        get_path_metadatas_helper(top_path, &path, &mut files)?;
    }

    // build a vec from the set, and return
    Ok(Vec::from_iter(files))
}

/// - The [`FileMetaLocal::short_path`] will strip the prefix
/// `top_path` from all paths. `top_path` must be a prefix of `path`.
/// - `path` is the file or directory where recursive traversal begins.
/// - `files` is a [`HashSet`] to which found files will be inserted.
fn get_path_metadatas_helper(
    top_path: &Path,
    path: &Path,
    files: &mut HashSet<FileMetaLocal>,
) -> std::io::Result<()> {
    if path.is_dir() {
        // recursively serch subdirectories
        for entry in path.read_dir()? {
            get_path_metadatas_helper(top_path, &entry?.path(), files)?;
        }
    } else if path.is_file() {
        // return an error if a file couldn't be opened.
        std::fs::File::open(path)?;

        // get the shortened path
        let short_path = path
            .strip_prefix(top_path)
            .expect("Prefix couldn't be stripped?")
            .to_path_buf();

        // get the file's size
        let size = path.metadata()?.len();

        // insert this file metatada into set
        let meta = FileMetaLocal {
            local_path: path.to_path_buf(),
            short_path,
            len: size,
        };
        files.insert(meta);
    }

    Ok(())
}
