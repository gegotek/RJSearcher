//! Remotive – https://remotive.com/api/remote-jobs
//!
//! Regulamin API prosi o rzadkie odpytywanie (kilka razy dziennie) i link do źródła,
//! dlatego robimy jeden request na kategorię.

use super::JobSource;
use crate::model::Job;
use crate::text::strip_html;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::Deserialize;

pub struct Remotive;

#[derive(Deserialize)]
struct Response {
    jobs: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    id: u64,
    url: String,
    title: String,
    company_name: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    candidate_required_location: String,
    #[serde(default)]
    salary: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    publication_date: String,
}

#[async_trait]
impl JobSource for Remotive {
    fn name(&self) -> &'static str {
        "remotive"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Job>> {
        let resp: Response = client
            .get("https://remotive.com/api/remote-jobs")
            .query(&[("category", "software-dev"), ("limit", "200")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp.jobs.into_iter().map(into_job).collect())
    }
}

fn into_job(i: Item) -> Job {
    let published = NaiveDateTime::parse_from_str(&i.publication_date, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|n| DateTime::<Utc>::from_naive_utc_and_offset(n, Utc));

    Job::new(
        "remotive",
        i.id.to_string(),
        i.title,
        i.company_name,
        i.candidate_required_location,
        true, // cały serwis jest zdalny
        i.url,
        strip_html(&i.description),
        i.tags,
        Some(i.salary).filter(|s| !s.is_empty()),
        published,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_payload() {
        let json = r#"{"jobs":[{"id":1,"url":"https://remotive.com/x","title":"Rust Dev",
            "company_name":"ACME","tags":["rust"],"candidate_required_location":"Worldwide",
            "salary":"","description":"<p>Hi</p>","publication_date":"2026-10-01T10:00:00"}]}"#;
        let resp: Response = serde_json::from_str(json).unwrap();
        let job = into_job(resp.jobs.into_iter().next().unwrap());
        assert_eq!(job.title, "Rust Dev");
        assert_eq!(job.description, "Hi");
        assert!(job.salary.is_none());
        assert!(job.published_at.is_some());
    }
}
