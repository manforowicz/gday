use crate::Error;
use os_str_bytes::OsStrBytesExt;
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

/// Information about an offered file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileMeta {
    /// The path offered to the peer
    pub short_path: PathBuf,
    /// Length of the offered file in bytes
    pub len: u64,
}

/// Information about a locally stored file
#[derive(Debug, Clone, PartialEq)]
pub struct FileMetaLocal {
    /// The shortened path that will be offered to the peer
    pub short_path: PathBuf,
    /// The file's canonicalized location on this local machine
    pub local_path: PathBuf,
    /// Length of the file in bytes
    pub len: u64,
}

impl FileMeta {
    /// Gets the base path where the file that this
    /// [`FileMetaLocal`] represents should be saved.
    ///
    /// Returns `save_dir` joined with [`Self::short_path`].
    ///
    /// Use [`Self::get_unoccupied_save_path()`]
    /// to get an unoccupied version of this save path.
    pub fn get_save_path(&self, save_dir: &Path) -> PathBuf {
        save_dir.join(&self.short_path)
    }

    /// Returns a version of [`Self::get_save_path()`]
    /// that isn't taken yet.
    ///
    /// If [`self.get_save_path(save_dir)`](Self::get_save_path)
    /// is taken, suffixes its file stem with
    /// `" (1)"`, `" (2)"`, ..., `" (99)"` until a free path is found.
    ///
    /// If all of these (up to `" (99)"`) are occupied,
    /// returns [`Error::FilenameOccupied`].
    pub fn get_unoccupied_save_path(&self, save_dir: &Path) -> Result<PathBuf, Error> {
        let mut path = self.get_save_path(save_dir);
        let number = get_first_unoccupied_number(&path)?;

        if number != 0 {
            suffix_with_number(&mut path, number);
        }
        Ok(path)
    }

    /// Returns the occupied save path
    /// with the greatest numerical suffix.
    ///
    /// Iff [`Self::get_save_path()`]
    /// isn't occupied, returns `None`.
    ///
    /// The numerical suffix of the returned path
    /// will be one less than that of
    /// [`Self::get_unoccupied_save_path()`] (or no suffix
    /// if [`Self::get_unoccupied_save_path()`] has suffix of 1).
    pub fn get_last_occupied_save_path(&self, save_dir: &Path) -> Result<Option<PathBuf>, Error> {
        let mut path = self.get_save_path(save_dir);
        let number = get_first_unoccupied_number(&path)?;

        if number == 0 {
            Ok(None)
        } else {
            suffix_with_number(&mut path, number - 1);
            Ok(Some(path))
        }
    }

    /// Returns `true` iff a file is already saved at
    /// [`Self::get_last_occupied_save_path()`]
    /// with the same length as [`Self::len`].
    pub fn already_exists(&self, save_dir: &Path) -> Result<bool, Error> {
        if let Some(occupied) = self.get_last_occupied_save_path(save_dir)? {
            if let Ok(metadata) = occupied.metadata() {
                if metadata.is_file() && metadata.len() == self.len {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Gets the path where to store the temporary download file.
    ///
    /// Returns [`self.get_save_path(save_dir)`](Self::get_save_path)
    /// suffixed by the extension `".part{self.len}"`.
    pub fn get_partial_download_path(&self, save_dir: &Path) -> Result<PathBuf, Error> {
        let mut path = self.get_save_path(save_dir);
        let extension = format!(".part{}", self.len);
        let mut filename = path
            .file_name()
            .expect("Path terminates in ..")
            .to_os_string();
        filename.push(extension);
        path.set_file_name(filename);
        Ok(path)
    }

    /// Checks if [`Self::get_partial_download_path()`]
    /// already exists and has a length smaller than [`Self::len`].
    /// If so, returns the length of the partially downloaded file.
    /// If it doesn't exist, returns None.
    pub fn partial_download_exists(&self, save_dir: &Path) -> Result<Option<u64>, Error> {
        let local_path = self.get_partial_download_path(save_dir)?;

        // check if the file can be opened
        if let Ok(file) = std::fs::File::open(local_path) {
            // check if its length is less than the meta length
            if let Ok(local_meta) = file.metadata() {
                let local_len = local_meta.len();
                if local_len < self.len {
                    return Ok(Some(local_len));
                }
            }
        }
        Ok(None)
    }
}

impl From<FileMetaLocal> for FileMeta {
    /// Converts a [`FileMetaLocal`] into a [`FileMeta`].
    fn from(other: FileMetaLocal) -> Self {
        Self {
            short_path: other.short_path,
            len: other.len,
        }
    }
}

/// If the path isn't taken, returns `0`.
///
/// Otherwise, returns the smallest number, starting at 1, that
/// when suffixed to `path` (using [`suffix_with_number()`]),
/// gives an unoccupied path.
fn get_first_unoccupied_number(path: &Path) -> Result<u32, Error> {
    if !path.exists() {
        return Ok(0);
    }

    for i in 1..100 {
        let mut modified_path = PathBuf::from(path);
        suffix_with_number(&mut modified_path, i);

        if !modified_path.exists() {
            return Ok(i);
        }
    }

    Err(Error::FilenameOccupied(PathBuf::from(path)))
}

/// Appends the suffix `" ({number})"` to the file stem of `path`.
fn suffix_with_number(path: &mut PathBuf, number: u32) {
    // isolate the file name
    let filename = path.file_name().expect("Path terminates in ..");

    let suffix = format!(" ({number})");

    // split the filename at the first '.'
    if let Some((first, second)) = filename.split_once('.') {
        let mut filename = OsString::from(first);
        filename.push(suffix);
        filename.push(".");
        filename.push(second);
        path.set_file_name(filename);

    // if filename doesn't contain '.'
    // then append the suffix to the whole filename
    } else {
        let mut filename = OsString::from(filename);
        filename.push(suffix);
        path.set_file_name(filename);
    }
}

/// Takes a list of distinct `paths`, each of which may be a directory or file.
///
/// Returns the [`FileMetaLocal`] of each file, including those in nested directories.
///
/// Returns an error if can't access a path, one path is the prefix
/// of another path, or two paths end in the same name.
///
/// Each file's [`FileMeta::short_path`] will contain the path to the file,
/// starting at the provided level, ignoring parent directories.
pub fn get_file_metas(paths: &[PathBuf]) -> Result<Vec<FileMetaLocal>, Error> {
    // canonicalize the paths to remove symlinks
    let paths = paths
        .iter()
        .map(|p| p.canonicalize())
        .collect::<std::io::Result<Vec<PathBuf>>>()?;

    // Return an error if any path is a prefix of another,
    // or has the same folder or file name.
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            let a = &paths[i];
            let b = &paths[j];

            // we don't want two top-level folders or files with the same name
            // then we'd run into weird cases with FileMetaLocal.short_path
            if a.file_name() == b.file_name() && a.is_file() == b.is_file() {
                let name = a.file_name().unwrap_or(OsStr::new("")).to_os_string();
                return Err(Error::PathsHaveSameName(name));
            }

            if a.starts_with(b) {
                return Err(Error::PathIsPrefix(b.to_path_buf(), a.to_path_buf()));
            }
            if b.starts_with(a) {
                return Err(Error::PathIsPrefix(a.to_path_buf(), b.to_path_buf()));
            }
        }
    }

    let mut files = Vec::new();
    for path in paths {
        // get the parent path
        let top_path = path.parent().unwrap_or(Path::new(""));

        // add all files in this path to the files set
        get_file_metas_helper(top_path, &path, &mut files)?;
    }

    // build a vec from the set, and return
    Ok(files)
}

/// - The [`FileMetaLocal::short_path`] will strip the prefix
/// `top_path` from all paths. `top_path` must be a prefix of `path`.
/// - `path` is the file or directory where recursive traversal begins.
/// - `files` is a [`HashSet`] to which found files will be inserted.
fn get_file_metas_helper(
    top_path: &Path,
    path: &Path,
    files: &mut Vec<FileMetaLocal>,
) -> std::io::Result<()> {
    if path.is_dir() {
        // recursively traverse subdirectories
        for entry in path.read_dir()? {
            get_file_metas_helper(top_path, &entry?.path(), files)?;
        }
    } else if path.is_file() {
        // return an error if a file couldn't be opened.
        std::fs::File::open(path)?;

        // get the shortened path
        let short_path = path
            .strip_prefix(top_path)
            .expect("`top_path` was not a prefix of `path`.")
            .to_path_buf();

        // get the file's size
        let size = path.metadata()?.len();

        // insert this file metadata into set
        let meta = FileMetaLocal {
            local_path: path.to_path_buf(),
            short_path,
            len: size,
        };
        files.push(meta);
    }

    Ok(())
}
