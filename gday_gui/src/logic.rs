use std::sync::mpsc;

use gday_encryption::EncryptedStream;
use gday_hole_punch::{server_connector, ContactSharer};
use log::info;

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Call [`ChannelLogger::init()`] to initialize [`log`] with this
/// logger.
///
/// All messages logged with [`log`] will be sent to an [`std::sync::mpsc::Receiver`]
/// returned.
struct ChannelLogger {
    tx: std::sync::mpsc::SyncSender<(log::Level, String)>,
}

impl ChannelLogger {
    /// All messages logged with [`log`] will be sent to the [`std::sync::mpsc::Receiver`]
    /// returned.
    ///
    /// Panics if a [`log`] logger has already been set.
    fn init() -> mpsc::Receiver<(log::Level, String)> {
        let (tx, rx) = mpsc::sync_channel(10);
        log::set_boxed_logger(Box::new(Self { tx }))
            .expect("Another logger has already been initialized.");
        rx
    }
}

impl log::Log for ChannelLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() >= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let _ = self
                .tx
                .try_send((record.level(), record.args().to_string()));
        }
    }

    fn flush(&self) {}
}

pub fn connect_to_peer(
    peer_code: gday_hole_punch::PeerCode,
    custom_server: Option<(String, u16, bool)>,
    is_creator: bool,
) -> Result<EncryptedStream<std::net::TcpStream>, Box<dyn std::error::Error>> {
    todo!()
}
