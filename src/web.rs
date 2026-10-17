//! Dashboard WWW (axum + maud) i proste JSON API.
//!
//! 🦀 Nauka: `Arc` (współdzielony stan), extractory axum (`State`, `Query`, `Path`),
//! typy odpowiedzi (`IntoResponse`), szablony typowane w Rust (maud) – HTML jest
//! domyślnie escapowany, więc treść ofert nie wstrzyknie nam skryptów.

use crate::config::Profile;
use crate::db::{Db, Filter, JobRecord};
use crate::llm::LlmClient;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub struct AppState {
    pub db: Db,
    pub profile: Profile,
    pub llm: Option<LlmClient>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/analyze/{id}", post(analyze))
        .route("/api/jobs", get(api_jobs))
        .route("/api/stats", get(api_stats))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state)
}

#[derive(Debug, Deserialize, Default)]
struct DashQuery {
    #[serde(default)]
    q: String,
    min: Option<u8>,
    /// Checkbox: obecny (`new=1`) lub nie.
    new: Option<String>,
}

impl DashQuery {
    fn filter(&self, default_min: u8) -> Filter {
        Filter {
            min_score: self.min.unwrap_or(default_min),
            query: self.q.trim().to_string(),
            only_new: self.new.is_some(),
            limit: 200,
        }
    }
}

/// Błąd handlera → odpowiedź HTTP. Szczegóły logujemy, użytkownikowi pokazujemy skrót.
struct AppError(StatusCode, String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!(error = %err, "błąd handlera");
        AppError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "wewnętrzny błąd serwera".into(),
        )
    }
}

async fn dashboard(
    State(st): State<Arc<AppState>>,
    Query(q): Query<DashQuery>,
) -> Result<Markup, AppError> {
    let filter = q.filter(0);
    let jobs = st.db.list(&filter)?;
    let stats = st.db.stats(st.profile.match_threshold)?;
    let llm_enabled = st.llm.is_some();

    Ok(html! {
        (DOCTYPE)
        html lang="pl" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "RemoteJob AI" }
                style { (PreEscaped(CSS)) }
            }
            body {
                main {
                    header {
                        h1 { "RemoteJob AI" }
                        p.muted { "AI-powered job discovery & analysis" }
                    }

                    section.stats {
                        (stat_card("Oferty", stats.total))
                        (stat_card("Dopasowane", stats.matched))
                        (stat_card("Nowe", stats.new))
                    }

                    form.filters method="get" action="/" {
                        input type="search" name="q" placeholder="Szukaj: tytuł, firma, tag…" value=(filter.query);
                        select name="min" {
                            @for (val, label) in [(0u8, "Wszystkie"), (40, "≥ 40%"), (60, "≥ 60%"), (80, "≥ 80%")] {
                                option value=(val) selected[filter.min_score == val] { (label) }
                            }
                        }
                        label { input type="checkbox" name="new" value="1" checked[filter.only_new]; " tylko nowe (24h)" }
                        button type="submit" { "Filtruj" }
                    }

                    @if jobs.is_empty() {
                        p.muted { "Brak ofert. Uruchom: " code { "rjsearcher fetch" } }
                    }
                    @for rec in &jobs {
                        (job_card(rec, llm_enabled))
                    }

                    footer.muted {
                        "Dane z Remotive, RemoteOK i Arbeitnow – kliknij tytuł, aby przejść do oryginalnej oferty."
                    }
                }
            }
        }
    })
}

fn stat_card(label: &str, value: u32) -> Markup {
    html! { div.card { div.muted.small { (label) } div.big { (value) } } }
}

fn job_card(rec: &JobRecord, llm_enabled: bool) -> Markup {
    let job = &rec.job;
    let score = rec.best_score();
    let class = match score {
        80..=100 => "score hi",
        60..=79 => "score mid",
        _ => "score lo",
    };

    html! {
        article.job {
            div.row {
                div {
                    h3 {
                        // Adres pochodzi z zewnętrznego API – dopuszczamy tylko http(s).
                        @if let Some(url) = safe_url(&job.url) {
                            a href=(url) target="_blank" rel="noopener noreferrer" { (job.title) }
                        } @else {
                            (job.title)
                        }
                    }
                    p.muted.small {
                        (job.company) " · " (job.location) " · " (job.source)
                        @if let Some(s) = &job.salary { " · " (s) }
                    }
                }
                div.right {
                    div class=(class) { (score) "%" }
                    div.muted.small { @if rec.ai.is_some() { "AI match" } @else { "rule match" } }
                }
            }
            @if !job.tags.is_empty() {
                p { @for t in job.tags.iter().take(8) { span.tag { (t) } } }
            }
            @if let Some(a) = &rec.ai {
                div.analysis {
                    p { (a.summary) }
                    @if !a.matched_skills.is_empty() { p.small { "✅ " (a.matched_skills.join(", ")) } }
                    @if !a.missing_skills.is_empty() { p.small { "➖ " (a.missing_skills.join(", ")) } }
                    @if !a.red_flags.is_empty() { p.small.warn { "⚠️ " (a.red_flags.join("; ")) } }
                }
            } @else if llm_enabled {
                form method="post" action=(format!("/analyze/{}", job.id)) {
                    button type="submit" { "Analizuj AI" }
                }
            }
        }
    }
}

/// Zwraca URL tylko jeśli używa `http`/`https` (blokuje np. `javascript:`).
fn safe_url(url: &str) -> Option<&str> {
    let lower = url.trim().to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")).then_some(url.trim())
}

async fn analyze(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Redirect, AppError> {
    let llm = st.llm.as_ref().ok_or_else(|| {
        AppError(
            StatusCode::SERVICE_UNAVAILABLE,
            "LLM nie jest skonfigurowany".into(),
        )
    })?;
    let rec = st
        .db
        .get(&id)?
        .ok_or_else(|| AppError(StatusCode::NOT_FOUND, "nie ma takiej oferty".into()))?;

    let analysis = llm.analyze(&rec.job, &st.profile).await.map_err(|e| {
        tracing::error!(error = %e, "analiza LLM nie powiodła się");
        AppError(
            StatusCode::BAD_GATEWAY,
            "analiza LLM nie powiodła się".into(),
        )
    })?;
    st.db.save_analysis(&id, &analysis)?;
    Ok(Redirect::to("/"))
}

async fn api_jobs(
    State(st): State<Arc<AppState>>,
    Query(q): Query<DashQuery>,
) -> Result<Json<Value>, AppError> {
    let jobs = st.db.list(&q.filter(0))?;
    let items: Vec<Value> = jobs
        .iter()
        .map(|r| {
            json!({
                "job": r.job,
                "score": r.best_score(),
                "rule_score": r.rule_score,
                "analysis": r.ai,
                "first_seen": r.first_seen,
            })
        })
        .collect();
    Ok(Json(json!({ "count": items.len(), "jobs": items })))
}

async fn api_stats(State(st): State<Arc<AppState>>) -> Result<Json<crate::db::Stats>, AppError> {
    Ok(Json(st.db.stats(st.profile.match_threshold)?))
}

const CSS: &str = r#"
:root { color-scheme: dark; }
* { box-sizing: border-box; }
body { margin: 0; background: #000; color: #f2f2f2; font: 15px/1.5 system-ui, sans-serif; }
main { max-width: 860px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { margin: 0; font-size: 22px; } h3 { margin: 0; font-size: 16px; }
a { color: inherit; } a:hover { text-decoration: underline; }
.muted { color: #9a9a9a; } .small { font-size: 12px; } .warn { color: #e6b450; }
.stats { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin: 20px 0; }
.card { border: 1px solid #2a2a2a; border-radius: 10px; padding: 12px 14px; }
.big { font-size: 28px; font-weight: 600; }
.filters { display: flex; gap: 8px; flex-wrap: wrap; align-items: center; margin-bottom: 16px; }
input[type=search], select, button { background: #111; color: inherit; border: 1px solid #2a2a2a; border-radius: 8px; padding: 8px 10px; font: inherit; }
input[type=search] { flex: 1; min-width: 200px; }
button { cursor: pointer; } button:hover { background: #1c1c1c; }
.job { border-top: 1px solid #1f1f1f; padding: 14px 0; }
.row { display: flex; justify-content: space-between; gap: 16px; }
.right { text-align: right; flex-shrink: 0; }
.score { font-size: 18px; font-weight: 700; } .hi { color: #5fd38d; } .mid { color: #e6b450; } .lo { color: #9a9a9a; }
.tag { display: inline-block; background: #161616; border: 1px solid #262626; border-radius: 999px; padding: 1px 8px; margin: 0 6px 4px 0; font-size: 12px; }
.analysis { background: #0d0d0d; border: 1px solid #1f1f1f; border-radius: 8px; padding: 4px 12px; }
footer { margin-top: 32px; font-size: 12px; }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_url_blocks_dangerous_schemes() {
        assert_eq!(
            safe_url("https://remotive.com/x"),
            Some("https://remotive.com/x")
        );
        assert_eq!(safe_url("  HTTP://x.y "), Some("HTTP://x.y"));
        assert_eq!(safe_url("javascript:alert(1)"), None);
        assert_eq!(safe_url("data:text/html,hi"), None);
    }

    #[test]
    fn card_escapes_html_in_titles() {
        use crate::model::Job;
        let rec = JobRecord {
            job: Job::new(
                "t",
                "1",
                "<script>alert(1)</script>",
                "ACME",
                "x",
                true,
                "https://x",
                "",
                vec![],
                None,
                None,
            ),
            rule_score: 70,
            ai: None,
            first_seen: chrono::Utc::now(),
        };
        let out = job_card(&rec, false).into_string();
        assert!(!out.contains("<script>"));
        assert!(out.contains("&lt;script&gt;"));
    }
}
