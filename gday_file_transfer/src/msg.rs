use crate::{
    already_exists, detect_interrupted_download, get_download_path, Error, PROTOCOL_VERSION,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Information about an offered file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct FileMetadata {
    /// Size in bytes of the offered file
    pub size: u64,
    /// Last modified date of the offered file
    pub last_modified: SystemTime,
}

/// The sending peer sends this message to offer files,
/// and the receiver replies with [`FileRequestMsg`].
///
/// Contains a map from offered filenames to metadata about them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileOfferMsg {
    pub offer: HashMap<PathBuf, FileMetadata>,
}

impl FileOfferMsg {
    pub fn lookup_request<'a>(
        &'a self,
        request: &'a FileRequestMsg,
    ) -> Result<Vec<(&'a SingleFileRequest, &'a FileMetadata)>, Error> {
        if request.request.len() > self.offer.len() {
            return Err(Error::TooManyFilesRequested);
        }

        let mut transfer_files = Vec::new();

        for single_request in &request.request {
            let metadata = self
                .offer
                .get(&single_request.path)
                .ok_or(Error::UnknownFileRequested)?;

            if single_request.start_offset >= metadata.size {
                return Err(Error::InvalidStartIndex);
            }

            transfer_files.push((single_request, metadata))
        }

        Ok(transfer_files)
    }

    /// Returns the sum of sizes of all offered files.
    pub fn get_total_offered_size(&self) -> u64 {
        self.offer.values().map(|f| f.size).sum()
    }

    /// Returns the number of bytes that would be transferred for this
    /// [`FileOfferMsg`] and corresponding [`FileRequestMsg`].
    pub fn get_transfer_size(&self, request: &FileRequestMsg) -> Result<u64, Error> {
        let pairs = self.lookup_request(request)?;
        Ok(pairs
            .iter()
            .map(|(req, meta)| meta.size.checked_sub(req.start_offset).unwrap())
            .sum())
    }
}

/// The receiving peer replies with this message after getting a [`FileOfferMsg`].
///
/// A [`Vec`] of [`SingleFileRequest`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FileRequestMsg {
    pub request: Vec<SingleFileRequest>,
}

/// A part of [`FileRequestMsg`]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SingleFileRequest {
    /// Path of the requested file.
    pub path: PathBuf,
    /// The byte offset at which file transmission should start.
    /// Zero means full file request.
    /// Non-zero is used for interrupted transfer resumption.
    pub start_offset: u64,
}

impl FileRequestMsg {
    /// Returns a [`FileRequestMsg`] that would
    /// accept all the offered files.
    pub fn accept_all_files(offer: &FileOfferMsg) -> Self {
        Self {
            request: offer
                .offer
                .keys()
                .map(|path| SingleFileRequest {
                    path: path.to_path_buf(),
                    start_offset: 0,
                })
                .collect(),
        }
    }

    /// Returns a [`FileRequestMsg`] that would
    /// reject all the offered files.
    pub fn reject_all_files() -> Self {
        Self {
            request: Vec::new(),
        }
    }

    /// Returns a [`FileRequestMsg`] that would
    /// accept only files that are not yet in `save_dir`,
    /// or have a different size.
    ///
    /// Will NOT try to resume interrupted downloads
    /// by partially accepting files.
    ///
    /// Rejects all other files.
    pub fn accept_only_full_new_files(
        offer: &FileOfferMsg,
        save_dir: &Path,
    ) -> Result<Self, Error> {
        let mut response = Vec::with_capacity(offer.offer.len());

        for (path, file_meta) in &offer.offer {
            if !already_exists(&get_download_path(save_dir, path)?, file_meta)? {
                // accept full
                response.push(SingleFileRequest {
                    path: path.to_path_buf(),
                    start_offset: 0,
                });
            }
        }
        Ok(Self { request: response })
    }

    /// Get a [`FileResponseMsg`] that would:
    /// - Accept the remaining portions of files whose
    ///   downloads to `save_dir` have been previously interrupted,
    /// - AND files that are not yet in `save_dir`,
    ///   or have a different size.
    ///
    /// Rejects all other files.
    pub fn accept_only_new_and_interrupted(
        offer: &FileOfferMsg,
        save_dir: &Path,
    ) -> Result<Self, Error> {
        let mut request = Vec::new();

        let mut interrupted_download_path = None;

        if let Some((path, start_offset)) = detect_interrupted_download(save_dir, offer) {
            request.push(SingleFileRequest {
                path: path.clone(),
                start_offset,
            });
            interrupted_download_path = Some(path);
        }

        for (offered_path, offered_meta) in &offer.offer {
            if Some(offered_path) == interrupted_download_path.as_ref() {
                continue;
            }

            let download_path = get_download_path(save_dir, offered_path)?;

            if !already_exists(&download_path, offered_meta)? {
                request.push(SingleFileRequest {
                    path: offered_path.to_path_buf(),
                    start_offset: 0,
                });
            }
        }

        Ok(Self { request })
    }

    /// Returns the number of fully accepted files.
    pub fn get_num_fully_accepted(&self) -> usize {
        self.request.iter().filter(|r| r.start_offset == 0).count()
    }

    /// Returns the number of non-rejected files.
    pub fn get_num_not_rejected(&self) -> usize {
        self.request.len()
    }

    /// Returns the total number of only partially accepted files.
    pub fn get_num_partially_accepted(&self) -> usize {
        self.request.iter().filter(|r| r.start_offset != 0).count()
    }
}

/// Writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 4 bytes holding the length of the following message (all in big-endian).
pub fn write_to(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len = u32::try_from(vec.len())?;

    let mut header = [0; 5];
    header[0] = PROTOCOL_VERSION;
    header[1..5].copy_from_slice(&len.to_be_bytes());

    writer.write_all(&header)?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Asynchronously writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 4 bytes holding the length of the following message (all in big-endian).
pub async fn write_to_async(
    msg: impl Serialize,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len = u32::try_from(vec.len())?;

    let mut header = [0; 5];
    header[0] = PROTOCOL_VERSION;
    header[1..5].copy_from_slice(&len.to_be_bytes());

    writer.write_all(&header).await?;
    writer.write_all(&vec).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 4 big-endian bytes holding the length of the following message.
pub fn read_from<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, Error> {
    let mut header = [0_u8; 5];
    reader.read_exact(&mut header)?;
    if header[0] != PROTOCOL_VERSION {
        return Err(Error::IncompatibleProtocol);
    }
    let len = u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}

/// Asynchronously reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 4 big-endian bytes holding the length of the following message.
pub async fn read_from_async<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<T, Error> {
    let mut header = [0_u8; 5];
    reader.read_exact(&mut header).await?;
    if header[0] != PROTOCOL_VERSION {
        return Err(Error::IncompatibleProtocol);
    }
    let len = u32::from_be_bytes(header[1..5].try_into().unwrap()) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf).await?;
    Ok(serde_json::from_reader(&buf[..])?)
}
