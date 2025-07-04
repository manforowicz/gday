use crate::{Error, FileMetadata};
use os_str_bytes::OsStrBytesExt;
use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

/// Joins `save_dir` and `offered_path`.
///
/// Returns an error if `offered_filepath` contains
/// invalid components such as .. or the root /.
pub fn get_download_path(download_dir: &Path, offered_filepath: &Path) -> Result<PathBuf, Error> {
    if !offered_filepath
        .components()
        .all(|c| matches!(c, Component::CurDir) || matches!(c, Component::Normal(_)))
    {
        return Err(Error::IllegalOfferedPath(offered_filepath.to_path_buf()));
    }

    Ok(download_dir.join(offered_filepath))
}

/// Returns a version of `path` that isn't occupied.
///
/// If `path` is occupied suffixes its file stem with
/// `" (1)"`, `" (2)"`, ..., `" (99)"` until a free path is found.
///
/// If all of these up to `" (99)"` are occupied,
/// returns [`Error::FilenameOccupied`].
pub fn get_unoccupied_version(path: &Path) -> Result<PathBuf, Error> {
    let number = get_first_unoccupied_number(path)?;
    Ok(suffix_path(path, number))
}

/// Returns the occupied `path`
/// with the greatest numerical suffix.
///
/// Iff `path` isn't occupied, returns `None`.
pub fn get_last_occupied_version(path: &Path) -> Result<Option<PathBuf>, Error> {
    let number = get_first_unoccupied_number(path)?;

    if number == 0 {
        Ok(None)
    } else {
        Ok(Some(suffix_path(path, number - 1)))
    }
}

/// Returns `true` iff a file is already saved at
/// `get_last_occupied_version(path)`
/// with the same length as in `metadata`.
pub fn already_exists(path: &Path, metadata: &FileMetadata) -> Result<bool, Error> {
    let Some(occupied) = get_last_occupied_version(path)? else {
        return Ok(false);
    };

    let Ok(local_meta) = occupied.metadata() else {
        return Ok(false);
    };

    if !local_meta.is_file() {
        return Ok(false);
    }

    if local_meta.len() != metadata.size {
        return Ok(false);
    }

    Ok(true)
}

/// If the path isn't taken, returns `0`.
///
/// Otherwise, returns the smallest number, starting at 1, that
/// when suffixed to `path` using [`suffix_path()`],
/// gives an unoccupied path.
fn get_first_unoccupied_number(path: &Path) -> Result<u32, Error> {
    // if the file doesn't exist
    if !path.exists() {
        return Ok(0);
    }

    for i in 1..100 {
        let modified_path = suffix_path(path, i);

        if !modified_path.exists() {
            return Ok(i);
        }
    }

    Err(Error::FilenameOccupied(PathBuf::from(path)))
}

/// Returns `path` suffixed with `" ({number})"`.
/// If `number` is 0, returns `path` unchanged.
fn suffix_path(path: &Path, number: u32) -> PathBuf {
    if number == 0 {
        return path.to_path_buf();
    }

    let mut new_path = path.to_path_buf();

    // isolate the file name
    let filename = path.file_name().expect("Path terminates in ..");

    let suffix = format!(" ({number})");

    // split the filename at the first '.'
    if let Some((first, second)) = filename.split_once('.') {
        let mut filename = OsString::from(first);
        filename.push(suffix);
        filename.push(".");
        filename.push(second);
        new_path.set_file_name(filename);

    // if filename doesn't contain '.'
    // then append the suffix to the whole filename
    } else {
        let mut filename = OsString::from(filename);
        filename.push(suffix);
        new_path.set_file_name(filename);
    }

    new_path
}
