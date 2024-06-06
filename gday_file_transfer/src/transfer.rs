use gday_encryption::EncryptedStream;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufWriter, Read, Seek, SeekFrom, Write};

use crate::{Error, FileMeta, FileMetaLocal, FileOfferMsg, FileResponseMsg};

const FILE_BUFFER_SIZE: usize = 1_000_000;

/// Reports the status of the file transfer
#[derive(Debug, Clone)]
pub struct TransferReport {
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub processed_files: u64,
    pub total_files: u64,
    pub current_file: std::path::PathBuf,
}

pub fn send_files(
    offer: Vec<FileMetaLocal>,
    response: FileResponseMsg,
    writer: &mut EncryptedStream<impl Write>,
    progress_callback: Option<impl FnMut(&TransferReport)>,
) -> Result<(), Error> {
    let files: Vec<(FileMetaLocal, u64)> = offer
        .into_iter()
        .zip(response.response)
        .filter_map(|(file, response)| response.map(|response| (file, response)))
        .collect();

    send_these_files(&files, writer, progress_callback)
}

pub fn receive_files(
    offer: FileOfferMsg,
    response: FileResponseMsg,
    reader: &mut EncryptedStream<impl Read>,
    progress_callback: Option<impl FnMut(&TransferReport)>,
) -> Result<(), Error> {
    let files: Vec<(FileMeta, u64)> = offer
        .files
        .into_iter()
        .zip(response.response)
        .filter_map(|(file, response)| response.map(|response| (file, response)))
        .collect();

    receive_these_files(&files, reader, progress_callback)
}

/// Sequentially write the requested parts of `files` to `writer`.
///
/// `files` is a slice of tuples:
/// (my offered file: [`FileMetaLocal`], peer's response: [`Option<u64>`])
///
/// `progress_callback` gets repeatedly called when writing to report progress.
/// These arguments are passed to it:
/// - `&str` - `short_path` of the file currently being sent.
/// - `u64` - The number of bytes written so far.
/// - `u64` - The total number of bytes to write.
pub fn send_these_files(
    files: &[(FileMetaLocal, u64)],
    writer: &mut EncryptedStream<impl Write>,
    progress_callback: Option<impl FnMut(&TransferReport)>,
) -> Result<(), Error> {
    // sum up total transfer size
    let mut total_bytes = 0;
    for (file, start) in files {
        total_bytes += file
            .len
            .checked_sub(*start)
            .ok_or(Error::InvalidStartIndex)?;
    }

    // Wrap the writer to report progress over `progress_tx`
    let mut writer = ProgressWrapper::new(
        BufWriter::with_capacity(FILE_BUFFER_SIZE, writer),
        total_bytes,
        files.len() as u64,
        progress_callback,
    );

    // iterate over all the files
    for (offer, start) in files {
        // report the file path
        writer.progress.current_file.clone_from(&offer.short_path);

        let mut file = File::open(&offer.local_path)?;

        // confirm file length matches metadata length
        if file.metadata()?.len() != offer.len {
            return Err(Error::UnexpectedFileLen);
        }

        // copy the file into the writer
        file.seek(SeekFrom::Start(*start))?;
        std::io::copy(&mut file, &mut writer)?;

        // report the number of processed files
        writer.progress.processed_files += 1;
    }

    writer.flush()?;

    Ok(())
}

/// Sequentially save the requested `files` from `reader`.
///
/// `files` is a slice of tuples:
/// (file offer I received: [`FileMetaLocal`], my response: [`Option<u64>`])
///
/// `progress_callback` gets repeatedly called when writing to report progress.
/// These arguments are passed to it:
/// - `&str` - `short_path` of the file currently being sent.
/// - `u64` - The number of bytes read so far.
/// - `u64` - The total number of bytes to read.
pub fn receive_these_files(
    files: &[(FileMeta, u64)],
    reader: &mut EncryptedStream<impl Read>,
    progress_callback: Option<impl FnMut(&TransferReport)>,
) -> Result<(), Error> {
    // sum up total transfer size
    let mut total_bytes = 0;
    for (file, start) in files {
        total_bytes += file
            .len
            .checked_sub(*start)
            .ok_or(Error::InvalidStartIndex)?;
    }

    // Wrap the reader to report progress over `progress_tx`
    let mut reader =
        ProgressWrapper::new(reader, total_bytes, files.len() as u64, progress_callback);

    // iterate over all the files
    for (offer, start) in files {
        // set progress bar message to file path
        reader.progress.current_file.clone_from(&offer.short_path);

        // get the partial download path
        let tmp_path = offer.get_partial_download_path()?;

        // download whole file
        if *start == 0 {
            // create a directory and TMP file
            if let Some(parent) = tmp_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let file = File::create(&tmp_path)?;

            // buffer the writer
            let buf_size = std::cmp::min(FILE_BUFFER_SIZE, offer.len as usize);
            let mut file = BufWriter::with_capacity(buf_size, file);

            // only take the length of the file from the reader
            let mut reader = (&mut reader).take(offer.len);

            // copy from the reader into the file
            std::io::copy(&mut reader, &mut file)?;

        // resume interrupted download
        } else {
            // open the partially downloaded file in append mode
            let file = OpenOptions::new().append(true).open(&tmp_path)?;
            if file.metadata()?.len() != *start {
                return Err(Error::UnexpectedFileLen);
            }

            // buffer the writer
            let buf_size = std::cmp::min(FILE_BUFFER_SIZE, offer.len as usize - *start as usize);
            let mut file = BufWriter::with_capacity(buf_size, file);

            // only take the length of the remaining part of the file from the reader
            let mut reader = (&mut reader).take(offer.len - start);

            // copy from the reader into the file
            std::io::copy(&mut reader, &mut file)?;
        }
        reader.progress.processed_files += 1;
        std::fs::rename(tmp_path, offer.get_unoccupied_save_path()?)?;
    }

    Ok(())
}

/// Wraps an IO stream. Calls a callback function
/// with the number of bytes read/written out of a total.
///
/// `progress_callback` gets repeatedly called when writing to report progress.
/// These arguments are passed to it:
/// - `&str` - `short_path` of the file currently being sent.
/// - `u64` - The number of bytes read so far.
/// - `u64` - The total number of bytes to read.
struct ProgressWrapper<T, F: FnMut(&TransferReport)> {
    /// The callback function called to report progress
    progress_callback: Option<F>,

    /// The inner IO stream
    inner_io: T,

    /// The current progress of the file transfer.
    progress: TransferReport,
}

impl<T, F: FnMut(&TransferReport)> ProgressWrapper<T, F> {
    fn new(inner_io: T, total_bytes: u64, total_files: u64, progress_callback: Option<F>) -> Self {
        Self {
            progress_callback,
            inner_io,
            progress: TransferReport {
                processed_bytes: 0,
                total_bytes,
                processed_files: 0,
                total_files,
                current_file: "".into(),
            },
        }
    }

    /// Increment the number of bytes processed
    fn inc_bytes_processed(&mut self, bytes: usize) {
        if let Some(progress_callback) = &mut self.progress_callback {
            self.progress.processed_bytes += bytes as u64;
            (progress_callback)(&self.progress)
        }
    }
}

impl<T: Write, F: FnMut(&TransferReport)> Write for ProgressWrapper<T, F> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let amt = self.inner_io.write(buf)?;
        self.inc_bytes_processed(amt);
        Ok(amt)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner_io.flush()
    }
}

impl<T: Read, F: FnMut(&TransferReport)> Read for ProgressWrapper<T, F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let amt = self.inner_io.read(buf)?;
        self.inc_bytes_processed(amt);
        Ok(amt)
    }
}

impl<T: BufRead, F: FnMut(&TransferReport)> BufRead for ProgressWrapper<T, F> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.inner_io.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner_io.consume(amt);
        self.inc_bytes_processed(amt);
    }
}
