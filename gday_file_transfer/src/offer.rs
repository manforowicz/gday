use crate::{Error, FileMeta, FileMetaLocal};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// A [`Vec`] of file metadatas that this peer is offering
/// to send. The other peer should reply with [`FileResponseMsg`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileOfferMsg {
    pub files: Vec<FileMeta>,
}

impl FileOfferMsg {
    /// Returns the sum of sizes
    /// of all offered files.
    pub fn get_total_offered_size(&self) -> u64 {
        self.files.iter().map(|f| f.len).sum()
    }

    /// Returns the number of bytes that would be transferred for this
    /// [`FileOfferMsg`] and corresponding [`FileResponseMsg`].
    pub fn get_transfer_size(&self, response: &FileResponseMsg) -> Result<u64, Error> {
        // The response must have the same number of elements
        // as the offer.
        if self.files.len() != response.response.len() {
            return Err(Error::InvalidResponseLength);
        }

        // sum up total transfer size
        let mut total_bytes = 0;
        for (file, start) in self.files.iter().zip(response.response.iter()) {
            if let Some(start) = start {
                total_bytes += file
                    .len
                    .checked_sub(*start)
                    .ok_or(Error::InvalidStartIndex)?;
            }
        }
        Ok(total_bytes)
    }
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
    /// Returns a [`FileResponseMsg`] that would
    /// accept all the offered files.
    pub fn accept_all_files(offer: &FileOfferMsg) -> Self {
        Self {
            response: vec![Some(0); offer.files.len()],
        }
    }

    /// Returns a [`FileResponseMsg`] that would
    /// reject all the offered files.
    pub fn reject_all_files(offer: &FileOfferMsg) -> Self {
        Self {
            response: vec![None; offer.files.len()],
        }
    }

    /// Returns a [`FileResponseMsg`] that would
    /// accept only files that are not yet in `save_dir`,
    /// or have a different size.
    ///
    /// Will NOT try to resume interrupted downloads
    /// by partially accepting files.
    ///
    /// Rejects all other files.
    pub async fn accept_only_full_new_files(
        offer: &FileOfferMsg,
        save_dir: &Path,
    ) -> Result<Self, Error> {
        let mut response = Vec::with_capacity(offer.files.len());

        for file_meta in &offer.files {
            if file_meta.already_exists(save_dir).await? {
                // reject
                response.push(None);
            } else {
                // accept full
                response.push(Some(0));
            }
        }
        Ok(Self { response })
    }

    /// Returns a [`FileResponseMsg`] that would
    /// accept only the remaining portions of files
    /// whose downloads to `save_dir` have been previously interrupted.
    ///
    /// Rejects all other files.
    pub async fn accept_only_remaining_portions(
        offer: &FileOfferMsg,
        save_dir: &Path,
    ) -> Result<Self, Error> {
        let mut response = Vec::with_capacity(offer.files.len());

        for offered in &offer.files {
            if let Some(existing_size) = offered.partial_download_exists(save_dir).await? {
                response.push(Some(existing_size));
            } else {
                response.push(None);
            }
        }
        Ok(Self { response })
    }

    /// Get a [`FileResponseMsg`] that would:
    /// - Accept the remaining portions of files whose
    /// downloads to `save_dir` have been previously interrupted,
    /// - AND files that are not yet in `save_dir`,
    /// or have a different size.
    ///
    /// Rejects all other files.
    pub async fn accept_only_new_and_interrupted(
        offer: &FileOfferMsg,
        save_dir: &Path,
    ) -> Result<FileResponseMsg, Error> {
        let mut response = Vec::with_capacity(offer.files.len());

        for offered in &offer.files {
            if let Some(existing_size) = offered.partial_download_exists(save_dir).await? {
                response.push(Some(existing_size));
            } else if offered.already_exists(save_dir).await? {
                response.push(None);
            } else {
                response.push(Some(0));
            }
        }
        Ok(FileResponseMsg { response })
    }

    /// Returns the number of fully accepted files.
    pub fn get_num_fully_accepted(&self) -> usize {
        self.response
            .iter()
            .filter_map(|f| *f)
            .filter(|f| *f == 0)
            .count()
    }

    /// Returns the number of non-rejected files.
    pub fn get_num_not_rejected(&self) -> usize {
        self.response.iter().filter(|f| f.is_some()).count()
    }

    /// Returns the total number of only partially accepted files.
    pub fn get_num_partially_accepted(&self) -> usize {
        self.response
            .iter()
            .filter_map(|f| *f)
            .filter(|&x| x != 0)
            .count()
    }
}

/// Writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with 4 big-endian bytes that hold its length.
pub fn write_to(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len_byte = u32::try_from(vec.len())?;
    writer.write_all(&len_byte.to_be_bytes())?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Asynchronously writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with a 4 big-endian bytes that hold its length.
pub async fn write_to_async(
    msg: impl Serialize,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len_byte = u32::try_from(vec.len())?;
    writer.write_all(&len_byte.to_be_bytes()).await?;
    writer.write_all(&vec).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 4 big-endian bytes that holds its length.
pub fn read_from<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}

/// Asynchronously reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 4 big-endian bytes that hold its length.
pub async fn read_from_async<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf).await?;
    Ok(serde_json::from_reader(&buf[..])?)
}
