//! RemoteOK – https://remoteok.com/api
//!
//! Regulamin wymaga linkowania do RemoteOK jako źródła – zachowujemy oryginalny URL oferty.
//! Pierwszy element tablicy to nota prawna, nie oferta.

use super::JobSource;
use crate::model::Job;
use crate::text::strip_html;
use anyhow::Result;
use async_trait::async_trait;
use chrono::DateTime;
use serde::Deserialize;
use serde_json::Value;

pub struct RemoteOk;

#[derive(Deserialize)]
struct Item {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    position: String,
    #[serde(default)]
    company: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    salary_min: Option<u64>,
    #[serde(default)]
    salary_max: Option<u64>,
    #[serde(default)]
    epoch: Option<i64>,
}

#[async_trait]
impl JobSource for RemoteOk {
    fn name(&self) -> &'static str {
        "remoteok"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Job>> {
        let raw: Vec<Value> = client
            .get("https://remoteok.com/api")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(parse(raw))
    }
}

fn parse(raw: Vec<Value>) -> Vec<Job> {
    raw.into_iter()
        // `filter_map` + `ok()` – pomijamy elementy, które nie są ofertami (np. nota prawna).
        .filter_map(|v| serde_json::from_value::<Item>(v).ok())
        .filter(|i| !i.position.is_empty() && !i.company.is_empty())
        .map(|i| {
            let salary = match (i.salary_min, i.salary_max) {
                (Some(lo), Some(hi)) if hi > 0 => Some(format!("${lo}–${hi}")),
                _ => None,
            };
            Job::new(
                "remoteok",
                i.slug,
                i.position,
                i.company,
                if i.location.is_empty() {
                    "Worldwide".to_string()
                } else {
                    i.location
                },
                true,
                i.url,
                strip_html(&i.description),
                i.tags,
                salary,
                i.epoch.and_then(|t| DateTime::from_timestamp(t, 0)),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_legal_notice_and_parses_jobs() {
        let raw: Vec<Value> = serde_json::from_str(
            r#"[{"legal":"terms"},
                {"slug":"rust-1","position":"Rust Dev","company":"ACME","url":"https://remoteok.com/x",
                 "tags":["rust"],"description":"<b>Hi</b>","salary_min":100000,"salary_max":150000,"epoch":1790000000}]"#,
        )
        .unwrap();
        let jobs = parse(raw);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].salary.as_deref(), Some("$100000–$150000"));
    }
}
