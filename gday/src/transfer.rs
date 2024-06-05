use gday_encryption::EncryptedStream;
use gday_file_transfer::{FileMeta, FileMetaLocal, TransferReport};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Sequentially write the given files to this `writer`.
pub fn send_files(
    writer: &mut EncryptedStream<std::net::TcpStream>,
    files: &[(FileMetaLocal, Option<u64>)],
) -> Result<(), Error> {
    let progress_bar = create_progress_bar();
    let mut current_file = String::from("Starting...");

    let update_progress = |report: &TransferReport| {
        progress_bar.set_position(report.processed_bytes);
        progress_bar.set_length(report.total_bytes);
        if current_file.as_str() != report.current_file.to_string_lossy() {
            current_file = report.current_file.to_string_lossy().to_string();
            progress_bar.set_message(format!("Receiving {}", current_file));
        }
    };

    match gday_file_transfer::send_files(writer, files, Some(update_progress)) {
        Ok(()) => {
            progress_bar.finish_with_message("Transfer complete.");
            Ok(())
        }
        Err(err) => {
            progress_bar.abandon_with_message("Transfer failed.");
            Err(err.into())
        }
    }
}

/// Sequentially save the given `files` from this `reader`.
pub fn receive_files(
    reader: &mut EncryptedStream<std::net::TcpStream>,
    files: &[(FileMeta, Option<u64>)],
) -> Result<(), Error> {
    let progress_bar = create_progress_bar();
    let mut current_file = String::from("Starting...");

    let update_progress = |report: &TransferReport| {
        progress_bar.set_position(report.processed_bytes);
        progress_bar.set_length(report.total_bytes);
        if current_file.as_str() != report.current_file.to_string_lossy() {
            current_file = report.current_file.to_string_lossy().to_string();
            progress_bar.set_message(format!("Receiving {}", current_file));
        }
    };

    match gday_file_transfer::receive_files(reader, files, Some(update_progress)) {
        Ok(()) => {
            progress_bar.finish_with_message("Transfer complete.");
            Ok(())
        }
        Err(err) => {
            progress_bar.abandon_with_message("Transfer failed.");
            Err(err.into())
        }
    }
}

/// Create a stylded [`ProgressBar`].
fn create_progress_bar() -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{msg} [{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | eta: {eta}",
    )
    .expect("Progress bar style string was invalid.");
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(None, draw)
        .with_style(style)
        .with_message("starting...")
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Couldn't find an empty save path for a file
    #[error("Error transfering files: {0}")]
    FileTransferError(#[from] gday_file_transfer::Error),

    /// IO Error
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
}
