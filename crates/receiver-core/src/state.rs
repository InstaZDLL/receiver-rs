use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::Station;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub volume: f64,
    pub last_station_id: i64,
    pub search_query: String,
    pub language_filter: String,
    pub country_filter: String,
    pub download_dir: String,
    pub lastfm_session_key: String,
    pub lastfm_username: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            last_station_id: 0,
            search_query: String::new(),
            language_filter: "all".to_owned(),
            country_filter: "all".to_owned(),
            download_dir: String::new(),
            lastfm_session_key: String::new(),
            lastfm_username: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    config_dir: PathBuf,
    pub settings: AppSettings,
    favourites: Vec<Station>,
}

impl AppState {
    pub fn open(config_dir: impl AsRef<Path>) -> Result<Self> {
        let config_dir = config_dir.as_ref().to_path_buf();
        fs::create_dir_all(&config_dir)?;
        let settings = read_json(config_dir.join("settings.json"))?.unwrap_or_default();
        let favourites = read_json(config_dir.join("favourites.json"))?.unwrap_or_default();
        Ok(Self {
            config_dir,
            settings,
            favourites,
        })
    }

    pub fn receiver_config_dir() -> Result<PathBuf> {
        let dirs =
            directories::ProjectDirs::from("io.github", "meehow", "receiver").ok_or_else(|| {
                crate::ReceiverError::InvalidData("cannot find config directory".into())
            })?;
        Ok(dirs.config_dir().to_path_buf())
    }

    pub fn open_default() -> Result<Self> {
        Self::open(Self::receiver_config_dir()?)
    }

    pub fn favourites(&self) -> &[Station] {
        &self.favourites
    }

    pub fn favourite_ids(&self) -> Vec<i64> {
        self.favourites.iter().map(|station| station.id).collect()
    }

    pub fn is_favourite(&self, id: i64) -> bool {
        self.favourites.iter().any(|station| station.id == id)
    }

    pub fn toggle_favourite(&mut self, station: Station) -> Result<()> {
        if let Some(pos) = self.favourites.iter().position(|s| s.id == station.id) {
            self.favourites.remove(pos);
        } else {
            self.favourites.push(station);
        }
        self.save_favourites()
    }

    pub fn move_favourite(&mut self, from: usize, to: usize) -> Result<()> {
        if from == to || from >= self.favourites.len() || to >= self.favourites.len() {
            return Ok(());
        }
        let station = self.favourites.remove(from);
        self.favourites.insert(to, station);
        self.save_favourites()
    }

    pub fn save_settings(&self) -> Result<()> {
        write_json(self.config_dir.join("settings.json"), &self.settings)
    }

    pub fn save_favourites(&self) -> Result<()> {
        write_json(self.config_dir.join("favourites.json"), &self.favourites)
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read(path)?;
    Ok(Some(serde_json::from_slice(&data)?))
}

fn write_json<T: Serialize>(path: PathBuf, value: &T) -> Result<()> {
    let data = serde_json::to_vec_pretty(value)?;
    fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(id: i64) -> Station {
        Station {
            id,
            source: 1,
            name: format!("Station {id}"),
            homepage: None,
            country: None,
            streams_raw: None,
            tags_raw: None,
            image_hash: 0,
        }
    }

    #[test]
    fn toggles_and_moves_favourites() {
        let tmp = tempfile::tempdir().unwrap();
        let mut state = AppState::open(tmp.path()).unwrap();
        state.toggle_favourite(station(1)).unwrap();
        state.toggle_favourite(station(2)).unwrap();
        state.move_favourite(1, 0).unwrap();
        assert_eq!(state.favourite_ids(), vec![2, 1]);
        state.toggle_favourite(station(2)).unwrap();
        assert_eq!(state.favourite_ids(), vec![1]);
    }
}
