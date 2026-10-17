//! Arbeitnow – https://www.arbeitnow.com/api/job-board-api (publiczne, bez klucza).

use super::JobSource;
use crate::model::Job;
use crate::text::strip_html;
use anyhow::Result;
use async_trait::async_trait;
use chrono::DateTime;
use serde::Deserialize;

pub struct Arbeitnow;

#[derive(Deserialize)]
struct Response {
    data: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    slug: String,
    company_name: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    remote: bool,
    url: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    location: String,
    #[serde(default)]
    created_at: Option<i64>,
}

#[async_trait]
impl JobSource for Arbeitnow {
    fn name(&self) -> &'static str {
        "arbeitnow"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Job>> {
        let resp: Response = client
            .get("https://www.arbeitnow.com/api/job-board-api")
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(resp
            .data
            .into_iter()
            .map(|i| {
                Job::new(
                    "arbeitnow",
                    i.slug,
                    i.title,
                    i.company_name,
                    i.location,
                    i.remote,
                    i.url,
                    strip_html(&i.description),
                    i.tags,
                    None,
                    i.created_at.and_then(|t| DateTime::from_timestamp(t, 0)),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample_payload() {
        let json = r#"{"data":[{"slug":"a","company_name":"ACME","title":"Dev","description":"<p>x</p>",
            "remote":true,"url":"https://arbeitnow.com/a","tags":[],"location":"Berlin","created_at":1790000000}]}"#;
        let resp: Response = serde_json::from_str(json).unwrap();
        assert_eq!(resp.data.len(), 1);
        assert!(resp.data[0].remote);
    }
}
