use encoding_rs::{EUC_JP, EUC_KR, GB18030, ISO_8859_2, SHIFT_JIS, WINDOWS_1250, WINDOWS_1252};
use regex::Regex;

use crate::models::TrackInfo;

#[derive(Debug, Clone)]
pub struct MetadataParser {
    decimal: Regex,
    hex: Regex,
    xml_artist: Regex,
    xml_title: Regex,
}

impl Default for MetadataParser {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataParser {
    pub fn new() -> Self {
        Self {
            decimal: Regex::new(r"&#(\d+);").unwrap(),
            hex: Regex::new(r"&#[xX]([0-9a-fA-F]+);").unwrap(),
            xml_artist: Regex::new(
                r"<DB_(?:DALET_|LEAD_)?ARTIST_NAME>(.*?)</DB_(?:DALET_|LEAD_)?ARTIST_NAME>",
            )
            .unwrap(),
            xml_title: Regex::new(
                r"<DB_(?:DALET_|LEAD_)?TITLE_NAME>(.*?)</DB_(?:DALET_|LEAD_)?TITLE_NAME>",
            )
            .unwrap(),
        }
    }

    pub fn clean_metadata(&self, raw_title: &str, raw_artist: Option<&str>) -> String {
        let mut title = self.fix_encoding(strip_bom(raw_title));
        title = self.parse_xml(&title);

        if let Some(raw_artist) = raw_artist.filter(|s| !s.is_empty()) {
            let artist = self.fix_encoding(strip_bom(raw_artist));
            if !title.to_lowercase().contains(&artist.to_lowercase()) && !title.contains(" - ") {
                title = format!("{artist} - {title}");
            }
        }

        self.strip_script_artifacts(&self.decode_entities(&title))
    }

    fn fix_encoding(&self, text: &str) -> String {
        if !has_latin1_supplement(text) {
            return text.to_owned();
        }

        let raw: Vec<u8> = text.chars().map(|c| c as u32 as u8).collect();
        for encoding in [EUC_KR, EUC_JP, SHIFT_JIS, GB18030] {
            let (decoded, _, had_errors) = encoding.decode(&raw);
            if !had_errors && has_cjk(&decoded) {
                return decoded.into_owned();
            }
        }
        for encoding in [WINDOWS_1250, ISO_8859_2] {
            let (decoded, _, had_errors) = encoding.decode(&raw);
            if !had_errors && has_extended_latin(&decoded) {
                return decoded.into_owned();
            }
        }
        let (decoded, _, had_errors) = WINDOWS_1252.decode(&raw);
        if !had_errors && decoded != text {
            decoded.into_owned()
        } else {
            text.to_owned()
        }
    }

    fn parse_xml(&self, text: &str) -> String {
        if !text.starts_with("<?xml") {
            return text.to_owned();
        }
        let artist = self
            .xml_artist
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|m| m.as_str());
        let title = self
            .xml_title
            .captures(text)
            .and_then(|m| m.get(1))
            .map(|m| m.as_str());
        match (artist, title) {
            (Some(artist), Some(title)) => format!("{artist} - {title}"),
            (Some(artist), None) => artist.to_owned(),
            (None, Some(title)) => title.to_owned(),
            _ => text.to_owned(),
        }
    }

    fn decode_entities(&self, text: &str) -> String {
        if !text.contains('&') {
            return text.to_owned();
        }

        let mut result = html_escape::decode_html_entities(text).into_owned();
        result = self
            .decimal
            .replace_all(&result, |caps: &regex::Captures<'_>| {
                caps[1]
                    .parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| caps[0].to_owned())
            })
            .into_owned();
        self.hex
            .replace_all(&result, |caps: &regex::Captures<'_>| {
                u32::from_str_radix(&caps[1], 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| caps[0].to_owned())
            })
            .into_owned()
    }

    fn strip_script_artifacts(&self, text: &str) -> String {
        let latin = text.chars().filter(|&c| is_latin_letter(c)).count();
        let foreign = text
            .chars()
            .filter(|&c| c > '\u{024f}' && c.is_alphabetic())
            .count();
        if latin < 3 || foreign == 0 || foreign * 4 > latin {
            return text.to_owned();
        }
        text.chars()
            .filter(|&c| !(c > '\u{024f}' && c.is_alphabetic()))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct MetadataExtractor {
    non_song_patterns: Vec<Regex>,
    strip_prefixes: Vec<Regex>,
    station_artist_patterns: Vec<Regex>,
    non_song_title_patterns: Vec<Regex>,
    non_song_title_only_patterns: Vec<Regex>,
    semicolon: Regex,
    slash: Regex,
    tight_dash: Regex,
    von: Regex,
    by: Regex,
    tilde: Regex,
    featuring: Regex,
    title_suffix: Regex,
    dj_suffix: Regex,
    freq_prefix: Regex,
    multi_spaces: Regex,
    trailing_patterns: Vec<Regex>,
}

impl Default for MetadataExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataExtractor {
    pub fn new() -> Self {
        Self {
            non_song_patterns: regexes(&[
                r"(?i)^\d+\.\d+\s*(?:FM|MHz)\b",
                r"(?i)^(?:live\s*!?|live broadcast|live on air)$",
                r"(?i)^now\s+playing\b",
                r"(?i)^(?:song|unknown|\.\.|-)$",
                r"(?i)^https?://",
                r"(?i)^mms?://",
                r"(?i)\bvisit\b.*\.(com|net|org)",
                r"(?i)^(?:jingle|commercial|ads)\b",
                r"(?i)^(?:powered\s+by|streaming|download)\b",
                r"(?i)^(?:radio\s+\w+(?:\s+\w+)?(?:\s+\d+(?:\.\d+)?(?:\s*(?:FM|AM|MHz))?)?)$",
                r"(?i)^\w+\s*(?:FM|AM)\s*(?:\d+(?:\.\d+)?)?$",
                r"(?i)^\w+\s+radio$",
                r"(?i)^www\.\w+\.\w+",
                r"(?i)^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$",
                r"^\d+$",
                r"^\w{1,3}$",
                r"(?i)^AutoDJ\b",
            ]),
            strip_prefixes: regexes(&[
                r"(?i)^now\s+on\s+air:\d*\s*",
                r"(?i)^now\s+playing\\?:\s*",
                r"^NOW:\s*",
                r"(?i)^track:\s*",
                r"(?i)^AutoDJ:\s*",
                r"(?i)^\d{1,2}:\d{2}\s*(?:am|pm)?\s*-\s*",
            ]),
            station_artist_patterns: regexes(&[
                r"(?i)\bradio\b",
                r"\bFM\b",
                r"(?i)\.(?:com|net|org|fm|am|today)\b",
                r"(?i)\bstation\b",
                r"(?i)^(?:Airtime|LibreTime)$",
            ]),
            non_song_title_patterns: regexes(&[
                r"(?i)^(?:offline|on\s*air|en\s+vivo|nonstop|non.stop|tune!?|live)$",
                r"(?i)^\d+(?:\.\d+)?\s*(?:FM|MHz|AM|khz)\b",
                r"(?i)^(?:we play hits|sounds like you|upgrade to premium)$",
                r"^\d+(?:\.\d+)?$",
            ]),
            non_song_title_only_patterns: regexes(&[
                r"(?i)\bradio\b",
                r"(?i)\b\d+\.\d+\s*(?:FM|MHz|AM)\b",
                r"\bFM\b",
                r"(?i)\.com\b",
                r"^[A-Z\s\d]{4,}$",
                r"\d{4}-\d{2}-\d{2}",
                r"(?i)^\w+FM$",
                r"(?i)^(?:Commercial|Reklama|PUBBLICITA)\b",
            ]),
            semicolon: Regex::new(r"^(.+?)\s*;\s*(.+)$").unwrap(),
            slash: Regex::new(r"^(.+?)\s*/\s*(.+)$").unwrap(),
            tight_dash: Regex::new(r"^([A-Za-z\u{00C0}-\u{024F}].{1,}?)-([A-Za-z\u{00C0}-\u{024F}].{1,})$").unwrap(),
            von: Regex::new(r"(?i)^(.+?)\s+von\s+(.+)$").unwrap(),
            by: Regex::new(r"(?i)^(.+?)\s+by\s+(.+)$").unwrap(),
            tilde: Regex::new(r"^(.+?)~(.+?)~~").unwrap(),
            featuring: Regex::new(r"(?i)\s*(?:\[\+\]\s*.*|\s+feat\.?\s+.*|\s+ft\.?\s+.*|\s*\(feat\.?[^)]*\)|\s*\(ft\.?[^)]*\))\s*$").unwrap(),
            title_suffix: Regex::new(r"(?i)\s*\((?:Live|Remaster(?:ed)?|Bonus Track|Radio Edit|Single Version|Album Version|Acoustic)[^)]*\)\s*$").unwrap(),
            dj_suffix: Regex::new(r"(?i)(?:\s+-\s+\d{1,2}[AB])?\s+-\s+\d{2,3}\s*$").unwrap(),
            freq_prefix: Regex::new(r"(?i)^\d+\.\d+\s*(?:FM|MHz)\s*\|\s*").unwrap(),
            multi_spaces: Regex::new(r"\s{2,}").unwrap(),
            trailing_patterns: regexes(&[
                r"(?i)\s*\*{2,}\s*(?:NEXT:|www\.).*$",
                r"(?i)\s*\|\s*\d+\.\d+\s*(?:MHz|FM)\b.*$",
                r"(?i)\s*-?\s*www\.\S+\s*(?:\(\d+:\d+\))?\s*$",
                r"(?i)\s*\(?\s*www\.\S+\)?\s*$",
                r"(?i)\s*\|\s*https?://\S+.*$",
                r"(?i)\s*\([^)]*\.[a-z]{2,}\)\s*$",
                r"(?i)\s*[-*]*\s*NEXT:.*$",
                r"[\s*~]+$",
                r"(?:\s+-)+\s*$",
                r"^\d{1,3}\s*[-\.]\s*",
            ]),
        }
    }

    pub fn clean(&self, raw: &str) -> String {
        let mut text = raw.trim().replace('_', " ").replace(" amp ", " & ");
        text = Regex::new(r"(\w)\(")
            .unwrap()
            .replace_all(&text, "$1 (")
            .into_owned();
        text = text
            .replace('\u{02D7}', "-")
            .replace(['\u{2013}', '\u{2014}', '\u{2015}', '\u{2212}'], "-");

        if let Some(caps) = self.tilde.captures(&text) {
            let rest = &text[caps.get(0).unwrap().end()..];
            if rest.contains('~') {
                let title = caps[1].trim();
                let artist = caps[2].trim();
                return match (!artist.is_empty(), !title.is_empty()) {
                    (true, true) => format!("{artist} - {title}"),
                    (false, true) => title.to_owned(),
                    (true, false) => artist.to_owned(),
                    _ => String::new(),
                };
            }
        }

        if text.contains('\t') && !text.contains(" - ") {
            text = text.replace('\t', " - ");
        }
        for re in &self.strip_prefixes {
            text = re.replace_all(&text, "").into_owned();
        }
        text = self.freq_prefix.replace_all(&text, "").into_owned();
        for re in &self.trailing_patterns {
            text = re.replace_all(&text, "").into_owned();
        }
        text = self.dj_suffix.replace_all(&text, "").into_owned();
        if let Some(stripped) = text.strip_prefix(". - ") {
            text = stripped.to_owned();
        } else if let Some(stripped) = text.strip_prefix("- ") {
            text = stripped.to_owned();
        }
        self.multi_spaces.replace_all(&text, " ").trim().to_owned()
    }

    pub fn extract_artist_title(&self, cleaned: &str) -> TrackInfo {
        let cleaned = cleaned.trim();
        if cleaned.is_empty() || cleaned == "-" || cleaned == ".." {
            return empty_track();
        }
        if self.non_song_patterns.iter().any(|re| re.is_match(cleaned)) {
            return empty_track();
        }

        if let Some((artist, title)) = cleaned.split_once(" - ") {
            return self.track_from_artist_title(artist, title);
        }
        if let Some((artist, title)) = cleaned.split_once(" -- ") {
            return self.track_from_artist_title(artist, title);
        }
        if let Some(caps) = self.semicolon.captures(cleaned) {
            return self.track_from_artist_title(&caps[2], &caps[1]);
        }
        if let Some(caps) = self.slash.captures(cleaned) {
            return self.track_from_artist_title(&caps[1], &caps[2]);
        }
        if let Some(caps) = self.von.captures(cleaned) {
            return self.track_from_artist_title(&caps[2], &caps[1]);
        }
        if let Some(caps) = self.by.captures(cleaned) {
            if caps[2].split_whitespace().count() >= 2 {
                return self.track_from_artist_title(&caps[2], &caps[1]);
            }
        }
        if let Some(caps) = self.tight_dash.captures(cleaned) {
            if caps[1].trim().len() >= 2 && caps[2].trim().len() >= 2 {
                return self.track_from_artist_title(&caps[1], &caps[2]);
            }
        }
        if self.looks_like_non_song_title(cleaned) {
            empty_track()
        } else {
            TrackInfo {
                artist: None,
                title: Some(cleaned.to_owned()),
            }
        }
    }

    fn track_from_artist_title(&self, artist: &str, title: &str) -> TrackInfo {
        let artist = artist.trim();
        let mut title = title.trim().to_owned();
        if artist.is_empty() || title.is_empty() {
            return empty_track();
        }
        let prefix = format!("{artist} - ");
        if title.to_lowercase().starts_with(&prefix.to_lowercase()) {
            title = title[prefix.len()..].trim().to_owned();
        }
        if self.looks_like_station_name(artist) && title.contains(" - ") {
            return self.extract_artist_title(&title);
        }
        if self.looks_like_station(artist, &title) {
            return empty_track();
        }
        TrackInfo {
            artist: Some(self.clean_artist(artist)),
            title: Some(self.clean_title(&title)),
        }
    }

    fn looks_like_station_name(&self, artist: &str) -> bool {
        self.station_artist_patterns
            .iter()
            .any(|re| re.is_match(artist))
    }

    fn looks_like_station(&self, artist: &str, title: &str) -> bool {
        self.looks_like_station_name(artist)
            && (self
                .non_song_title_patterns
                .iter()
                .any(|re| re.is_match(title))
                || Regex::new(r"(?i)(?:www\.|https?://)")
                    .unwrap()
                    .is_match(title)
                || artist.trim().eq_ignore_ascii_case(title.trim()))
    }

    fn looks_like_non_song_title(&self, text: &str) -> bool {
        self.non_song_title_only_patterns
            .iter()
            .any(|re| re.is_match(text))
    }

    fn clean_artist(&self, artist: &str) -> String {
        let cleaned = self.featuring.replace_all(artist, "").trim().to_owned();
        if cleaned.is_empty() {
            artist.to_owned()
        } else {
            cleaned
        }
    }

    fn clean_title(&self, title: &str) -> String {
        let cleaned = self.title_suffix.replace_all(title, "").trim().to_owned();
        let cleaned = self.dj_suffix.replace_all(&cleaned, "").trim().to_owned();
        if cleaned.is_empty() {
            title.to_owned()
        } else {
            cleaned
        }
    }
}

fn regexes(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .map(|pattern| Regex::new(pattern).unwrap())
        .collect()
}

fn empty_track() -> TrackInfo {
    TrackInfo {
        artist: None,
        title: None,
    }
}

fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn has_latin1_supplement(text: &str) -> bool {
    text.chars().any(|c| ('\u{0080}'..='\u{00ff}').contains(&c))
}

fn has_extended_latin(text: &str) -> bool {
    text.chars().any(|c| ('\u{0100}'..='\u{024f}').contains(&c))
}

fn has_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{ac00}'..='\u{d7a3}').contains(&c)
            || ('\u{1100}'..='\u{11ff}').contains(&c)
            || ('\u{3400}'..='\u{9fff}').contains(&c)
            || ('\u{3040}'..='\u{30ff}').contains(&c)
    })
}

fn is_latin_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
        || (('\u{00c0}'..='\u{00ff}').contains(&c) && c != '\u{00d7}' && c != '\u{00f7}')
        || ('\u{0100}'..='\u{024f}').contains(&c)
        || ('\u{1e00}'..='\u{1eff}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_artist_tag_with_title() {
        let parser = MetadataParser::new();
        assert_eq!(
            parser.clean_metadata("Song &amp; More", Some("Artist")),
            "Artist - Song & More"
        );
    }

    #[test]
    fn extracts_artist_and_title() {
        let extractor = MetadataExtractor::new();
        let info = extractor.extract_artist_title("Daft Punk - One More Time");
        assert_eq!(info.artist.as_deref(), Some("Daft Punk"));
        assert_eq!(info.title.as_deref(), Some("One More Time"));
    }

    #[test]
    fn rejects_station_promos() {
        let extractor = MetadataExtractor::new();
        assert!(!extractor.extract_artist_title("Radio Test").is_song());
    }
}
