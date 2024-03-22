use crate::protocol::{FileMeta, FileMetaLocal};
use indicatif::HumanBytes;
use owo_colors::OwoColorize;
use std::{
    collections::HashSet,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

/// Asks the user which of these files to accept.
/// Every accepted file in `files` will be represented by a
/// `true` at the same index in the returned `Vec<bool>`.
/// TODO: Update this DOC comment
pub fn confirm_receive(files: &[FileMeta]) -> std::io::Result<Vec<Option<u64>>> {
    // size of all new and interrupted files to download
    let mut new_files = Vec::<Option<u64>>::with_capacity(files.len());
    let mut new_size = 0;
    let mut total_size = 0;

    println!("Your mate wants to send you {} files:", files.len().bold());
    for file in files {
        // print file metadata
        print!("{} ({})", file.short_path.display(), HumanBytes(file.len));

        // file was already downloaded
        if file_exists(file)? {
            print!(" {}", "ALREADY EXISTS".bold());
            new_files.push(None);

        // an interrupted download exists
        } else if let Some(local_len) = interrupted_exists(file)? {
            print!(" {}", "INTERRUPTED".bold());
            new_size += file.len - local_len;
            new_files.push(Some(local_len));

        // this file does not exist
        } else {
            new_size += file.len;
            new_files.push(Some(file.len));
        }

        total_size += file.len;
        println!();
    }

    println!();
    println!(
        "Total size of offered files: {}",
        HumanBytes(total_size).bold()
    );
    println!(
        "Size of files that have a new/changed path or size or were interrupted: {}",
        HumanBytes(new_size).bold()
    );
    println!("{}", "Options:".bold());
    println!("1. Download all files.");
    println!("2. Download only files with new path or size. Resume any interrupted downloads.");
    println!("3. Cancel.");
    print!("{} ", "Choose an option (1, 2, or 3):".bold());
    std::io::stdout().flush()?;

    // act on user choice
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    match response.trim() {
        // all files
        "1" => Ok(vec![Some(0)]),
        // new files
        "2" => Ok(new_files),
        // cancel
        _ => Ok(vec![None; files.len()]),
    }
}

/// Finds all the files at the provided paths.
/// Asks the user to confirm they want to send them, otherwise exits.
/// Returns the list of files these paths lead to.
pub fn confirm_send(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
    // get the file metadatas for all these paths
    let files = get_paths_metadatas(paths)?;

    // print all the file names and sizes
    println!("{} {}", files.len().bold(), "files:".bold());
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

    // ask the user whether they'd like to send these files
    print!("{}", "Would you like to send these files? (y/n): ".bold());
    std::io::stdout().flush()?;
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    if "yes".starts_with(&response.trim().to_lowercase()) {
        Ok(files)
    } else {
        println!("User cancelled file send.");
        std::process::exit(0);
    }
}

/// Checks if there already exists a file with the same save path
/// and size as `meta`.
fn file_exists(meta: &FileMeta) -> std::io::Result<bool> {
    let local_save_path = meta.get_save_path()?;

    // check if the file can be opened
    if let Ok(file) = File::open(local_save_path) {
        // check if its length is the same
        if let Ok(local_meta) = file.metadata() {
            if local_meta.len() == meta.len {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Checks if there exists a file like `meta`
/// whose download was interrupted.
/// Iff there is an interrupted file, returns Some(size of interrupted file)
fn interrupted_exists(meta: &FileMeta) -> std::io::Result<Option<u64>> {
    let local_path = meta.get_tmp_download_path()?;

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
