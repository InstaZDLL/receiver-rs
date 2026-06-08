use std::collections::BTreeMap;

use crate::Result;

const LASTFM_API_URL: &str = "http://ws.audioscrobbler.com/2.0/";
const LASTFM_API_KEY: &str = "83f81327ea8f83f4becbc8fa1255611d";
const LASTFM_API_SECRET: &str = "037b74525c3c6ce5a5535ee04fa58a27";
const LASTFM_AUTH_URL: &str = "https://www.last.fm/api/auth/";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LastfmResponse {
    pub ok: bool,
    pub error_code: i32,
    pub error_message: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LastfmClient {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
}

impl Default for LastfmClient {
    fn default() -> Self {
        Self::new()
    }
}

impl LastfmClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: LASTFM_API_KEY.to_owned(),
            api_secret: LASTFM_API_SECRET.to_owned(),
        }
    }

    pub fn with_credentials(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            api_secret: api_secret.into(),
        }
    }

    pub fn sign(&self, params: &BTreeMap<String, String>) -> String {
        let mut input = String::new();
        for (key, value) in params {
            input.push_str(key);
            input.push_str(value);
        }
        input.push_str(&self.api_secret);
        format!("{:x}", md5::compute(input))
    }

    pub async fn auth_get_token(&self) -> Result<Option<String>> {
        let mut params = BTreeMap::new();
        params.insert("method".to_owned(), "auth.getToken".to_owned());
        params.insert("api_key".to_owned(), self.api_key.clone());
        params.insert("api_sig".to_owned(), self.sign(&params));

        let body = self
            .client
            .get(LASTFM_API_URL)
            .query(&params)
            .send()
            .await?
            .text()
            .await?;
        Ok(extract_xml_value(&body, "token"))
    }

    pub fn auth_url(&self, token: &str) -> String {
        format!("{LASTFM_AUTH_URL}?api_key={}&token={token}", self.api_key)
    }

    pub async fn auth_get_session(&self, token: &str) -> Result<Option<(String, Option<String>)>> {
        let mut params = BTreeMap::new();
        params.insert("method".to_owned(), "auth.getSession".to_owned());
        params.insert("api_key".to_owned(), self.api_key.clone());
        params.insert("token".to_owned(), token.to_owned());
        params.insert("api_sig".to_owned(), self.sign(&params));

        let body = self
            .client
            .get(LASTFM_API_URL)
            .query(&params)
            .send()
            .await?
            .text()
            .await?;
        if body.contains("<error") {
            return Ok(None);
        }
        Ok(extract_xml_value(&body, "key").map(|key| (key, extract_xml_value(&body, "name"))))
    }

    pub async fn update_now_playing(
        &self,
        artist: &str,
        track: &str,
        session_key: &str,
    ) -> Result<LastfmResponse> {
        let mut params =
            self.base_track_params("track.updateNowPlaying", artist, track, session_key);
        let sig = self.sign(&params);
        params.insert("api_sig".to_owned(), sig);
        self.post(params).await
    }

    pub async fn scrobble(
        &self,
        artist: &str,
        track: &str,
        timestamp: i64,
        session_key: &str,
    ) -> Result<LastfmResponse> {
        let mut params = BTreeMap::new();
        params.insert("method".to_owned(), "track.scrobble".to_owned());
        params.insert("api_key".to_owned(), self.api_key.clone());
        params.insert("sk".to_owned(), session_key.to_owned());
        params.insert("artist[0]".to_owned(), artist.to_owned());
        params.insert("track[0]".to_owned(), track.to_owned());
        params.insert("timestamp[0]".to_owned(), timestamp.to_string());
        params.insert("chosenByUser[0]".to_owned(), "0".to_owned());
        let sig = self.sign(&params);
        params.insert("api_sig".to_owned(), sig);
        self.post(params).await
    }

    fn base_track_params(
        &self,
        method: &str,
        artist: &str,
        track: &str,
        session_key: &str,
    ) -> BTreeMap<String, String> {
        let mut params = BTreeMap::new();
        params.insert("method".to_owned(), method.to_owned());
        params.insert("api_key".to_owned(), self.api_key.clone());
        params.insert("sk".to_owned(), session_key.to_owned());
        params.insert("artist".to_owned(), artist.to_owned());
        params.insert("track".to_owned(), track.to_owned());
        params
    }

    async fn post(&self, params: BTreeMap<String, String>) -> Result<LastfmResponse> {
        let body = self
            .client
            .post(LASTFM_API_URL)
            .form(&params)
            .send()
            .await?
            .text()
            .await?;

        let mut response = LastfmResponse {
            ok: body.contains("status=\"ok\""),
            body: Some(body.clone()),
            ..LastfmResponse::default()
        };
        if !response.ok {
            response.error_code = extract_xml_attr(&body, "error", "code")
                .and_then(|code| code.parse().ok())
                .unwrap_or_default();
            response.error_message = extract_xml_value(&body, "error");
        }
        Ok(response)
    }
}

fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].to_owned())
}

fn extract_xml_attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("<{tag} {attr}=\"");
    let start = xml.find(&pattern)? + pattern.len();
    let end = xml[start..].find('"')? + start;
    Some(xml[start..end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_params_in_key_order() {
        let client = LastfmClient::with_credentials("key", "secret");
        let mut params = BTreeMap::new();
        params.insert("track".to_owned(), "Song".to_owned());
        params.insert("artist".to_owned(), "Artist".to_owned());
        assert_eq!(
            client.sign(&params),
            format!("{:x}", md5::compute("artistArtisttrackSongsecret"))
        );
    }
}
