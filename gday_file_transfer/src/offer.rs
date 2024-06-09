use std::io::{Read, Write};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{Error, FileMeta, FileMetaLocal};

/// A [`Vec`] of file metadatas that this peer is offering
/// to send. The other peer should reply with [`FileResponseMsg`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileOfferMsg {
    pub files: Vec<FileMeta>,
}

impl From<Vec<FileMetaLocal>> for FileOfferMsg {
    fn from(local_files: Vec<FileMetaLocal>) -> Self {
        let files = local_files.into_iter().map(FileMeta::from).collect();

        Self { files }
    }
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

impl FileResponseMsg {
    /// Returns the number of non-rejected files.
    /// Returns the number of fully and partially accepted files.
    pub fn get_total_num_accepted(&self) -> usize {
        self.response.iter().filter_map(|f| *f).count()
    }

    /// Returns only the total number of partially accepted files.
    pub fn get_num_partially_accepted(&self) -> usize {
        self.response
            .iter()
            .filter_map(|f| *f)
            .filter(|&x| x != 0)
            .count()
    }
}

/// Write `msg` to `writer` using [`serde_json`], and flush.
///
/// Prefixes the message with 4 big-endian bytes that hold its length.
pub fn write_to(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let Ok(len_byte) = u32::try_from(vec.len()) else {
        return Err(Error::MsgTooLong);
    };
    writer.write_all(&len_byte.to_be_bytes())?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Read a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 4 big-endian bytes that hold its length.
pub fn read_from<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}
