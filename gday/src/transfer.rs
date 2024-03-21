#![warn(clippy::all)]
use crate::protocol::{FileMeta, FileMetaLocal};
use gday_encryption::EncryptedStream;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::fs::File;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

/// Wrap a [`TcpStream`] in an [`EncryptedStream`].
fn encrypt_connection(
    mut tcp_stream: TcpStream,
    shared_key: &[u8; 32],
) -> std::io::Result<EncryptedStream<TcpStream>> {
    // send and receive nonces over unencrypted TCP
    let write_nonce: [u8; 7] = rand::random();
    tcp_stream.write_all(&write_nonce)?;
    let mut read_nonce = [0; 7];
    tcp_stream.read_exact(&mut read_nonce)?;

    Ok(EncryptedStream::new(tcp_stream, shared_key, &write_nonce))
}

/// Sequentially write the given `files` to this `writer`.
pub fn send_files(writer: &mut impl Write, files: &[FileMetaLocal]) -> std::io::Result<()> {
    // create a progress bar object
    let size: u64 = files.iter().map(|meta| meta.len).sum();
    let progress = create_progress_bar(size);

    // iterate over all the files
    for meta in files {
        // set progress bar message to file path
        let msg = meta.short_path.to_string_lossy().to_string();
        progress.set_message(msg);

        // copy the file into the writer
        let mut file = File::open(&meta.local_path)?;
        let mut writer = ProgressWrite {
            writer,
            progress: &progress,
        };
        std::io::copy(&mut file, &mut writer)?;
    }

    // flush the writer
    writer.flush()?;
    Ok(())
}

/// Sequentially save the given `files`` from this `reader`.
pub fn receive_files(reader: &mut impl Read, files: &[FileMeta]) -> std::io::Result<()> {
    // create a progress bar object
    let total_len: u64 = files.iter().map(|meta| meta.len).sum();
    let progress = create_progress_bar(total_len);

    // iterate over all the files
    for meta in files {
        // set progress bar message to file path
        let msg = meta.short_path.to_string_lossy().to_string();
        progress.set_message(format!("receiving {msg}"));

        // get this file's save path
        let save_path = meta.get_save_path()?;

        // create a directory for this file if it's missing
        let parent = save_path.parent().unwrap_or(Path::new(""));
        std::fs::create_dir_all(parent)?;

        // get a temporary download path
        let save_file_name = save_path.file_name().expect("Path terminates in .. ?");
        let tmp_name = Path::new("PARTIAL_GDAY_DOWNLOAD").join(save_file_name);
        let tmp_path = parent.join(tmp_name);

        // create the temporary download file
        let mut file = File::create(&tmp_path)?;

        // copy from the reader into the file
        let mut reader = reader.take(meta.len);
        let mut writer = ProgressWrite {
            writer: &mut file,
            progress: &progress,
        };
        std::io::copy(&mut reader, &mut writer)?;

        // rename the file to its intended name
        std::fs::rename(tmp_path, save_path)?;
    }

    Ok(())
}

fn create_progress_bar(bytes: u64) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{msg} [{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | eta: {eta}",
    )
    .unwrap();
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(Some(bytes), draw)
        .with_style(style)
        .with_message("Starting")
}

/// A thin wrapper around a [`Write`] IO stream and [`ProgressBar`].
/// Increments the [`ProgressBar`] when writting.
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
