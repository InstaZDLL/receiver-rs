use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::models::{PlayerState, Station};
use crate::{ReceiverError, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamDetails {
    pub codec: String,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub channels: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    StateChanged(PlayerState),
    Error(String),
    MetadataChanged(String),
    StreamResolved(String),
    StreamInfoChanged(StreamDetails),
}

#[derive(Debug)]
pub struct Player {
    current_station: Option<Station>,
    current_stream_url: Option<String>,
    state: PlayerState,
    volume: f64,
    events: broadcast::Sender<PlayerEvent>,
    client: reqwest::Client,
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(64);
        Self {
            current_station: None,
            current_stream_url: None,
            state: PlayerState::Stopped,
            volume: 1.0,
            events,
            client: reqwest::Client::new(),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<PlayerEvent> {
        self.events.subscribe()
    }

    pub fn current_station(&self) -> Option<&Station> {
        self.current_station.as_ref()
    }

    pub fn current_stream_url(&self) -> Option<&str> {
        self.current_stream_url.as_deref()
    }

    pub fn state(&self) -> PlayerState {
        self.state
    }

    pub fn volume(&self) -> f64 {
        self.volume
    }

    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub async fn play(&mut self, station: Station) -> Result<()> {
        let Some(url) = station.stream_url().map(ToOwned::to_owned) else {
            self.emit(PlayerEvent::Error("No stream URL available".to_owned()));
            return Err(ReceiverError::InvalidData("No stream URL available".into()));
        };
        self.current_station = Some(station);
        self.emit(PlayerEvent::MetadataChanged(
            self.current_station.as_ref().unwrap().name.clone(),
        ));
        let resolved = self.resolve_stream_url(&url).await?;
        self.current_stream_url = Some(resolved.clone());
        self.emit(PlayerEvent::StreamResolved(resolved));
        self.update_state(PlayerState::Playing);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.current_station = None;
        self.current_stream_url = None;
        self.update_state(PlayerState::Stopped);
        Ok(())
    }

    pub async fn toggle_pause(&mut self) -> Result<()> {
        match self.state {
            PlayerState::Playing => {
                self.update_state(PlayerState::Paused);
                Ok(())
            }
            PlayerState::Paused => {
                self.update_state(PlayerState::Playing);
                Ok(())
            }
            _ => {
                if let Some(station) = self.current_station.clone() {
                    self.play(station).await
                } else {
                    Ok(())
                }
            }
        }
    }

    pub async fn resolve_stream_url(&self, url: &str) -> Result<String> {
        if url.ends_with(".pls") || url.ends_with(".m3u") {
            self.resolve_playlist(url).await
        } else {
            self.resolve_redirects(url).await
        }
    }

    async fn resolve_redirects(&self, url: &str) -> Result<String> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;
        Ok(response.url().to_string())
    }

    async fn resolve_playlist(&self, url: &str) -> Result<String> {
        let content = self
            .client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        for line in content.lines().map(str::trim) {
            if url.ends_with(".pls") && line.starts_with("File") {
                if let Some((_, value)) = line.split_once('=') {
                    return Ok(value.to_owned());
                }
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with("http://") || line.starts_with("https://") {
                return Ok(line.to_owned());
            }
        }

        Err(ReceiverError::InvalidData(
            "No stream URL in playlist".into(),
        ))
    }

    fn update_state(&mut self, state: PlayerState) {
        if self.state != state {
            self.state = state;
            self.emit(PlayerEvent::StateChanged(state));
        }
    }

    fn emit(&self, event: PlayerEvent) {
        let _ = self.events.send(event);
    }
}
