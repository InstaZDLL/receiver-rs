use serde::{Deserialize, Serialize};

pub const SOURCE_RADIO_BROWSER: i32 = 1;
pub const SOURCE_SOMAFM: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    pub id: i64,
    pub source: i32,
    pub name: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub streams_raw: Option<String>,
    #[serde(default)]
    pub tags_raw: Option<String>,
    #[serde(default)]
    pub image_hash: i64,
}

impl Station {
    pub fn stream_url(&self) -> Option<&str> {
        best_stream_url(self.streams_raw.as_deref())
    }

    pub fn streams(&self) -> Vec<StreamInfo> {
        parse_streams(self.streams_raw.as_deref())
    }

    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();
        if let Some(country) = self.country.as_ref().filter(|s| !s.is_empty()) {
            parts.push(country.clone());
        }
        if let Some(tags) = self.tags_raw.as_ref() {
            parts.extend(tags.split_whitespace().map(ToOwned::to_owned));
        }
        parts.join(" \u{2022} ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamInfo {
    pub url: String,
    pub quality: String,
    pub bitrate: Option<u32>,
    pub codec: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub artist: Option<String>,
    pub title: Option<String>,
}

impl TrackInfo {
    pub fn is_song(&self) -> bool {
        self.title.is_some()
    }
}

pub fn best_stream_url(raw: Option<&str>) -> Option<&str> {
    let mut best_url = None;

    for part in raw?.split(';') {
        let mut fields = part.split('|');
        let url = fields.next().unwrap_or("").trim();
        if url.is_empty() {
            continue;
        }
        let quality = fields.next().unwrap_or("medium").trim();
        if quality == "highest" {
            return Some(url);
        }
        if best_url.is_none() || quality == "high" {
            best_url = Some(url);
        }
    }

    best_url
}

pub fn parse_streams(raw: Option<&str>) -> Vec<StreamInfo> {
    raw.unwrap_or("")
        .split(';')
        .filter_map(|part| {
            let fields: Vec<_> = part.split('|').collect();
            let url = fields.first()?.trim();
            if url.is_empty() {
                return None;
            }
            Some(StreamInfo {
                url: url.to_owned(),
                quality: fields.get(1).unwrap_or(&"medium").trim().to_owned(),
                bitrate: fields.get(2).and_then(|s| s.trim().parse().ok()),
                codec: fields
                    .get(3)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_highest_stream_first() {
        let raw = "a|medium|128|mp3;b|high|192|mp3;c|highest|320|mp3";
        assert_eq!(best_stream_url(Some(raw)), Some("c"));
    }

    #[test]
    fn falls_back_to_high_stream() {
        let raw = "a|medium;b|high;c|low";
        assert_eq!(best_stream_url(Some(raw)), Some("b"));
    }
}
