use crate::Error;
use gday_encryption::EncryptedStream;
use os_str_bytes::OsStrBytesExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    io::{Read, Write},
    path::PathBuf,
};

/// Information about an offered file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileMeta {
    /// The path offered
    pub short_path: PathBuf,
    /// Length of the file in bytes
    pub len: u64,
}

/// Information about a locally stored file
#[derive(Debug, Clone)]
pub struct FileMetaLocal {
    /// The path that will be offered to the peer
    pub short_path: PathBuf,
    /// The file's location on this local machine
    pub local_path: PathBuf,
    /// Length of the file in bytes
    pub len: u64,
}

impl FileMeta {
    /// Returns the current directory joined with [`Self::short_path`]
    pub fn get_save_path(&self) -> std::io::Result<PathBuf> {
        Ok(std::env::current_dir()?.join(&self.short_path))
    }

    /// Returns `true` iff a file already exists at [`Self::get_save_path()`]
    /// with the same length as [`Self::len`].
    pub fn already_exists(&self) -> std::io::Result<bool> {
        let local_save_path = self.get_save_path()?;

        // check if the file can be opened
        if let Ok(file) = std::fs::File::open(local_save_path) {
            // check if its length is the same
            if let Ok(local_meta) = file.metadata() {
                if local_meta.len() == self.len {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Returns a suffixed [`Self::get_save_path()`]
    /// that isn't taken yet.
    ///
    /// If [`Self::get_save_path()`] already exists, suffixes its file stem with
    /// `_1`, `_2`, ..., `_99` until a free path is found. If all of
    /// these are occupied, returns [`Error::FilenameOccupied`].
    pub fn get_unoccupied_save_path(&self) -> Result<PathBuf, Error> {
        let plain_path = self.get_save_path()?;

        if !plain_path.exists() {
            return Ok(plain_path)
        }

        for i in 1..100 {
            // otherwise make a new `modified_path`
            // with a different suffix
            let mut modified_path = plain_path.clone();
            let suffix = OsString::from(format!(" ({i})"));
            add_suffix_to_file_stem(&mut modified_path, &suffix)?;

            // if the `modified_path` doesn't exist,
            // then return it
            if !modified_path.exists() {
                return Ok(modified_path);
            }
        }

        Err(Error::FilenameOccupied(plain_path))
    }

    /// Returns [`Self::get_unoccupied_save_path()`] suffixed by `".part"`.
    pub fn get_partial_download_path(&self) -> Result<PathBuf, Error> {
        let mut path = self.get_unoccupied_save_path()?;
        let mut filename = path
            .file_name()
            .expect("Path terminates in ..")
            .to_os_string();
        filename.push(".part");
        path.set_file_name(filename);
        Ok(path)
    }

    /// Returns `true` iff [`Self::get_partial_download_path()`] already exists.
    pub fn partial_download_exists(&self) -> Result<bool, Error> {
        let download_path = self.get_partial_download_path()?;

        // check if the file can be opened
        Ok(std::fs::File::open(download_path).is_ok())
    }
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

/// A [`Vec`] of file metadatas that this peer is offering
/// to send. The other peer should reply with [`FileResponseMsg`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileOfferMsg {
    pub files: Vec<FileMeta>,
}

/// The receiving peer should reply with this message to [`FileOfferMsg`].
/// Specifies which of the offered files the other peer should send.
///
/// A [`Vec`] of [`Option<u64>`] that correspond to the offered [`FileMeta`]
/// at the same indices.
///
/// - `None` indicates that the corresponding file is rejected.
/// - `Some(0)` indicates that the corresponding file is fully accepted.
/// - `Some(k)` indicates that the corresponding file is accepted,
/// except for the first `k` bytes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileResponseMsg {
    /// The accepted files. `Some(start_byte)` element accepts the offered
    /// file from [`FileOfferMsg::files`] at the same index.
    /// Only bytes `(start_byte..)` will be sent.
    pub response: Vec<Option<u64>>,
}

/// Write `msg` to `writer` using [`serde_json`].
/// Prefixes the message with 4 big-endian bytes that hold its length.
pub fn write_to(
    msg: impl Serialize,
    writer: &mut EncryptedStream<impl Write>,
) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let Ok(len_byte) = u32::try_from(vec.len()) else {
        return Err(Error::MsgTooLong);
    };
    writer.write_all(&len_byte.to_be_bytes())?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Read `msg` from `reader` using [`serde_json`].
/// Assumes the message is prefixed with 4 big-endian bytes that hold its length.
pub fn read_from<T: DeserializeOwned>(reader: &mut EncryptedStream<impl Read>) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}

/// Private helper function. Appends `suffix` to the file stem of `path`.
fn add_suffix_to_file_stem(path: &mut PathBuf, suffix: &OsStr) -> std::io::Result<()> {
    // isolate the file name
    let filename = path.file_name().expect("Path terminates in ..");

    // split the filename at the first '.'
    if let Some((first, second)) = filename.split_once('.') {
        let mut filename = OsString::from(first);
        filename.push(suffix);
        filename.push(".");
        filename.push(second);
        path.set_file_name(filename);

    // if filename doesn't contain '.'
    // then append the suffix to the whole filename
    } else {
        let mut filename = OsString::from(filename);
        filename.push(suffix);
        path.set_file_name(filename);
    }

    Ok(())
}
