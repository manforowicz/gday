//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! This library lets you offer and transfer files to another peer,
//! assuming you already have a reliable connection established.
//!
//! # Example steps
//!
//! 1. Both peers encrypt their connection, using a crate
//! such as [gday_encryption](https://docs.rs/gday_encryption/).
//!
//! 2. Peer A calls [`get_file_metas()`] to get a [`Vec`] of [`FileMetaLocal`]
//! containing metadata about the files they'd like to send.
//!
//! 3. Peer A calls [`FileOfferMsg::from()`] on the `Vec<FileMetaLocal>`, to get
//! a serializable [`FileOfferMsg`].
//!
//! 4. Peer A sends [`FileOfferMsg`] to Peer B using [`write_to()`].
//!
//! 5. Peer B sends [`FileResponseMsg`] to Peer A, containing a corresponding
//! [`Vec`] of [`Option<u64>`] indicating how much of each offered file to send.
//! Each `None` rejects the offered file at the corresponding index.
//! Each `Some(0)` accepts the entire file at the corresponding index.
//! Each `Some(k)` requests only the part of the file starting at the `k`th byte
//! to be sent.
//!
//! 6. Peer A calls [`send_files()`].
//!
//! 7. Peer B calls [`receive_files()`].
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod file_meta;
mod offer;
mod transfer;

use std::path::PathBuf;
use thiserror::Error;

pub use crate::file_meta::{get_file_metas, FileMeta, FileMetaLocal};

pub use crate::offer::{read_from, write_to, FileOfferMsg, FileResponseMsg};

pub use crate::transfer::{receive_files, send_files, TransferReport};

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

    /// A requested start byte index in [`FileResponseMsg`]
    /// is greater than the length of the corresponding file offered in
    /// [`FileOfferMsg`].
    #[error("Requested start index greater than offered file length.")]
    InvalidStartIndex,

    /// The number of elements in [`FileResponseMsg`] didn't match
    /// the number of elements in the [`FileOfferMsg`].
    #[error("Number of elements in response message, doesn't match number of files offered.")]
    InvalidResponseLength,

    /// One path is a prefix of another. Local paths to send can't be nested within each other!
    #[error(
        "'{0}' is prefix of '{1}'. \
        Local paths to send can't be duplicated or nested within each other!"
    )]
    PathIsPrefix(PathBuf, PathBuf),

    /// Two folders or files have same name. This would make the offered metadata ambiguous.
    #[error("Two folders or files have same name: '{0:?}'. This would make the offered metadata ambiguous.")]
    PathsHaveSameName(std::ffi::OsString),
}
