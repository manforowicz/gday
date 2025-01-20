use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::partial_download::TmpInfoFile;
use crate::{
    delete_tmp_info_file, get_download_path, get_unoccupied_version, write_tmp_info_file, Error,
    FileOfferMsg, FileRequestMsg, LocalFileOffer, TMP_DOWNLOAD_FILE,
};
use std::io::{ErrorKind, Seek, SeekFrom};
use std::path::Path;
use std::pin::{pin, Pin};
use std::task::{ready, Context, Poll};

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
/// - `offer` is the `Vec` of [`FileMetaLocal`] you sent to your peer.
/// - `response` is the [`FileResponseMsg`] received from your peer.
/// - `writer` is the the IO stream on which the files will be sent.
/// - `progress_callback` is a function that gets frequently
///   called with [`TransferReport`] to report progress.
///
/// Transfers the accepted files in order, sequentially, back-to-back.
pub async fn send_files(
    offer: &LocalFileOffer,
    request: &FileRequestMsg,
    writer: impl AsyncWrite,
    progress_callback: impl FnMut(&TransferReport),
) -> Result<(), Error> {
    let writer = pin!(writer);
    let files = offer.offer.lookup_request(request)?;
    let total_bytes = offer.offer.get_transfer_size(request)?;

    // Wrap the writer to report progress over `progress_tx`
    let mut writer =
        ProgressWrapper::new(writer, total_bytes, files.len() as u64, progress_callback);

    // 64 KiB copy buffer
    let mut buf = vec![0; 0x10000];

    // iterate over all the files
    for (request, metadata) in files {
        // report the file path
        writer.progress.current_file.clone_from(&request.path);

        let mut file = std::fs::File::open(&offer.offered_path_to_local[&request.path])?;

        // confirm file length matches metadata length
        if file.metadata()?.len() != metadata.size {
            return Err(Error::UnexpectedFileLen);
        }

        // copy the file into the writer
        file.seek(SeekFrom::Start(request.start_offset))?;

        file_to_net(
            &mut file,
            &mut writer,
            metadata.size - request.start_offset,
            &mut buf,
        )
        .await?;

        // report the number of processed files
        writer.progress.processed_files += 1;
    }

    writer.flush().await?;

    Ok(())
}

/// Receives the requested files from `reader`.
///
/// - `offer` is the [`FileOfferMsg`] offered by the peer.
/// - `response` is the [`FileResponseMsg`] that you've sent in response.
/// - `save_path` is the directory where the files should be saved.
/// - `reader` is the IO stream on which the files will be received.
/// - `progress_callback` is an function that gets frequently
///   called with [`TransferReport`] to report progress.
///
/// The accepted files must be sent in order, sequentially, back-to-back.
pub async fn receive_files(
    offer: &FileOfferMsg,
    request: &FileRequestMsg,
    save_path: &Path,
    reader: impl AsyncBufRead,
    progress_callback: impl FnMut(&TransferReport),
) -> Result<(), Error> {
    let reader = pin!(reader);
    let files = offer.lookup_request(request)?;
    let total_bytes = offer.get_transfer_size(request)?;

    // Wrap the reader to report progress over `progress_tx`
    let mut reader =
        ProgressWrapper::new(reader, total_bytes, files.len() as u64, progress_callback);

    // iterate over all the files
    for (request, metadata) in files {
        // set progress bar message to file path
        reader.progress.current_file.clone_from(&request.path);

        write_tmp_info_file(
            save_path,
            &TmpInfoFile {
                file_short_path: request.path.clone(),
                file_metadata: metadata.clone(),
            },
        )?;

        // download whole file
        if request.start_offset != 0 {
            // open the partially downloaded file in append mode
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(save_path.join(TMP_DOWNLOAD_FILE))?;
            if file.metadata()?.len() != request.start_offset {
                return Err(Error::UnexpectedFileLen);
            }

            net_to_file(&mut reader, &mut file, metadata.size - request.start_offset).await?;
        } else {
            // create a directory and TMP file
            let mut file = std::fs::File::create(save_path.join(TMP_DOWNLOAD_FILE))?;

            // copy from the reader into the file
            net_to_file(&mut reader, &mut file, metadata.size).await?;

            // resume interrupted download
        }

        let final_save_path =
            get_unoccupied_version(&get_download_path(save_path, &request.path)?)?;
        if let Some(parent) = final_save_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(save_path.join(TMP_DOWNLOAD_FILE), final_save_path)?;

        delete_tmp_info_file(save_path)?;

        reader.progress.processed_files += 1;
    }

    Ok(())
}

/// We're using this instead of [`tokio::io::copy()`].
///
/// [`tokio::io::copy()`] spawns a task on a thread
/// during every file read/write. This occurs 1000s of times,
/// introducing unnecessary overhead.
///
/// This function is similar, but uses standard blocking
/// reads from `src`. This is made on the assumption that each read
/// won't block everything for too long, so this
/// function should still be cancellable.
async fn file_to_net(
    mut src: impl std::io::Read,
    mut dst: impl tokio::io::AsyncWrite + Unpin,
    mut amt: u64,
    buf: &mut [u8],
) -> std::io::Result<()> {
    while amt > 0 {
        let to_read = std::cmp::min(amt, buf.len() as u64) as usize;
        let bytes_read = src.read(&mut buf[0..to_read])?;
        if bytes_read == 0 {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "Peer interrupted transfer.",
            ));
        }
        amt -= bytes_read as u64;
        dst.write_all(&buf[0..to_read]).await?;
    }
    Ok(())
}

/// We're using this instead of [`tokio::io::copy_buf()`].
///
/// [`tokio::io::copy_buf()`] spawns a task on a thread
/// during every file read/write. This occurs 1000s of times,
/// introducing unnecessary overhead.
///
/// This function is similar, but uses standard blocking
/// writes to `dst`. This is made on the assumption that each write
/// won't block everything for too long, so this
/// function should still be cancellable.
async fn net_to_file(
    mut src: impl tokio::io::AsyncBufRead + Unpin,
    mut dst: impl std::io::Write,
    mut amt: u64,
) -> std::io::Result<()> {
    while amt > 0 {
        let buf = src.fill_buf().await?;
        if buf.is_empty() {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "Peer interrupted transfer.",
            ));
        }
        let to_write = std::cmp::min(amt, buf.len() as u64) as usize;
        let written = dst.write(&buf[0..to_write])?;
        src.consume(written);
        amt -= written as u64;
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
