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
//! #   write_to_async,
//! #   read_from_async,
//! #   send_files,
//! #   receive_files,
//! # };
//! # use std::path::Path;
//! #
//! # let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
//! # rt.block_on( async {
//! # let (stream1, stream2) = tokio::io::duplex(64);
//! # let mut stream1 = tokio::io::BufReader::new(stream1);
//! # let mut stream2 = tokio::io::BufReader::new(stream2);
//! // Peer A offers files and folders they'd like to send
//! let paths_to_send = ["folder/to/send/".into(), "a/file.txt".into()];
//! let files_to_send = get_file_metas(&paths_to_send)?;
//! let offer_msg = FileOfferMsg::from(files_to_send.clone());
//! write_to_async(offer_msg, &mut stream1).await?;
//!
//! // Peer B responds to the offer
//! let offer_msg: FileOfferMsg = read_from_async(&mut stream2).await?;
//! let response_msg = FileResponseMsg::accept_only_new_and_interrupted(
//!     &offer_msg,
//!     Path::new("save/the/files/here/"),
//! )?;
//! write_to_async(response_msg, &mut stream2).await?;
//!
//! // Peer A sends the accepted files
//! let response_msg: FileResponseMsg = read_from_async(&mut stream1).await?;
//! send_files(&files_to_send, &response_msg, &mut stream1, |progress| {}).await?;
//!
//! // Peer B receives the accepted files
//! let save_path = Path::new("save/the/files/here/");
//! receive_files(&offer_msg, &response_msg, save_path, &mut stream2, |progress| {}).await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }).unwrap();
//! ```
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod msg;
mod offer;
mod partial_download;
mod save_path;
mod transfer;

use std::path::PathBuf;
use thiserror::Error;

pub use crate::msg::*;
pub use crate::offer::*;
pub use crate::partial_download::*;
pub use crate::save_path::*;
pub use crate::transfer::*;

/// Version of the protocol.
/// Different numbers wound indicate
/// incompatible protocol breaking changes.
pub const PROTOCOL_VERSION: u8 = 1;

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

    /// All 100 suitable locations to save [`FileMeta`] are occupied.
    ///
    /// Comes from [`FileMeta::get_unoccupied_save_path()`]
    /// or [`FileMeta::get_partial_download_path()`].
    #[error("100 files with base name '{0}' already exist. Aborting save.")]
    FilenameOccupied(PathBuf),

    /// [`FileOfferMsg`] or [`FileResponseMsg`] was longer than 2^32
    /// bytes when serialized.
    ///
    /// Can't send message longer than 2^32 bytes.
    #[error("Can't send message longer than 2^32 bytes: {0}")]
    MsgTooLong(#[from] std::num::TryFromIntError),

    /// A local file had an unexpected length.
    #[error("A local file changed length between checks.")]
    UnexpectedFileLen,

    /// A requested start byte index in [`FileResponseMsg`]
    /// is greater than the length of the corresponding file offered in
    /// [`FileOfferMsg`].
    #[error("Requested start index greater than offered file length.")]
    InvalidStartIndex,

    /// Peer requested more files than were listed in the offer.
    #[error("Peer requested more files than were listed in the offer.")]
    TooManyFilesRequested,

    /// Peer requested a filename which wasn't in the offer.
    #[error("Peer requested a filename which wasn't in the offer.")]
    UnknownFileRequested,

    /// One path is a prefix of another. Local paths to send can't be nested within each other!
    #[error(
        "'{0}' is prefix of '{1}'. \
        Local paths to send can't be duplicated or nested within each other!"
    )]
    PathIsPrefix(PathBuf, PathBuf),

    /// Two of the given folders or files have same name.
    /// This would make the offered metadata ambiguous.
    #[error(
        "Two of the given folders or files have same name: '{0:?}'.
        This would make the offered metadata ambiguous."
    )]
    PathsHaveSameName(std::ffi::OsString),

    /// Received a message with an incompatible protocol version.
    /// Check if this software is up-to-date.
    #[error(
        "Received a message with an incompatible protocol version. \
        Check if this software is up-to-date."
    )]
    IncompatibleProtocol,

    /// Offered path contained illegal component such as .. or root /.
    #[error("Offered path {0} contained illegal component such as .. or root /.")]
    IllegalOfferedPath(PathBuf),
}
