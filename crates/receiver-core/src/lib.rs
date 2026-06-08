//! Technical core for Receiver.
//!
//! This crate intentionally contains no GTK or Vala integration. It preserves
//! Receiver's existing data formats so it can be used beside the current app or
//! by a future Rust UI.

pub mod history;
pub mod images;
pub mod lastfm;
pub mod metadata;
pub mod models;
pub mod player;
pub mod scrobbler;
pub mod state;
pub mod stations;

pub use history::{HistoryEntry, HistoryStore};
pub use images::ImageCache;
pub use lastfm::{LastfmClient, LastfmResponse};
pub use metadata::{MetadataExtractor, MetadataParser};
pub use models::{PlayerState, Station, StreamInfo, TrackInfo};
pub use player::{Player, PlayerEvent, StreamDetails};
pub use scrobbler::Scrobbler;
pub use state::{AppSettings, AppState};
pub use stations::{StationFilter, StationRepository};

#[derive(Debug, thiserror::Error)]
pub enum ReceiverError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid data: {0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, ReceiverError>;
