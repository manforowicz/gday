use tokio::io::{
    AsyncBufRead, AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufWriter,
};

use crate::{Error, FileMeta, FileMetaLocal, FileOfferMsg, FileResponseMsg};
use std::io::SeekFrom;
use std::path::Path;
use std::pin::{pin, Pin};
use std::task::{ready, Context, Poll};

const FILE_BUFFER_SIZE: usize = 1_000_000;

/// Holds the status of a file transfer
#[derive(Debug, Clone)]
pub struct TransferReport {
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub processed_files: u64,
    pub total_files: u64,
    pub current_file: std::path::PathBuf,
}

/// Transfers the requested files to `writer`.
///
/// - `offer` is the `Vec` of [`FileMetaLocal`] offered to the peer.
/// - `response` is the peer's [`FileResponseMsg`].
/// - `progress_callback` is a function that gets frequently
///   called with [`TransferReport`] to report progress.
///
/// Transfers the accepted files in order, sequentially, back-to-back.
pub async fn send_files(
    offer: &[FileMetaLocal],
    response: &FileResponseMsg,
    writer: impl AsyncWrite,
    progress_callback: impl FnMut(&TransferReport),
) -> Result<(), Error> {
    let writer = pin!(writer);
    let files: Vec<(&FileMetaLocal, u64)> = offer
        .iter()
        .zip(&response.response)
        .filter_map(|(file, response)| response.map(|response| (file, response)))
        .collect();

    // sum up total transfer size
    let mut total_bytes = 0;
    for (file, start) in &files {
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

        let mut file = tokio::fs::File::open(&offer.local_path).await?;

        // confirm file length matches metadata length
        if file.metadata().await?.len() != offer.len {
            return Err(Error::UnexpectedFileLen);
        }

        // copy the file into the writer
        file.seek(SeekFrom::Start(start)).await?;
        tokio::io::copy(&mut file, &mut writer).await?;

        // report the number of processed files
        writer.progress.processed_files += 1;
    }

    writer.flush().await?;

    Ok(())
}

/// Receives the requested files from `reader`.
///
/// - `offer` is the [`FileOfferMsg`] offered by the peer.
/// - `response` is your corresponding [`FileResponseMsg`].
/// - `save_path` is the directory where the files should be saved.
/// - `reader` is the IO stream on which the files will be received.
/// - `progress_callback` is an function that gets frequently
///   called with [`TransferReport`] to report progress.
///
/// The accepted files must be sent in order, sequentially, back-to-back.
pub async fn receive_files(
    offer: &FileOfferMsg,
    response: &FileResponseMsg,
    save_path: &Path,
    reader: impl AsyncRead,
    progress_callback: impl FnMut(&TransferReport),
) -> Result<(), Error> {
    let reader = pin!(reader);
    let files: Vec<(&FileMeta, u64)> = offer
        .files
        .iter()
        .zip(&response.response)
        .filter_map(|(file, response)| response.map(|response| (file, response)))
        .collect();

    // sum up total transfer size
    let mut total_bytes = 0;
    for (file, start) in &files {
        total_bytes += file
            .len
            .checked_sub(*start)
            .ok_or(Error::InvalidStartIndex)?;
    }

    // Wrap the reader to report progress over `progress_tx`
    let mut reader = ProgressWrapper::new(
        tokio::io::BufReader::with_capacity(FILE_BUFFER_SIZE, reader),
        total_bytes,
        files.len() as u64,
        progress_callback,
    );

    // iterate over all the files
    for (offer, start) in files {
        // set progress bar message to file path
        reader.progress.current_file.clone_from(&offer.short_path);

        // get the partial download path
        let tmp_path = offer.get_partial_download_path(save_path)?;

        // download whole file
        if start == 0 {
            // create a directory and TMP file
            if let Some(parent) = tmp_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let mut file = tokio::fs::File::create(&tmp_path).await?;

            // only take the length of the file from the reader
            let mut reader = (&mut reader).take(offer.len);

            // copy from the reader into the file
            tokio::io::copy(&mut reader, &mut file).await?;

        // resume interrupted download
        } else {
            // open the partially downloaded file in append mode
            let mut file = tokio::fs::OpenOptions::new()
                .append(true)
                .open(&tmp_path)
                .await?;
            if file.metadata().await?.len() != start {
                return Err(Error::UnexpectedFileLen);
            }

            // only take the length of the remaining part of the file from the reader
            let mut reader = (&mut reader).take(offer.len - start);

            // copy from the reader into the file
            tokio::io::copy(&mut reader, &mut file).await?;
        }
        reader.progress.processed_files += 1;
        tokio::fs::rename(tmp_path, offer.get_unoccupied_save_path(save_path).await?).await?;
    }

    Ok(())
}

/// Wraps an IO stream. Calls `progress_callback` on each
/// read/write to report progress.
#[pin_project::pin_project]
struct ProgressWrapper<T, F: FnMut(&TransferReport)> {
    /// The callback function called to report progress
    progress_callback: F,

    /// The inner IO stream
    #[pin]
    inner_io: T,

    /// The current progress of the file transfer.
    progress: TransferReport,
}

impl<T, F: FnMut(&TransferReport)> ProgressWrapper<T, F> {
    fn new(inner_io: T, total_bytes: u64, total_files: u64, progress_callback: F) -> Self {
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
}

impl<T: AsyncWrite, F: FnMut(&TransferReport)> AsyncWrite for ProgressWrapper<T, F> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let me = self.project();
        let amt = ready!(me.inner_io.poll_write(cx, buf))?;
        me.progress.processed_bytes += amt as u64;
        (me.progress_callback)(me.progress);
        Poll::Ready(Ok(amt))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        self.project().inner_io.poll_flush(cx)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        self.project().inner_io.poll_shutdown(cx)
    }
}

impl<T: AsyncRead, F: FnMut(&TransferReport)> AsyncRead for ProgressWrapper<T, F> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let me = self.project();
        let filled = buf.filled().len();
        ready!(me.inner_io.poll_read(cx, buf))?;
        me.progress.processed_bytes += (buf.filled().len() - filled) as u64;
        (me.progress_callback)(me.progress);
        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncBufRead, F: FnMut(&TransferReport)> AsyncBufRead for ProgressWrapper<T, F> {
    fn consume(self: Pin<&mut Self>, amt: usize) {
        let me = self.project();
        me.inner_io.consume(amt);
        me.progress.processed_bytes += amt as u64;
        (me.progress_callback)(me.progress);
    }

    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        self.project().inner_io.poll_fill_buf(cx)
    }
}
