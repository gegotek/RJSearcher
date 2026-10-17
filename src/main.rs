//! CLI RemoteJob AI.
//!
//! 🦀 Nauka: `clap` (derive), `enum` z danymi, `match`, `#[tokio::main]`, graceful shutdown.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rjsearcher::config::Profile;
use rjsearcher::db::{Db, Filter};
use rjsearcher::llm::LlmClient;
use rjsearcher::web::{self, AppState};
use rjsearcher::{notify, pipeline, sources};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "rjsearcher",
    version,
    about = "RemoteJob AI – wyszukiwanie i analiza ofert pracy"
)]
struct Cli {
    /// Plik bazy SQLite.
    #[arg(long, global = true, env = "RJ_DB", default_value = "rjsearcher.db")]
    db: PathBuf,

    /// Plik profilu kandydata.
    #[arg(
        long,
        global = true,
        env = "RJ_PROFILE",
        default_value = "profile.toml"
    )]
    profile: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pobierz oferty ze wszystkich źródeł, zdeduplikuj i oceń.
    Fetch {
        /// Dodatkowo przeanalizuj LLM-em N najlepszych ofert (wymaga konfiguracji LLM).
        #[arg(long, default_value_t = 0)]
        analyze: usize,
        /// Nie wysyłaj powiadomienia webhook, nawet gdy ustawiono RJ_WEBHOOK_URL.
        #[arg(long)]
        no_notify: bool,
    },
    /// Wyświetl oferty w terminalu.
    List {
        #[arg(long, default_value_t = 60)]
        min: u8,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Szukaj w tytule/firmie/tagach.
        #[arg(short, long, default_value = "")]
        query: String,
    },
    /// Przeanalizuj LLM-em najlepsze oferty, które jeszcze nie mają analizy.
    Analyze {
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
    /// Uruchom dashboard WWW.
    Serve {
        #[arg(long, env = "RJ_ADDR", default_value = "127.0.0.1:3000")]
        addr: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let profile = Profile::load(&cli.profile)?;
    let db = Db::open(&cli.db)?;

    match cli.command {
        Command::Fetch { analyze, no_notify } => {
            let client = sources::http_client()?;
            let report = pipeline::fetch_and_store(&db, &profile, &client, &sources::all()).await?;
            println!(
                "Pobrano {} ofert, w tym {} nowych ({} dopasowanych).",
                report.fetched,
                report.new,
                report.new_matches.len()
            );

            if analyze > 0 {
                let llm = LlmClient::from_env(client.clone())?;
                let n = pipeline::analyze_pending(&db, &profile, &llm, analyze).await?;
                println!("Przeanalizowano LLM: {n}");
            }

            if !no_notify && let Ok(url) = std::env::var("RJ_WEBHOOK_URL") {
                notify::send_webhook(&client, &url, &report.new_matches)
                    .await
                    .context("wysyłka webhooka nie powiodła się")?;
                println!("Wysłano powiadomienie.");
            }
        }

        Command::List { min, limit, query } => {
            let jobs = db.list(&Filter {
                min_score: min,
                limit,
                query,
                ..Filter::default()
            })?;
            if jobs.is_empty() {
                println!("Brak ofert (uruchom `rjsearcher fetch`).");
            }
            for rec in jobs {
                println!(
                    "{:>3}%  {} @ {}\n      {}",
                    rec.best_score(),
                    rec.job.title,
                    rec.job.company,
                    rec.job.url
                );
            }
        }

        Command::Analyze { limit } => {
            let llm = LlmClient::from_env(sources::http_client()?)?;
            let n = pipeline::analyze_pending(&db, &profile, &llm, limit).await?;
            println!("Przeanalizowano: {n}");
        }

        Command::Serve { addr } => {
            // LLM jest opcjonalny – bez niego dashboard działa na samych regułach.
            let llm = match LlmClient::from_env(sources::http_client()?) {
                Ok(llm) => Some(llm),
                Err(e) => {
                    tracing::warn!("LLM wyłączony: {e}");
                    None
                }
            };
            let state = Arc::new(AppState { db, profile, llm });
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .with_context(|| format!("nie można nasłuchiwać na {addr}"))?;
            tracing::info!("dashboard: http://{addr}");
            axum::serve(listener, web::router(state))
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await?;
        }
    }
    Ok(())
}
