use log::{LevelFilter, Metadata, Record};

use crate::console::kprintln;

struct KernelLogger;

static LOGGER: KernelLogger = KernelLogger;

impl log::Log for KernelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            kprintln!("[{}] {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

pub unsafe fn init_logger() {
    log::set_logger_racy(&LOGGER)
        .map(|()| {
            let mut log_level = match option_env!("LOG_LEVEL") {
                Some(level) => {
                    match level {
                        "ERROR" | "error" | "Error" => LevelFilter::Error,
                        "WARN" | "warn" | "Warn" => LevelFilter::Warn,
                        "INFO" | "info" | "Info" => LevelFilter::Info,
                        "DEBUG" | "debug" | "Debug" => LevelFilter::Debug,
                        "TRACE" | "trace" | "Trace" => LevelFilter::Trace,
                        "OFF" | "off" | "Off" => LevelFilter::Off,
                        _level => LevelFilter::Info,
                    }
                }
                None => LevelFilter::Info,
            };
            log::set_max_level(log_level)
        })
        .expect("Failed to initialize the logger");

}
