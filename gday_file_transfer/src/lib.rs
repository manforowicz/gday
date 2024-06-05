//! This protocol lets one user offer to send some files,
//! and the other user respond with the files it wants to receive.
//!
//! On it's own, this crate doesn't do anything other than define a shared protocol, and functions to
//! send and receive messages of this protocol.
//!
//! # Process
//!
//! Using this protocol goes something like this:
//!
//! 1. Peer A sends [`FileOfferMsg`] to Peer B, containing a [`Vec`] of metadata about
//!     files it offers to send.
//!
//! 2. Peer B sends [`FileResponseMsg`] to Peer A, containing a [`Vec`] of [`Option<u64>`] indicating
//!     how much of each file to send.
//!
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod offer;
mod tests;
mod transfer;

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use gday_encryption::EncryptedStream;
use thiserror::Error;

pub use crate::offer::{
    read_from, write_to, FileMeta, FileMetaLocal, FileOfferMsg, FileResponseMsg,
};

pub use crate::transfer::{receive_files, send_files, TransferReport};

/// Wrap an IO stream in a [`gday_encryption::EncryptedStream`].
pub fn encrypt_connection<T: Read + Write>(
    mut io_stream: T,
    shared_key: &[u8; 32],
) -> std::io::Result<EncryptedStream<T>> {
    // Exchange random seeds with peer.
    let my_seed: [u8; 7] = rand::random();
    io_stream.write_all(&my_seed)?;
    io_stream.flush()?;
    let mut peer_seed = [0; 7];
    io_stream.read_exact(&mut peer_seed)?;

    // The nonce is the XOR of the random seeds.
    peer_seed
        .iter_mut()
        .zip(my_seed.iter())
        .for_each(|(x1, x2)| *x1 ^= *x2);

    Ok(EncryptedStream::new(io_stream, shared_key, &peer_seed))
}

/// Takes a set of `paths`, each of which may be a directory or file.
///
/// Returns the [`FileMetaLocal`] of each file, including those in nested directories.
/// Deduplicates any repeated files.
///
/// Each file's [`FileMeta::short_path`] will contain the path to the file,
/// starting at the provided level, ignoring parent directories.
pub fn get_paths_metadatas(paths: &[PathBuf]) -> std::io::Result<Vec<FileMetaLocal>> {
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
        // recursively traverse subdirectories
        for entry in path.read_dir()? {
            get_path_metadatas_helper(top_path, &entry?.path(), files)?;
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
        files.insert(meta);
    }

    Ok(())
}

/// `gday_file_transfer` error.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Error serializing or deserializing
    /// [`FileOfferMsg`] or [`FileResponseMsg`] to JSON.
    #[error("JSON Error: {0}")]
    JSON(#[from] serde_json::Error),

    /// IO Error
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    /// All 100 suitable filenames for this [`FileMeta`] are occupied.
    ///
    /// Comes from [`FileMeta::get_unoccupied_save_path()`]
    /// or [`FileMeta::get_partial_download_path()`].
    #[error("100 files with base name '{0}' already exist. Aborting save.")]
    FilenameOccupied(PathBuf),

    /// [`FileOfferMsg`] or [`FileResponseMsg`] was longer than 2^32
    /// bytes when serialized.
    ///
    /// A message's length header limits it to 2^32 bytes.
    #[error("Can't serialize JSON message longer than 2^32 bytes")]
    MsgTooLong,

    /// A local file had an unexpected length.
    #[error("A local file changed length between checks.")]
    UnexpectedFileLen,
}
