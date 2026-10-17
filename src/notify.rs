//! Powiadomienia przez webhook (Discord / Slack-compatible / własny endpoint).
//!
//! Ustaw `RJ_WEBHOOK_URL`. Wysyłamy JSON z polami `content` (Discord) i `text` (Slack).
//!
//! 🦀 Nauka: `Option`, `format!`, budowanie `String` z iteratorów.

use crate::db::JobRecord;
use anyhow::Result;
use serde_json::json;

pub fn format_message(matches: &[JobRecord]) -> String {
    let mut msg = format!(
        "🔔 RemoteJob AI: {} nowych dopasowanych ofert\n",
        matches.len()
    );
    for rec in matches.iter().take(10) {
        msg.push_str(&format!(
            "• [{}%] {} @ {} — {}\n",
            rec.best_score(),
            rec.job.title,
            rec.job.company,
            rec.job.url
        ));
    }
    if matches.len() > 10 {
        msg.push_str(&format!("…i {} więcej\n", matches.len() - 10));
    }
    msg
}

pub async fn send_webhook(
    client: &reqwest::Client,
    url: &str,
    matches: &[JobRecord],
) -> Result<()> {
    if matches.is_empty() {
        return Ok(());
    }
    let msg = format_message(matches);
    client
        .post(url)
        .json(&json!({ "content": msg, "text": msg }))
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::JobRecord;
    use crate::model::Job;

    #[test]
    fn message_lists_matches_and_truncates() {
        let rec = JobRecord {
            job: Job::new(
                "t",
                "1",
                "Rust Dev",
                "ACME",
                "Remote",
                true,
                "https://x",
                "",
                vec![],
                None,
                None,
            ),
            rule_score: 80,
            ai: None,
            first_seen: chrono::Utc::now(),
        };
        let many = vec![rec; 12];
        let msg = format_message(&many);
        assert!(msg.contains("12 nowych"));
        assert!(msg.contains("[80%] Rust Dev @ ACME"));
        assert!(msg.contains("…i 2 więcej"));
    }
}
