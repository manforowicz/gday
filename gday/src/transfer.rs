use gday_encryption::EncryptedStream;
use gday_file_transfer::{FileMetaLocal, FileOfferMsg, FileResponseMsg, TransferReport};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Sequentially write the given files to this `writer`.
pub async fn send_files(
    offer: Vec<FileMetaLocal>,
    response: FileResponseMsg,
    writer: &mut EncryptedStream<tokio::net::TcpStream>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    match gday_file_transfer::send_files(&offer, &response, writer, update_progress).await {
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
///
/// `save_dir` is the directory where the files
/// will be saved.
pub async fn receive_files(
    offer: FileOfferMsg,
    response: FileResponseMsg,
    save_dir: &std::path::Path,
    reader: &mut EncryptedStream<tokio::net::TcpStream>,
) -> Result<(), Box<dyn std::error::Error>> {
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

    let result =
        gday_file_transfer::receive_files(&offer, &response, save_dir, reader, update_progress)
            .await;

    match result {
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
