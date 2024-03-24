use os_str_bytes::OsStrBytesExt;
use postcard::{from_bytes, to_extend};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::PathBuf,
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
    /// Returns the current directory joined with this [`FileMeta`]'s
    /// `short_path`.
    pub fn get_save_path(&self) -> std::io::Result<PathBuf> {
        Ok(std::env::current_dir()?.join(&self.short_path))
    }

    /// Return version of [`Self::get_save_path()`]
    /// that doesn't exist in the filesystem yet.
    ///
    /// If [`Self::get_save_path()`] already exists, suffixes its file stem with
    /// `(1)`, `(2)`, ..., `(99)` until a free path is found. If all of
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
    /// prefixed by `"PARTIAL_GDAY_DOWNLOAD_"`.
    pub fn get_tmp_download_path(&self) -> std::io::Result<PathBuf> {
        // get this file's save path
        let mut save_path = self.get_save_path()?;

        // add a prefix to its filename
        add_prefix_to_file_name(&mut save_path, OsString::from("PARTIAL_GDAY_DOWNLOAD_"))?;

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

/// TODO
pub fn serialize_into(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    // leave space for the message length
    let buf = vec![0_u8; 4];

    // write the message to buf
    let mut buf = to_extend(&msg, buf)?;

    // write the length to the beginning
    let len = u32::try_from(buf.len() - 4)?;
    buf[0..4].copy_from_slice(&len.to_be_bytes());

    // write to the writer
    writer.write_all(&buf)?;
    writer.flush()?;
    Ok(())
}

/// TODO
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

    #[error("100 files with base name {0} already exist. Aborting save.")]
    FilenameOccupied(PathBuf),
}
