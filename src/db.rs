//! Warstwa persystencji na SQLite (`rusqlite`).
//!
//! 🦀 Nauka: `Mutex`, `Result` i operator `?`, mapowanie wierszy, parametry nazwane,
//! dlaczego `Connection` nie jest `Sync` (i po co `Mutex`).

use crate::model::{Analysis, Job};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row, named_params};
use std::path::Path;
use std::sync::Mutex;

/// Oferta zapisana w bazie wraz z wynikami oceny.
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub job: Job,
    pub rule_score: u8,
    pub ai: Option<Analysis>,
    pub first_seen: DateTime<Utc>,
}

impl JobRecord {
    /// Wynik „najlepszy z dostępnych”: LLM jeśli jest, inaczej reguły.
    pub fn best_score(&self) -> u8 {
        self.ai.as_ref().map_or(self.rule_score, |a| a.score)
    }
}

/// Filtry listy ofert.
#[derive(Debug, Clone)]
pub struct Filter {
    pub min_score: u8,
    pub query: String,
    pub only_new: bool,
    pub limit: usize,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            min_score: 0,
            query: String::new(),
            only_new: false,
            limit: 200,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct Stats {
    pub total: u32,
    pub matched: u32,
    pub new: u32,
}

/// Okno, w którym oferta jest „nowa”.
const NEW_WINDOW_SECS: i64 = 24 * 3600;

pub struct Db {
    // `rusqlite::Connection` jest `Send`, ale nie `Sync` – Mutex pozwala
    // współdzielić go między wątkami serwera.
    conn: Mutex<Connection>,
}

const COLUMNS: &str = "id, source, external_id, title, company, location, remote, url, \
                       description, tags, salary, published_at, first_seen, rule_score, ai_json";

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("nie można otworzyć bazy {}", path.display()))?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS jobs (
                id           TEXT PRIMARY KEY,   -- fingerprint firma+tytuł (dedup)
                source       TEXT NOT NULL,
                external_id  TEXT NOT NULL,
                title        TEXT NOT NULL,
                company      TEXT NOT NULL,
                location     TEXT NOT NULL,
                remote       INTEGER NOT NULL,
                url          TEXT NOT NULL,
                description  TEXT NOT NULL,
                tags         TEXT NOT NULL,      -- JSON
                salary       TEXT,
                published_at INTEGER,
                first_seen   INTEGER NOT NULL,
                rule_score   INTEGER NOT NULL,
                ai_score     INTEGER,
                ai_json      TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_jobs_score ON jobs (COALESCE(ai_score, rule_score) DESC);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Wstawia ofertę. Zwraca `true`, jeśli była nowa (nie duplikat).
    pub fn insert_if_new(&self, job: &Job, rule_score: u8) -> Result<bool> {
        let conn = self.conn.lock().expect("mutex zatruty");
        let changed = conn.execute(
            "INSERT OR IGNORE INTO jobs
             (id, source, external_id, title, company, location, remote, url,
              description, tags, salary, published_at, first_seen, rule_score)
             VALUES (:id, :source, :ext, :title, :company, :location, :remote, :url,
                     :desc, :tags, :salary, :published, :seen, :score)",
            named_params! {
                ":id": job.id,
                ":source": job.source,
                ":ext": job.external_id,
                ":title": job.title,
                ":company": job.company,
                ":location": job.location,
                ":remote": job.remote,
                ":url": job.url,
                ":desc": job.description,
                ":tags": serde_json::to_string(&job.tags)?,
                ":salary": job.salary,
                ":published": job.published_at.map(|d| d.timestamp()),
                ":seen": Utc::now().timestamp(),
                ":score": rule_score,
            },
        )?;
        Ok(changed == 1)
    }

    pub fn list(&self, f: &Filter) -> Result<Vec<JobRecord>> {
        let conn = self.conn.lock().expect("mutex zatruty");
        let since = Utc::now().timestamp() - NEW_WINDOW_SECS;
        let sql = format!(
            "SELECT {COLUMNS} FROM jobs
             WHERE COALESCE(ai_score, rule_score) >= :min
               AND (:q = '' OR title LIKE '%' || :q || '%'
                            OR company LIKE '%' || :q || '%'
                            OR tags LIKE '%' || :q || '%')
               AND (:only_new = 0 OR first_seen >= :since)
             ORDER BY COALESCE(ai_score, rule_score) DESC,
                      COALESCE(published_at, first_seen) DESC
             LIMIT :limit"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            named_params! {
                ":min": f.min_score,
                ":q": f.query,
                ":only_new": f.only_new,
                ":since": since,
                ":limit": f.limit as i64,
            },
            row_to_record,
        )?;
        // `collect::<Result<Vec<_>, _>>()` zamienia iterator wyników w jeden wynik.
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn get(&self, id: &str) -> Result<Option<JobRecord>> {
        let conn = self.conn.lock().expect("mutex zatruty");
        let sql = format!("SELECT {COLUMNS} FROM jobs WHERE id = ?1");
        conn.query_row(&sql, [id], row_to_record)
            .optional()
            .map_err(Into::into)
    }

    pub fn save_analysis(&self, id: &str, analysis: &Analysis) -> Result<()> {
        let conn = self.conn.lock().expect("mutex zatruty");
        conn.execute(
            "UPDATE jobs SET ai_score = ?2, ai_json = ?3 WHERE id = ?1",
            rusqlite::params![id, analysis.score, serde_json::to_string(analysis)?],
        )?;
        Ok(())
    }

    pub fn stats(&self, threshold: u8) -> Result<Stats> {
        let conn = self.conn.lock().expect("mutex zatruty");
        let since = Utc::now().timestamp() - NEW_WINDOW_SECS;
        conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(COALESCE(ai_score, rule_score) >= ?1), 0),
                    COALESCE(SUM(first_seen >= ?2), 0)
             FROM jobs",
            rusqlite::params![threshold, since],
            |r| {
                Ok(Stats {
                    total: r.get(0)?,
                    matched: r.get(1)?,
                    new: r.get(2)?,
                })
            },
        )
        .map_err(Into::into)
    }
}

fn row_to_record(row: &Row<'_>) -> rusqlite::Result<JobRecord> {
    let tags: String = row.get("tags")?;
    let ai_json: Option<String> = row.get("ai_json")?;
    let published: Option<i64> = row.get("published_at")?;
    let first_seen: i64 = row.get("first_seen")?;

    Ok(JobRecord {
        job: Job {
            id: row.get("id")?,
            source: row.get("source")?,
            external_id: row.get("external_id")?,
            title: row.get("title")?,
            company: row.get("company")?,
            location: row.get("location")?,
            remote: row.get("remote")?,
            url: row.get("url")?,
            description: row.get("description")?,
            tags: serde_json::from_str(&tags).unwrap_or_default(),
            salary: row.get("salary")?,
            published_at: published.and_then(|t| DateTime::from_timestamp(t, 0)),
        },
        rule_score: row.get("rule_score")?,
        ai: ai_json.and_then(|j| serde_json::from_str(&j).ok()),
        first_seen: DateTime::from_timestamp(first_seen, 0).unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(company: &str, title: &str) -> Job {
        Job::new(
            "t",
            "1",
            title,
            company,
            "Remote",
            true,
            "https://x",
            "desc",
            vec!["rust".into()],
            None,
            None,
        )
    }

    #[test]
    fn duplicates_are_ignored() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.insert_if_new(&job("ACME", "Rust Dev"), 50).unwrap());
        assert!(!db.insert_if_new(&job("acme", "Rust  Dev!"), 50).unwrap());
        assert_eq!(db.stats(60).unwrap().total, 1);
    }

    #[test]
    fn list_filters_and_sorts_by_score() {
        let db = Db::open_in_memory().unwrap();
        db.insert_if_new(&job("A", "Low"), 10).unwrap();
        db.insert_if_new(&job("B", "High"), 90).unwrap();

        let all = db.list(&Filter::default()).unwrap();
        assert_eq!(all[0].job.title, "High");

        let strong = db
            .list(&Filter {
                min_score: 50,
                ..Filter::default()
            })
            .unwrap();
        assert_eq!(strong.len(), 1);

        let searched = db
            .list(&Filter {
                query: "low".into(),
                ..Filter::default()
            })
            .unwrap();
        assert_eq!(searched.len(), 1);
    }

    #[test]
    fn analysis_overrides_rule_score() {
        let db = Db::open_in_memory().unwrap();
        let j = job("A", "Dev");
        db.insert_if_new(&j, 10).unwrap();
        let a = Analysis {
            score: 95,
            summary: "ok".into(),
            matched_skills: vec![],
            missing_skills: vec![],
            red_flags: vec![],
        };
        db.save_analysis(&j.id, &a).unwrap();
        let rec = db.get(&j.id).unwrap().unwrap();
        assert_eq!(rec.best_score(), 95);
        assert_eq!(db.stats(60).unwrap().matched, 1);
    }
}
