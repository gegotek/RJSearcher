//! Główny przepływ: pobierz → oceń regułami → zapisz (z deduplikacją) → analizuj LLM.
//!
//! 🦀 Nauka: łączenie modułów, pożyczanie (`&`) zamiast klonowania, `async fn` w pętli.

use crate::config::Profile;
use crate::db::{Db, Filter, JobRecord};
use crate::llm::LlmClient;
use crate::matching;
use crate::sources::{self, JobSource};
use anyhow::Result;

#[derive(Debug, Default)]
pub struct FetchReport {
    pub fetched: usize,
    pub new: usize,
    /// Nowe oferty, które przekroczyły próg dopasowania.
    pub new_matches: Vec<JobRecord>,
}

pub async fn fetch_and_store(
    db: &Db,
    profile: &Profile,
    client: &reqwest::Client,
    sources: &[Box<dyn JobSource>],
) -> Result<FetchReport> {
    let jobs = sources::fetch_all(client, sources).await;
    let mut report = FetchReport {
        fetched: jobs.len(),
        ..Default::default()
    };

    for job in jobs {
        let score = matching::score(&job, profile);
        if db.insert_if_new(&job, score)? {
            report.new += 1;
            if score >= profile.match_threshold
                && let Some(rec) = db.get(&job.id)?
            {
                report.new_matches.push(rec);
            }
        }
    }
    Ok(report)
}

/// Analizuje LLM-em najlepsze oferty bez analizy (oszczędzamy koszty: tylko `limit` sztuk).
pub async fn analyze_pending(
    db: &Db,
    profile: &Profile,
    llm: &LlmClient,
    limit: usize,
) -> Result<usize> {
    // Kandydaci: wynik regułowy ≥ próg/2 i brak analizy AI.
    let candidates = db.list(&Filter {
        min_score: profile.match_threshold / 2,
        limit: 500,
        ..Filter::default()
    })?;

    let mut done = 0;
    for rec in candidates
        .into_iter()
        .filter(|r| r.ai.is_none())
        .take(limit)
    {
        match llm.analyze(&rec.job, profile).await {
            Ok(analysis) => {
                tracing::info!(title = %rec.job.title, score = analysis.score, "przeanalizowano");
                db.save_analysis(&rec.job.id, &analysis)?;
                done += 1;
            }
            Err(err) => {
                tracing::warn!(title = %rec.job.title, error = %err, "analiza nie powiodła się")
            }
        }
    }
    Ok(done)
}
