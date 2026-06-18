//! Async logging queue to remove logging contention from evaluation hot path
//! 
//! This module provides a non-blocking async logging mechanism that moves
//! all logging operations out of the evaluation hot path, eliminating
//! hidden global contention from logging mutexes.

use tokio::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Log message structure
#[derive(Debug, Clone)]
pub struct LogMessage {
    pub timestamp: u64,
    pub level: String,
    pub message: String,
}

/// Async logger that processes log messages off the hot path
pub struct AsyncLogger {
    log_rx: mpsc::Receiver<LogMessage>,
}

impl AsyncLogger {
    /// Create a new async logger
    pub fn new(log_rx: mpsc::Receiver<LogMessage>) -> Self {
        Self { log_rx }
    }

    /// Run the async logger worker
    pub async fn run(mut self) {
        while let Some(log_msg) = self.log_rx.recv().await {
            // Write log message to stderr
            eprintln!("[{}] {}", log_msg.level, log_msg.message);
        }
    }
}

/// Async log sender for the hot path
#[derive(Clone)]
pub struct AsyncLogSender {
    log_tx: mpsc::Sender<LogMessage>,
}

impl AsyncLogSender {
    /// Create a new async log sender
    pub fn new(log_tx: mpsc::Sender<LogMessage>) -> Self {
        Self { log_tx }
    }

    /// Try to send a log message without blocking
    /// Returns error if the channel is full (log is dropped)
    pub fn try_log(&self, level: &str, message: String) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let log_msg = LogMessage {
            timestamp,
            level: level.to_string(),
            message,
        };
        
        // Non-blocking send - drop log if channel is full
        let _ = self.log_tx.try_send(log_msg);
    }

    /// Try to log a debug message
    pub fn try_debug(&self, message: String) {
        self.try_log("DEBUG", message);
    }

    /// Try to log an info message
    pub fn try_info(&self, message: String) {
        self.try_log("INFO", message);
    }

    /// Try to log a warning message
    pub fn try_warn(&self, message: String) {
        self.try_log("WARN", message);
    }

    /// Try to log an error message
    pub fn try_error(&self, message: String) {
        self.try_log("ERROR", message);
    }
}

/// Create an async logging channel
/// Returns (AsyncLogSender, mpsc::Receiver<LogMessage>)
pub fn create_async_log_channel(capacity: usize) -> (AsyncLogSender, mpsc::Receiver<LogMessage>) {
    let (tx, rx) = mpsc::channel(capacity);
    let sender = AsyncLogSender::new(tx);
    (sender, rx)
}

/// Global async log sender (optional, for convenience)
static mut GLOBAL_ASYNC_LOGGER: Option<AsyncLogSender> = None;

/// Initialize global async logger
pub fn init_global_async_logger(sender: AsyncLogSender) {
    unsafe {
        GLOBAL_ASYNC_LOGGER = Some(sender);
    }
}

/// Get global async logger
pub fn get_global_async_logger() -> Option<AsyncLogSender> {
    unsafe {
        GLOBAL_ASYNC_LOGGER.as_ref().cloned()
    }
}
