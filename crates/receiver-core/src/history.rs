use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::metadata::MetadataExtractor;
use crate::models::Station;
use crate::Result;

pub const MAX_HISTORY_ENTRIES: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub station_id: i64,
    pub station_name: String,
    pub song_title: String,
    pub played_at: String,
}

#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
    entries: Vec<HistoryEntry>,
    extractor: MetadataExtractor,
}

impl HistoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let entries = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            Vec::new()
        };
        Ok(Self {
            path,
            entries,
            extractor: MetadataExtractor::new(),
        })
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn add(&mut self, station: &Station, song_title: &str) -> Result<bool> {
        if !self.extractor.extract_artist_title(song_title).is_song() {
            return Ok(false);
        }
        if self
            .entries
            .first()
            .map(|entry| entry.station_id == station.id && entry.song_title == song_title)
            .unwrap_or(false)
        {
            return Ok(false);
        }

        self.entries.insert(
            0,
            HistoryEntry {
                station_id: station.id,
                station_name: station.name.clone(),
                song_title: song_title.to_owned(),
                played_at: Local::now().to_rfc3339(),
            },
        );
        self.entries.truncate(MAX_HISTORY_ENTRIES);
        self.save()?;
        Ok(true)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.save()
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_vec_pretty(&self.entries)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station() -> Station {
        Station {
            id: 1,
            source: 1,
            name: "Radio".into(),
            homepage: None,
            country: None,
            streams_raw: None,
            tags_raw: None,
            image_hash: 0,
        }
    }

    #[test]
    fn adds_and_deduplicates_history() {
        let tmp = tempfile::tempdir().unwrap();
        let mut store = HistoryStore::open(tmp.path().join("history.json")).unwrap();
        assert!(store.add(&station(), "Artist - Song").unwrap());
        assert!(!store.add(&station(), "Artist - Song").unwrap());
        assert_eq!(store.entries().len(), 1);
    }
}
