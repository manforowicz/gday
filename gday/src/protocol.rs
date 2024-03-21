#![warn(clippy::all)]
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Information about an offered file
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMeta {
    /// The file path offered
    pub short_path: PathBuf,
    /// Length of the file
    pub len: u64,
}

impl FileMeta {
    /// Returns the path where this incoming file should
    /// be saved.
    pub fn get_save_path(&self) -> std::io::Result<PathBuf> {
        Ok(std::env::current_dir()?.join(&self.short_path))
    }
}

/// Information about a local file
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileMetaLocal {
    /// The path that will be offered to the peer
    pub short_path: PathBuf,
    /// The file's location on this local machine
    pub local_path: PathBuf,
    /// Length of the file
    pub len: u64,
}


/// At the start of peer to peer communication,
/// the creator peer sends this message.
/// 
/// Optinonally, they can offer to transmit files
/// by sending some Vec of their metadatas. In that case,
/// the other peer will reply with [`FileResponseMsg`].
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileOfferMsg {
    files: Vec<FileMeta>,
}

/// This message responds to [`FileOfferMsg`] that
/// had a not-`None` field of `files`.
/// 
/// Specifies which of the offered files the other peer
/// should transmit.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileResponseMsg {
    /// The accepted files. `Some(start_byte)` element accepts the offered
    /// file from [`FileOfferMsg::files`] at the same index.
    /// Only `start_byte..` will be sent.
    accepted: Vec<Option<u64>>,
}
