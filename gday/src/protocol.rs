use postcard::{from_bytes, to_extend};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Information about an offered file
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileMeta {
    /// The file path offered
    pub short_path: PathBuf,
    /// Length of the file
    pub len: u64,
}

impl FileMeta {
    /// Returns the current directory joined with the [`FileMeta`]'s
    /// `short_path`.
    pub fn get_save_path(&self) -> std::io::Result<PathBuf> {
        Ok(std::env::current_dir()?.join(&self.short_path))
    }

    pub fn get_tmp_download_path(&self) -> std::io::Result<PathBuf> {
        // get this file's save path
        let save_path = self.get_save_path()?;

        // isolate the file name
        let filename = save_path.file_name().expect("Path terminates in .. ?");

        // add a prefix to the file name
        let filename = Path::new("PARTIAL_GDAY_DOWNLOAD_").join(filename);

        // get the parent directory
        let parent = save_path.parent().unwrap_or(Path::new(""));

        // join the path together
        let tmp_path = parent.join(filename);

        Ok(tmp_path)
    }
}

impl From<FileMetaLocal> for FileMeta {
    fn from(other: FileMetaLocal) -> Self {
        Self {
            short_path: other.short_path,
            len: other.len,
        }
    }
}

/// Information about a local file
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

/// At the start of peer to peer communication,
/// the creator peer sends this message.
///
/// Optinonally, they can offer to transmit files
/// by sending some Vec of their metadatas. In that case,
/// the other peer will reply with [`FileResponseMsg`].
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileOfferMsg {
    pub files: Vec<FileMeta>,
}

/// This message responds to [`FileOfferMsg`].
///
/// Specifies which of the offered files the other peer
/// should transmit.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileResponseMsg {
    /// The accepted files. `Some(start_byte)` element accepts the offered
    /// file from [`FileOfferMsg::files`] at the same index.
    /// Only bytes `(start_byte..)` will be sent.
    pub accepted: Vec<Option<u64>>,
}

pub fn serialize_into(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let buf = vec![0_u8; 4];
    let mut buf = to_extend(&msg, buf)?;
    let len = u32::try_from(buf.len() - 4)?;
    buf[0..4].copy_from_slice(&len.to_be_bytes());
    writer.write_all(&buf)?;
    Ok(())
}

pub fn deserialize_from<'a, T: Deserialize<'a>>(
    reader: &mut impl Read,
    buf: &'a mut Vec<u8>,
) -> Result<T, Error> {
    // read the length of the message
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;

    // read the message
    buf.resize(len, 0);
    reader.read_exact(buf)?;
    Ok(from_bytes(buf)?)
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Error encoding or decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("Error encoding or decoding message: {0}")]
    IO(#[from] std::io::Error),

    #[error("Serialized message too large: {0}")]
    MsgTooLarge(#[from] std::num::TryFromIntError),
}
