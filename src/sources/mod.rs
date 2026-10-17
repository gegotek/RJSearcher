//! Źródła ofert pracy.
//!
//! Używamy wyłącznie publicznych API, które jawnie udostępniają oferty
//! (Remotive, RemoteOK, Arbeitnow) – bez scrapowania stron. Zawsze zachowujemy
//! link do oryginalnej oferty (`Job::url`), co jest wymagane przez ich regulaminy.
//!
//! 🦀 Nauka: `trait`, `dyn Trait` + `Box`, `async_trait`, `join_all` (współbieżność).

mod arbeitnow;
mod remoteok;
mod remotive;

use crate::model::Job;
use anyhow::Result;
use async_trait::async_trait;

/// Wspólny interfejs źródła. Dodanie nowego źródła = implementacja tego traita.
#[async_trait]
pub trait JobSource: Send + Sync {
    fn name(&self) -> &'static str;
    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Job>>;
}

/// Wszystkie wbudowane źródła.
pub fn all() -> Vec<Box<dyn JobSource>> {
    vec![
        Box::new(remotive::Remotive),
        Box::new(remoteok::RemoteOk),
        Box::new(arbeitnow::Arbeitnow),
    ]
}

/// Pobiera ze wszystkich źródeł równolegle. Błąd jednego źródła nie przerywa pozostałych.
pub async fn fetch_all(client: &reqwest::Client, sources: &[Box<dyn JobSource>]) -> Vec<Job> {
    let futures = sources.iter().map(|s| async move {
        let result = s.fetch(client).await;
        (s.name(), result)
    });

    let mut jobs = Vec::new();
    for (name, result) in futures::future::join_all(futures).await {
        match result {
            Ok(mut batch) => {
                tracing::info!(source = name, count = batch.len(), "pobrano oferty");
                jobs.append(&mut batch);
            }
            Err(err) => tracing::warn!(source = name, error = %err, "źródło zawiodło"),
        }
    }
    jobs
}

/// Buduje klienta HTTP z rozsądnym User-Agent i timeoutem.
pub fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(concat!(
            "RemoteJobAI/",
            env!("CARGO_PKG_VERSION"),
            " (personal job search tool)"
        ))
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}
