use std::sync::{Arc, Mutex, MutexGuard};

/// Logs to the log box in the UI.
#[derive(Clone, Debug)]
pub struct Logger {
    text: Arc<Mutex<String>>,
}

impl Logger {
    pub fn init() -> Self {
        let logger = Self {
            text: Arc::new(Mutex::new(String::new())),
        };

        log::set_boxed_logger(Box::new(logger.clone())).unwrap();
        log::set_max_level(log::LevelFilter::Debug);

        logger
    }
    pub fn get_log<'a>(&'a self) -> MutexGuard<'a, String> {
        self.text.lock().unwrap()
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "[{}] [{}] {}\n",
            record.level(),
            record.target(),
            record.args()
        );
        self.text.lock().unwrap().push_str(&line);
    }

    fn flush(&self) {}
}
