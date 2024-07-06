//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! This library lets you offer and transfer files to another peer,
//! assuming you already have a reliable connection established.
//!
//! # Example
//! The peers first establish a direct TCP connection using
//! [gday_hole_punch](https://docs.rs/gday_hole_punch/),
//! and encrypt it with
//! [gday_encryption](https://docs.rs/gday_encryption/).
//!
//! Peer A and peer B are on different computers in this example.
//! ```no_run
//! # use gday_file_transfer::{
//! #   get_file_metas,
//! #   FileOfferMsg,
//! #   FileResponseMsg,
//! #   write_to,
//! #   read_from,
//! #   send_files,
//! #   receive_files,
//! # };
//! # use std::path::Path;
//! # let mut stream = std::collections::VecDeque::new();
//! #
//! // Peer A offers files and folders they'd like to send
//! let paths_to_send = ["folder/to/send/".into(), "a/file.txt".into()];
//! let files_to_send = get_file_metas(&paths_to_send)?;
//! let offer_msg = FileOfferMsg::from(files_to_send.clone());
//! write_to(offer_msg, &mut stream)?;
//!
//! // Peer B responds to the offer
//! let offer_msg: FileOfferMsg = read_from(&mut stream)?;
//! let response_msg = FileResponseMsg::accept_only_new_and_interrupted(
//!     &offer_msg,
//!     Path::new("save/the/files/here/"),
//! )?;
//! write_to(response_msg, &mut stream)?;
//!
//! // Peer A sends the accepted files
//! let response_msg: FileResponseMsg = read_from(&mut stream)?;
//! send_files(&files_to_send, &response_msg, &mut stream, |progress| {})?;
//!
//! // Peer B receives the accepted files
//! let save_path = Path::new("save/the/files/here/");
//! receive_files(&offer_msg, &response_msg, save_path, &mut stream, |progress| {})?;
//! #
//! # Ok::<(), gday_file_transfer::Error>(())
//! ```
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
