use crate::TMP_DOWNLOAD_PREFIX;
use gday_encryption::EncryptedStream;
use gday_file_offer_protocol::{FileMeta, FileMetaLocal};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};

const FILE_BUFFER_SIZE: usize = 1_000_000;

/// Wrap a [`std::net::TcpStream`] in a [`gday_encryption::EncryptedStream`].
/// - The creator sends a random nonce, which the other peer receives and uses
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
    files: &[(FileMetaLocal, Option<u64>)],
) -> std::io::Result<()> {
    // sum up total transfer size,
    // for display on the progress bar
    let size: u64 = files
        .iter()
        .map(|(offer, response)| {
            if let Some(start) = response {
                offer.len - start
            } else {
                0
            }
        })
        .sum();
    let progress = create_progress_bar(size);
    let mut writer = progress.wrap_write(writer);

    // for all files
    for (offer, response) in files {
        if let Some(start) = response {
            // set progress bar message to file path
            let msg = offer.short_path.display();
            progress.set_message(format!("sending {msg}"));

            let file = File::open(&offer.local_path)?;
            if file.metadata()?.len() != offer.len {
                todo!("Throw an error!");
            }

            // copy the file into the writer
            let mut file = BufReader::with_capacity(FILE_BUFFER_SIZE, file);

            file.seek(SeekFrom::Start(*start))?;
            std::io::copy(&mut file, &mut writer)?;
        }
    }

    writer.flush()?;
    progress.finish_with_message("Done sending!");

    Ok(())
}

/// Sequentially save the given `files` from this `reader`.
pub fn receive_files(
    reader: &mut impl BufRead,
    files: &[(FileMeta, Option<u64>)],
) -> Result<(), gday_file_offer_protocol::Error> {
    // sum up total transfer size
    let size: u64 = files
        .iter()
        .map(|(offer, response)| {
            if let Some(start) = response {
                offer.len - start
            } else {
                0
            }
        })
        .sum();

    let progress = create_progress_bar(size);

    // iterate over all the files
    for (offer, response) in files {
        // download whole file
        if let Some(0) = response {
            // set progress bar message to file path
            let msg = offer.short_path.display();
            progress.set_message(format!("receiving {msg}"));

            // get this file's save path
            let final_save_path = offer.get_unused_save_path()?;

            // create a directory for this file if it's missing
            if let Some(parent) = final_save_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let tmp_path = offer.get_prefixed_save_path(TMP_DOWNLOAD_PREFIX.into())?;

            // create the temporary download file
            let mut file = BufWriter::with_capacity(FILE_BUFFER_SIZE, File::create(&tmp_path)?);

            // only take the length of the file from the reader
            let mut reader = reader.take(offer.len);

            // wrap the writer in progress bar
            let mut writer = progress.wrap_write(&mut file);

            // copy from the reader into the file
            std::io::copy(&mut reader, &mut writer)?;

            // rename the temporary download file to its final name
            std::fs::rename(tmp_path, final_save_path)?;

        // resume interrupted download
        } else if let Some(start) = response {
            // set progress bar message to file path
            let msg = offer.short_path.display();
            progress.set_message(format!("receiving {msg}"));

            // open the partially downloaded file in append mode
            let tmp_path = offer.get_prefixed_save_path(TMP_DOWNLOAD_PREFIX.into())?;
            let file = OpenOptions::new().append(true).open(&tmp_path)?;
            if file.metadata()?.len() != *start {
                todo!("Throw error");
            }
            let file = BufWriter::with_capacity(FILE_BUFFER_SIZE, file);

            // only take the length of the remaining part of the file from the reader
            let mut reader = reader.take(offer.len - start);

            // wrap the writer in progress bar
            let mut writer = progress.wrap_write(file);

            // copy from the reader into the file
            std::io::copy(&mut reader, &mut writer)?;

            // rename the temporary download file to its final name
            let save_path = offer.get_unused_save_path()?;
            std::fs::rename(tmp_path, save_path)?;
        }
    }

    progress.finish_with_message("Done downloading!");

    Ok(())
}

/// Create a stylded [`ProgressBar`].
fn create_progress_bar(bytes: u64) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{msg} [{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | eta: {eta}",
    )
    .expect("Progress bar style string was invalid.");
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(Some(bytes), draw)
        .with_style(style)
        .with_message("starting...")
}
