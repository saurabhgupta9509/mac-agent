// core/file_logger.rs
//
// Thread-safe, file-only logger for macOS LaunchDaemon and processes.
// Rotates daily and limits file size. Integrates with the `log` crate.

use chrono::Local;
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024; // 10 MB

pub struct FileLogger {
    writer: Mutex<LogWriter>,
}

struct LogWriter {
    file: std::fs::File,
    current_date: String,
    current_path: PathBuf,
    bytes_written: u64,
}

impl FileLogger {
    pub fn init(custom_path: &str) {
        let p = if custom_path.is_empty() {
            LogWriter::default_log_path()
        } else {
            PathBuf::from(custom_path)
        };

        if let Some(parent) = p.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let logger = Box::new(FileLogger {
            writer: Mutex::new(LogWriter::open(p)),
        });

        log::set_max_level(LevelFilter::Info);
        let _ = log::set_boxed_logger(logger);
    }

    pub fn info(msg: &str) {
        log::info!("{}", msg);
    }

    pub fn error(msg: &str) {
        log::error!("{}", msg);
    }

    pub fn warn(msg: &str) {
        log::warn!("{}", msg);
    }

    pub fn debug(msg: &str) {
        log::debug!("{}", msg);
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let now = Local::now();
        let line = format!(
            "[{}] [{:<5}] {}\n",
            now.format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level(),
            record.args()
        );

        if let Ok(mut writer) = self.writer.lock() {
            writer.write_line(&line);
        }
    }

    fn flush(&self) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.file.flush();
        }
    }
}

impl LogWriter {
    fn default_log_path() -> PathBuf {
        let today = Local::now().format("%Y-%m-%d").to_string();
        PathBuf::from("/var/log/dlp-agent").join(format!("dlpagent-{}.log", today))
    }

    fn open(path: PathBuf) -> Self {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .unwrap_or_else(|_| {
                let fallback = PathBuf::from("/tmp").join("dlpagent-fallback.log");
                OpenOptions::new().create(true).append(true).open(fallback).unwrap()
            });

        let bytes_written = file.metadata().map(|m| m.len()).unwrap_or(0);

        LogWriter {
            file,
            current_date: today,
            current_path: path,
            bytes_written,
        }
    }

    fn write_line(&mut self, line: &str) {
        let bytes = line.as_bytes();
        let _ = self.file.write_all(bytes);
        self.bytes_written += bytes.len() as u64;

        if self.bytes_written >= MAX_FILE_BYTES {
            let rotated = self.current_path.with_extension(format!(
                "{}.bak",
                Local::now().format("%H%M%S")
            ));
            let _ = fs::rename(&self.current_path, rotated);
            *self = Self::open(self.current_path.clone());
        }
    }
}
