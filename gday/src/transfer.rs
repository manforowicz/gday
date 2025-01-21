use gday_encryption::EncryptedStream;
use gday_file_transfer::{FileOfferMsg, FileRequestsMsg, LocalFileOffer, TransferReport};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// Sequentially write the given files to this `writer`.
pub async fn send_files(
    offer: LocalFileOffer,
    response: FileRequestsMsg,
    writer: &mut EncryptedStream<tokio::net::TcpStream>,
) -> Result<(), Box<dyn std::error::Error>> {
    let len = offer.offer.get_transfer_size(&response)?;
    let progress_bar = create_progress_bar(len);
    let mut current_file = String::from("Starting...");

    let update_progress = |report: &TransferReport| {
        progress_bar.set_position(report.processed_bytes);
        if current_file.as_str() != report.current_file.to_string_lossy() {
            current_file.clear();
            current_file.push_str(&report.current_file.to_string_lossy());
            progress_bar.set_message(format!("Sending {}", current_file));
        }
    };

    match gday_file_transfer::send_files(&offer, &response, writer, update_progress).await {
        Ok(()) => {
            progress_bar.finish_with_message("Transfer complete.");
            Ok(())
        }
        Err(err) => {
            progress_bar.abandon_with_message("Send failed.");
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
    response: FileRequestsMsg,
    save_dir: &std::path::Path,
    reader: &mut EncryptedStream<tokio::net::TcpStream>,
) -> Result<(), Box<dyn std::error::Error>> {
    let len = offer.get_transfer_size(&response)?;
    let progress_bar = create_progress_bar(len);
    let mut current_file = String::new();

    let update_progress = |report: &TransferReport| {
        progress_bar.set_position(report.processed_bytes);
        if current_file.as_str() != report.current_file.to_string_lossy() {
            current_file.clear();
            current_file.push_str(&report.current_file.to_string_lossy());
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
            progress_bar.abandon_with_message("Receive failed.");
            Err(err.into())
        }
    }
}

/// Create a stylded [`ProgressBar`].
fn create_progress_bar(len: u64) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{msg} [{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | eta: {eta}",
    )
    .expect("Progress bar style string was invalid.");
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(Some(len), draw)
        .with_style(style)
        .with_message("starting...")
}
