use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{FileMetadata, FileOfferMsg};

pub const TMP_DOWNLOAD_FILE: &str = "gday_tmp_download.dat";
pub const TMP_INFO_FILE: &str = "gday_tmp_download_metadata.json";

/// Information about the file currently being downloaded.
/// Saved in [`TMP_INFO_FILE`] as json before the download,
/// and deleted after the download.
///
/// Allows detecting an interrupted download.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TmpInfoFile {
    /// The offered path of the file being downloaded.
    pub file_short_path: PathBuf,
    /// The provided metadata of the file being downloaded.
    pub file_metadata: FileMetadata,
}

/// Checks for interrupted download.
///
/// Interrupted downloads leave behind a
/// "gday_tmp_download.dat" and "gday_tmp_download_metadata.json" file
/// in `download_dir`.
///
/// If `offer` is re-offering an interrupted file,
/// returns the offered path of the interrupted file,
/// and the number of bytes already downloaded.
///
/// Otherwise returns [`None`].
pub fn detect_interrupted_download(
    download_dir: &Path,
    offer: &FileOfferMsg,
) -> Option<(PathBuf, u64)> {
    // Get the metadata of the interrupted download if it exists
    let tmp_info = read_tmp_info_file(download_dir).ok()?;

    // Get the corresponding metadata in the offer if it exists
    let offered_file = offer.offer.get(&tmp_info.file_short_path)?;

    // Transfer can't be resumed if offered metadata doesn't match interrupted
    // metadata
    if *offered_file != tmp_info.file_metadata {
        return None;
    }

    // Get the partial download file if it exists
    let tmp_download_metadata = download_dir.join(TMP_DOWNLOAD_FILE).metadata().ok()?;

    // Confirm it's a file
    if !tmp_download_metadata.is_file() {
        return None;
    }

    // Confirm it is shorter than the offfered length
    if tmp_download_metadata.len() >= tmp_info.file_metadata.size {
        return None;
    }

    Some((tmp_info.file_short_path, tmp_download_metadata.len()))
}

/// Writes `info_file` in `download_dir/TMP_INFO_FILE`.
pub fn write_tmp_info_file(download_dir: &Path, info_file: &TmpInfoFile) -> std::io::Result<()> {
    let file = std::fs::File::create(download_dir.join(TMP_INFO_FILE))?;
    serde_json::to_writer_pretty(file, info_file)?;
    Ok(())
}

/// Reads a `TmpInfoFile` from `download_dir/TMP_INFO_FILE`.
pub fn read_tmp_info_file(download_dir: &Path) -> std::io::Result<TmpInfoFile> {
    let file = std::fs::File::open(download_dir.join(TMP_INFO_FILE))?;
    let info_file = serde_json::from_reader(file)?;
    Ok(info_file)
}

/// Deletes a `TmpInfoFile` in `download_dir/TMP_INFO_FILE`.
pub fn delete_tmp_info_file(download_dir: &Path) -> std::io::Result<()> {
    std::fs::remove_file(download_dir.join(TMP_INFO_FILE))
}
