use crate::protocol::{FileMeta, FileMetaLocal};
use indicatif::HumanBytes;
use std::{
    collections::HashSet,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use owo_colors::OwoColorize;

/// Asks the user which of these files to accept.
/// Every accepted file in `files` will be represented by a
/// `true` at the same index in the returned `Vec<bool>`.
pub fn confirm_receive(files: &[FileMeta]) -> std::io::Result<Vec<bool>> {
    let mut new_files = Vec::<bool>::with_capacity(files.len());
    let mut new_size = 0;
    let mut total_size = 0;

    println!("Peer wants to send you {} files:", files.len());
    for file in files {
        let already_exists = file_exists(file)?;
        print!("{} ({})", file.short_path.display(), HumanBytes(file.len));
        if !already_exists {
            new_size += file.len;
            print!(" NEW");
        }
        total_size += file.len;
        new_files.push(!already_exists);
        println!();
    }

    println!();
    println!("Total size of offered files: {}", HumanBytes(total_size));
    println!(
        "Size of files that have a new/changed path or size: {}",
        HumanBytes(new_size)
    );
    println!("Options: ");
    println!("1. Download all files.");
    println!("2. Download only files with new path or size.");
    println!("3. Cancel.");
    print!("Choose an option (1, 2, or 3): ");
    std::io::stdout().flush()?;

    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    match response.trim() {
        "1" => Ok(new_files),
        "2" => Ok(vec![true; files.len()]),
        _ => Ok(vec![false; files.len()]),
    }
}

/// Finds all the files at the provided paths.
/// Asks the user to confirm they want to send them, otherwise exits.
/// Returns the list of files these paths lead to.
pub fn confirm_send(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
    let files = get_paths_metadatas(paths)?;

    println!("{} {}", files.len().bold(), "files:".bold());
    for file in &files {
        println!("{} ({})", file.short_path.display(), HumanBytes(file.len));
    }

    let total_size: u64 = files.iter().map(|file| file.len).sum();
    println!("\n{} {}", "Total size: ".bold(), HumanBytes(total_size));
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
    if let Ok(file) = File::open(local_save_path) {
        if let Ok(local_meta) = file.metadata() {
            if local_meta.len() == meta.len {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Takes a set of `paths`, each of which may be a directory or file.
/// Returns the [`FileMetaLocal`] of each file, including those in the given directories.
fn get_paths_metadatas(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
    let mut files = HashSet::new();

    for path in paths {
        get_path_metadatas(&path.canonicalize()?, &path.canonicalize()?, &mut files)?;
    }

    Ok(Vec::from_iter(files))
}

/// - The [`FileMetaLocal::short_path`] will strip the prefix
/// `top_path` from all paths. `top_path` should be a prefix of `path`.
/// - `path` is the file/directory where recursive traversal begins.
/// - `files` is a hashset to which found files will be added.
fn get_path_metadatas(
    top_path: &Path,
    path: &Path,
    files: &mut HashSet<FileMetaLocal>,
) -> std::io::Result<()> {
    if path.is_dir() {
        // recursively serch subdirectories
        for entry in path.read_dir()? {
            get_path_metadatas(top_path, &entry?.path(), files)?;
        }
    } else if path.is_file() {
        // Return an error if a file couldn't be opened.
        std::fs::File::open(path)?;
        let short_path = path.strip_prefix(top_path).unwrap().to_path_buf();
        let size = path.metadata()?.len();
        let meta = FileMetaLocal {
            local_path: path.to_path_buf(),
            short_path,
            len: size,
        };
        files.insert(meta);
    }

    Ok(())
}
