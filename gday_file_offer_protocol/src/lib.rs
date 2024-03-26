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
#![forbid(unsafe_code)]
#![warn(clippy::all)]

use os_str_bytes::OsStrBytesExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::PathBuf,
};
use thiserror::Error;

/// Information about an offered file.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMeta {
    /// The file path offered
    pub short_path: PathBuf,
    /// Length of the file
    pub len: u64,
}

impl FileMeta {
    /// Returns the current directory joined with this [`FileMeta`]'s
    /// `short_path`.
    pub fn get_save_path(&self) -> std::io::Result<PathBuf> {
        Ok(std::env::current_dir()?.join(&self.short_path))
    }

    /// Returns a version of [`Self::get_save_path()`]
    /// that doesn't exist in the filesystem yet.
    ///
    /// If [`Self::get_save_path()`] already exists, suffixes its file stem with
    /// ` (1)`, ` (2)`, ..., ` (99)` until a free path is found. If all of
    /// these are occupied, returns [`Error::FilenameOccupied`].
    pub fn get_unused_save_path(&self) -> Result<PathBuf, Error> {
        let plain_path = self.get_save_path()?;

        let mut modified_path = plain_path.clone();

        for i in 1..100 {
            // if the `modified_path` doesn't exist,
            // then return it
            if !modified_path.exists() {
                return Ok(modified_path);
            }

            // otherwise make a new `modified_path`
            // with a different suffix
            modified_path = plain_path.clone();
            let suffix = OsString::from(format!(" ({i})"));
            add_suffix_to_file_stem(&mut modified_path, &suffix)?;
        }

        Err(Error::FilenameOccupied(plain_path))
    }

    /// Returns [`Self::get_save_path()`] with its file name
    /// prefixed by `prefix`.
    pub fn get_prefixed_save_path(&self, prefix: OsString) -> std::io::Result<PathBuf> {
        // get this file's save path
        let mut save_path = self.get_save_path()?;

        // add a prefix to its filename
        add_prefix_to_file_name(&mut save_path, prefix)?;

        Ok(save_path)
    }
}

/// Prepend `prefix` to the file name of `path`
fn add_prefix_to_file_name(path: &mut PathBuf, mut prefix: OsString) -> std::io::Result<()> {
    // isolate the file name
    let filename: &OsStr = path.file_name().expect("Path terminates in .. ?");

    // add a prefix to the file name
    prefix.push(filename);

    // join the path together
    path.set_file_name(prefix);

    Ok(())
}

/// Append `suffix` to the file stem of `path`
fn add_suffix_to_file_stem(path: &mut PathBuf, suffix: &OsStr) -> std::io::Result<()> {
    // isolate the file name
    let filename = path.file_name().expect("Path terminates in .. ?");

    // split the filename at the first '.'
    if let Some((first, second)) = filename.split_once('.') {
        let mut first = OsString::from(first);
        first.push(suffix);
        first.push(".");
        first.push(second);
        path.set_file_name(first);

    // if filename doesn't contain '.'
    // then append the suffix to the whole filename
    } else {
        let mut filename = OsString::from(filename);
        filename.push(suffix);
        path.set_file_name(filename);
    }

    Ok(())
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

/// Information about a locally stored file
#[derive(Debug, Clone)]
pub struct FileMetaLocal {
    /// The path that will be offered to the peer
    pub short_path: PathBuf,
    /// The file's location on this local machine
    pub local_path: PathBuf,
    /// Length of the file
    pub len: u64,
}

impl PartialEq for FileMetaLocal {
    /// Two local files are equal iff they're at the same path
    fn eq(&self, other: &Self) -> bool {
        self.local_path == other.local_path
    }
}

impl Eq for FileMetaLocal {}

impl std::hash::Hash for FileMetaLocal {
    /// Two local files are equal iff they're at the same path
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.local_path.hash(state);
    }
}

/// A list of file metadatas that this peer is offering
/// to send. The other peer should reply with [`FileResponseMsg`].
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileOfferMsg {
    pub files: Vec<FileMeta>,
}

/// The receiving peer should reply with this message to [`FileOfferMsg`].
///
/// Specifies which of the offered files the other peer
/// should send.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileResponseMsg {
    /// The accepted files. `Some(start_byte)` element accepts the offered
    /// file from [`FileOfferMsg::files`] at the same index.
    /// Only bytes `(start_byte..)` will be sent.
    pub accepted: Vec<Option<u64>>,
}

/// Write `msg` to `writer` using [`serde_json`].
/// Prefixes the message with 4 big-endian bytes that hold its length.
pub fn to_writer(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len_byte = u32::try_from(vec.len())?;
    writer.write_all(&len_byte.to_be_bytes())?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Read `msg` from `reader` using [`serde_json`].
/// Assumes the message is prefixed with 4 big-endian bytes that hold its length.
pub fn from_reader<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Error encoding or decoding message: {0}")]
    JSON(#[from] serde_json::Error),

    #[error("Error encoding or decoding message: {0}")]
    IO(#[from] std::io::Error),

    #[error("Serialized message too large: {0}")]
    MsgTooLarge(#[from] std::num::TryFromIntError),

    #[error("100 files with base name {0} already exist. Aborting save.")]
    FilenameOccupied(PathBuf),
}
