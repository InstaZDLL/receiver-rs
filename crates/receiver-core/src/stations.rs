use std::path::Path;

use rand::seq::SliceRandom;
use rusqlite::{params, Connection, Row};

use crate::models::{Station, SOURCE_SOMAFM};
use crate::Result;

const COLS: &str = "id, source, name, homepage, country, streams_raw, tags_raw, image_hash";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StationFilter {
    pub search_query: String,
    pub language: String,
    pub country: String,
    pub favourite_ids: Vec<i64>,
}

impl StationFilter {
    pub fn all() -> Self {
        Self {
            language: "all".to_owned(),
            country: "all".to_owned(),
            ..Self::default()
        }
    }
}

pub struct StationRepository {
    conn: Connection,
}

impl StationRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            conn: Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?,
        })
    }

    pub fn total_count(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM stations", [], |row| {
                row.get::<_, i64>(0)
            })? as usize)
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Station>> {
        let mut stmt = self
            .conn
            .prepare(&format!("SELECT {COLS} FROM stations WHERE id = ?"))?;
        let mut rows = stmt.query(params![id])?;
        Ok(rows.next()?.map(parse_station).transpose()?)
    }

    pub fn available_languages(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT code FROM languages")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn available_countries(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT country_code, country FROM stations \
             WHERE country_code IS NOT NULL AND country_code != '' \
             AND country IS NOT NULL AND country != '' ORDER BY country",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn locale_country_name(&self, country_code: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT country FROM stations WHERE country_code = ? LIMIT 1")?;
        let mut rows = stmt.query(params![country_code])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn query(&self, filter: &StationFilter) -> Result<Vec<Station>> {
        let use_fts = !filter.search_query.trim().is_empty();
        let prefix = if use_fts { "s." } else { "" };
        let mut sql = String::new();

        if use_fts {
            sql.push_str("SELECT s.");
            sql.push_str(&COLS.replace(", ", ", s."));
            sql.push_str(" FROM stations s JOIN stations_fts f ON s.rowid = f.rowid WHERE stations_fts MATCH ?");
        } else {
            sql.push_str("SELECT ");
            sql.push_str(COLS);
            sql.push_str(" FROM stations WHERE 1=1");
        }

        if filter.language != "all" && !filter.language.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(prefix);
            sql.push_str("languages_raw LIKE ?");
        }
        if filter.country != "all" && !filter.country.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(prefix);
            sql.push_str("country_code = ?");
        }

        sql.push_str(" ORDER BY CASE WHEN source = ");
        sql.push_str(&SOURCE_SOMAFM.to_string());
        sql.push_str(" THEN 2 WHEN image_hash != 0 THEN 3 ELSE 4 END");

        let mut stmt = self.conn.prepare(&sql)?;
        let fts = fts_query(&filter.search_query);
        let language = format!("%{}%", filter.language);
        let mut values: Vec<&dyn rusqlite::ToSql> = Vec::new();
        if use_fts {
            values.push(&fts);
        }
        if filter.language != "all" && !filter.language.is_empty() {
            values.push(&language);
        }
        if filter.country != "all" && !filter.country.is_empty() {
            values.push(&filter.country);
        }

        let rows = stmt.query_map(rusqlite::params_from_iter(values), parse_station)?;
        let mut stations = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        apply_favourite_sort(&mut stations, &filter.favourite_ids);
        Ok(stations)
    }
}

fn parse_station(row: &Row<'_>) -> rusqlite::Result<Station> {
    Ok(Station {
        id: row.get(0)?,
        source: row.get(1)?,
        name: row.get(2)?,
        homepage: row.get(3)?,
        country: row.get(4)?,
        streams_raw: row.get(5)?,
        tags_raw: row.get(6)?,
        image_hash: row.get::<_, Option<i64>>(7)?.unwrap_or_default(),
    })
}

fn fts_query(raw: &str) -> String {
    raw.trim()
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(|part| format!("{}*", part.replace('&', " ")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn apply_favourite_sort(stations: &mut [Station], favourite_ids: &[i64]) {
    stations.shuffle(&mut rand::rng());
    stations.sort_by_key(|station| {
        if favourite_ids.contains(&station.id) {
            1
        } else if station.source == SOURCE_SOMAFM {
            2
        } else if station.image_hash != 0 {
            3
        } else {
            4
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_fts_prefix_query() {
        assert_eq!(fts_query("rock jazz"), "rock* jazz*");
        assert_eq!(fts_query("r&b"), "r b*");
    }

    #[test]
    fn opens_bundled_receiver_database() {
        let db_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../data/receiver/receiver.db");
        if !db_path.exists() {
            return;
        }

        let repo = StationRepository::open(db_path).unwrap();
        assert!(repo.total_count().unwrap() > 0);
        assert!(!repo.available_languages().unwrap().is_empty());

        let stations = repo.query(&StationFilter::all()).unwrap();
        assert!(!stations.is_empty());
        let first = repo.get_by_id(stations[0].id).unwrap().unwrap();
        assert_eq!(first.id, stations[0].id);
    }
}
