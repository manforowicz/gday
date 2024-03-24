use gday_file_offer_protocol::{FileMeta, FileMetaLocal};
use gday_encryption::EncryptedStream;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

use crate::TMP_DOWNLOAD_PREFIX;

/// Wrap a [`TcpStream`] in a [`gday_encryption::EncryptedStream`].
pub fn encrypt_connection<T: Read + Write>(
    mut io_stream: T,
    shared_key: &[u8; 32],
    is_creator: bool,
) -> std::io::Result<EncryptedStream<T>> {
    // send and receive nonces over unencrypted TCP

    let nonce = if is_creator {
        let nonce: [u8; 7] = rand::random();
        io_stream.write_all(&nonce)?;
        io_stream.flush()?;
        nonce
    } else {
        let mut nonce = [0_u8; 7];
        io_stream.read_exact(&mut nonce)?;
        nonce
    };

    Ok(EncryptedStream::new(io_stream, shared_key, &nonce))
}

/// Sequentially write the given files to this `writer`.
pub fn send_files(
    writer: &mut impl Write,
    offered: &[FileMetaLocal],
    accepted: &[Option<u64>],
) -> std::io::Result<()> {
    // sum up total transfer size
    let size: u64 = offered
        .iter()
        .zip(accepted)
        .map(|(offer, response)| {
            if let Some(start) = response {
                offer.len - start
            } else {
                0
            }
        })
        .sum();

    // create a progress bar object
    let progress = create_progress_bar(size);

    // iterate over all the files
    for (meta, &accepted) in offered.iter().zip(accepted) {
        if let Some(start) = accepted {
            // set progress bar message to file path
            let msg = meta.short_path.display();
            progress.set_message(format!("sending {msg}"));

            // copy the file into the writer
            let mut file = File::open(&meta.local_path)?;
            // TODO: maybe check if file length is correct?

            file.seek(SeekFrom::Start(start))?;
            let mut writer = ProgressWrite {
                writer,
                progress: &progress,
            };
            std::io::copy(&mut file, &mut writer)?;
        }
    }

    // flush the writer
    writer.flush()?;

    progress.finish_with_message("Done sending!");

    Ok(())
}

/// Sequentially save the given `files` from this `reader`.
pub fn receive_files(
    reader: &mut impl Read,
    offered: &[FileMeta],
    accepted: &[Option<u64>],
) -> Result<(), gday_file_offer_protocol::Error> {
    // sum up total transfer size
    let size: u64 = offered
        .iter()
        .zip(accepted)
        .map(|(f, a)| {
            if let Some(start) = a {
                f.len - start
            } else {
                0
            }
        })
        .sum();

    let progress = create_progress_bar(size);

    // iterate over all the files
    for (meta, &accepted) in offered.iter().zip(accepted) {
        // download whole file
        if let Some(0) = accepted {
            // set progress bar message to file path
            let msg = meta.short_path.display();
            progress.set_message(format!("receiving {msg}"));

            // get this file's save path
            let save_path = meta.get_save_path()?;

            // create a directory for this file if it's missing
            if let Some(parent) = save_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let tmp_path = meta.get_prefixed_save_path(TMP_DOWNLOAD_PREFIX.into())?;

            // create the temporary download file
            let mut file = File::create(&tmp_path)?;

            // only take the length of the file from the reader
            let mut reader = reader.take(meta.len);

            // wrap the file to track write progress
            let mut writer = ProgressWrite {
                writer: &mut file,
                progress: &progress,
            };

            // copy from the reader into the file
            std::io::copy(&mut reader, &mut writer)?;

            // rename the temporary download file to its final name
            std::fs::rename(tmp_path, meta.get_unused_save_path()?)?;

        // resume interrupted download
        } else if let Some(start) = accepted {
            // set progress bar message to file path
            let msg = meta.short_path.display();
            progress.set_message(format!("receiving {msg}"));

            // TODO: ENSURE THE SAVED FILE IS ACTUALLY 'start' BYTES LONG

            // open the partially downloaded file in append mode
            let tmp_path = meta.get_prefixed_save_path(TMP_DOWNLOAD_PREFIX.into())?;
            let mut file = OpenOptions::new().append(true).open(&tmp_path).unwrap();

            // only take the length of the remaining part of the file from the reader
            let mut reader = reader.take(meta.len - start);

            // wrap the file to track write progress
            let mut writer = ProgressWrite {
                writer: &mut file,
                progress: &progress,
            };

            // copy from the reader into the file
            std::io::copy(&mut reader, &mut writer)?;

            // rename the temporary download file to its final name
            let save_path = meta.get_save_path()?;
            std::fs::rename(tmp_path, save_path)?;
        }

        progress.finish_with_message("Done downloading!");
    }

    Ok(())
}

/// Create a stylded [`ProgressBar`].
fn create_progress_bar(bytes: u64) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{msg} [{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | eta: {eta}",
    )
    .unwrap();
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(Some(bytes), draw)
        .with_style(style)
        .with_message("starting...")
}

/// A thin wrapper around a [`Write`] IO stream and [`ProgressBar`].
/// Increments the [`ProgressBar`] by the number of bytes written.
struct ProgressWrite<'a, T: Write> {
    writer: &'a mut T,
    progress: &'a ProgressBar,
}

impl<'a, T: Write> Write for ProgressWrite<'a, T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let bytes_written = self.writer.write(buf)?;

        self.progress.inc(bytes_written as u64);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}
