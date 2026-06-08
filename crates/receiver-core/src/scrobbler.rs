use chrono::Utc;

use crate::lastfm::{LastfmClient, LastfmResponse};
use crate::metadata::MetadataExtractor;
use crate::models::PlayerState;
use crate::Result;

pub const SCROBBLE_THRESHOLD_SECONDS: i64 = 30;

#[derive(Debug, Clone)]
pub struct Scrobbler {
    client: LastfmClient,
    extractor: MetadataExtractor,
    session_key: Option<String>,
    current_artist: Option<String>,
    current_title: Option<String>,
    track_start_time: i64,
}

impl Scrobbler {
    pub fn new(session_key: Option<String>) -> Self {
        Self {
            client: LastfmClient::new(),
            extractor: MetadataExtractor::new(),
            session_key,
            current_artist: None,
            current_title: None,
            track_start_time: 0,
        }
    }

    pub fn with_client(client: LastfmClient, session_key: Option<String>) -> Self {
        Self {
            client,
            extractor: MetadataExtractor::new(),
            session_key,
            current_artist: None,
            current_title: None,
            track_start_time: 0,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.session_key
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    pub fn set_session_key(&mut self, session_key: Option<String>) {
        self.session_key = session_key;
        if !self.is_enabled() {
            self.clear_current();
        }
    }

    pub async fn start_auth(&self) -> Result<Option<(String, String)>> {
        let Some(token) = self.client.auth_get_token().await? else {
            return Ok(None);
        };
        let url = self.client.auth_url(&token);
        Ok(Some((token, url)))
    }

    pub async fn complete_auth(&mut self, token: &str) -> Result<Option<(String, Option<String>)>> {
        if let Some((key, username)) = self.client.auth_get_session(token).await? {
            self.session_key = Some(key.clone());
            Ok(Some((key, username)))
        } else {
            Ok(None)
        }
    }

    pub async fn on_metadata_changed(&mut self, title: &str) -> Result<Option<LastfmResponse>> {
        let previous = self.maybe_scrobble_current().await?;
        if !self.is_enabled() {
            return Ok(previous);
        }

        let cleaned = self.extractor.clean(title);
        let info = self.extractor.extract_artist_title(&cleaned);
        let Some(artist) = info.artist else {
            self.clear_current();
            return Ok(previous);
        };
        let Some(track) = info.title else {
            self.clear_current();
            return Ok(previous);
        };

        self.current_artist = Some(artist.clone());
        self.current_title = Some(track.clone());
        self.track_start_time = Utc::now().timestamp();

        if let Some(session_key) = &self.session_key {
            let _ = self
                .client
                .update_now_playing(&artist, &track, session_key)
                .await?;
        }

        Ok(previous)
    }

    pub async fn on_state_changed(&mut self, state: PlayerState) -> Result<Option<LastfmResponse>> {
        if matches!(state, PlayerState::Stopped | PlayerState::Error) {
            let response = self.maybe_scrobble_current().await?;
            self.clear_current();
            Ok(response)
        } else {
            Ok(None)
        }
    }

    pub async fn maybe_scrobble_current(&mut self) -> Result<Option<LastfmResponse>> {
        let (Some(artist), Some(title), Some(session_key)) = (
            self.current_artist.clone(),
            self.current_title.clone(),
            self.session_key.clone(),
        ) else {
            return Ok(None);
        };
        if self.track_start_time == 0 {
            return Ok(None);
        }
        let elapsed = Utc::now().timestamp() - self.track_start_time;
        if elapsed < SCROBBLE_THRESHOLD_SECONDS {
            return Ok(None);
        }

        let response = self
            .client
            .scrobble(&artist, &title, self.track_start_time, &session_key)
            .await?;
        if response.error_code == 9 {
            self.session_key = None;
        }
        self.clear_current();
        Ok(Some(response))
    }

    fn clear_current(&mut self) {
        self.current_artist = None;
        self.current_title = None;
        self.track_start_time = 0;
    }
}
