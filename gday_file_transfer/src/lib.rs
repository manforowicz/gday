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

use std::io::{Read, Write};
use std::path::PathBuf;

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

/// Error with gday file transfer.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("JSON Error: {0}")]
    JSON(#[from] serde_json::Error),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("100 files with base name '{0}' already exist. Aborting save.")]
    FilenameOccupied(PathBuf),

    #[error("Can't serialize JSON message longer than 2^32 bytes")]
    MsgTooLong,

    /// A local file changed length between checks.
    #[error("A local file changed length between checks.")]
    UnexpectedFileLen,
}
