use crate::{Error, FileMetadata, FileOfferMsg};
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// The sending peer uses this struct to
/// store its [`FileOfferMsg`] and
/// a mapping from the shortened offered paths
/// to the local on-disk file paths.
pub struct LocalFileOffer {
    /// Offer that the sending peer will send.
    pub offer: FileOfferMsg,
    /// Sending peer's mapping from the shortened paths in `offer`
    /// to the local on-disk file paths.
    pub offered_path_to_local: HashMap<PathBuf, PathBuf>,
}

/// Returns a [`LocalFileOffer`] referring to all the files and directories
/// within `paths`.
///
/// Returns an error if can't access a path, one path is the prefix
/// of another path, or two of the given `paths` end in the same name.
pub fn create_file_offer(paths: &[PathBuf]) -> Result<LocalFileOffer, Error> {
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

            // We don't want two top-level folders or files with the same name.
            // Else we'd run into ambiguity with the offered file paths.
            if a.file_name() == b.file_name() && a.is_file() == b.is_file() {
                let name = a.file_name().unwrap_or(OsStr::new("")).to_os_string();
                return Err(Error::PathsHaveSameName(name));
            }

            // we don't want one path to be a prefix of another, or we'd
            // get duplicates
            if a.starts_with(b) {
                return Err(Error::PathIsPrefix(b.to_path_buf(), a.to_path_buf()));
            }
            if b.starts_with(a) {
                return Err(Error::PathIsPrefix(a.to_path_buf(), b.to_path_buf()));
            }
        }
    }

    let mut offer = LocalFileOffer {
        offer: FileOfferMsg {
            offer: HashMap::new(),
        },
        offered_path_to_local: HashMap::new(),
    };

    for path in paths {
        // get the parent path
        let top_path = path.parent().unwrap_or(Path::new(""));

        // add all files in this path to the offer
        get_file_metas_helper(top_path, &path, &mut offer)?;
    }

    Ok(offer)
}

/// - The offered filepaths have the `top_path` prefixed stripped form them.
/// - `path` is the file or directory where recursive traversal begins.
/// - All files will be inserted into `offer`.
fn get_file_metas_helper(
    top_path: &Path,
    path: &Path,
    offer: &mut LocalFileOffer,
) -> std::io::Result<()> {
    if path.is_dir() {
        // recursively traverse subdirectories
        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            get_file_metas_helper(top_path, &entry?.path(), offer)?;
        }
    } else if path.is_file() {
        // return an error if a file couldn't be opened.
        let metadata = std::fs::File::open(path)?.metadata()?;

        // get the shortened path
        let short_path = path
            .strip_prefix(top_path)
            .expect("`top_path` was not a prefix of `path`.")
            .to_path_buf();

        // insert this file metadata into the offer
        let meta = FileMetadata {
            size: metadata.len(),
            last_modified: metadata.modified().unwrap_or(SystemTime::now()),
        };
        let res = offer.offer.offer.insert(short_path.clone(), meta);
        assert_eq!(res, None);
        let res = offer
            .offered_path_to_local
            .insert(short_path, path.to_path_buf());
        assert_eq!(res, None);
    }

    Ok(())
}
