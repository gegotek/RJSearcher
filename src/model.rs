//! Główne typy domenowe.
//!
//! 🦀 Nauka: `struct`, `derive`, `Option<T>`, własność (ownership) danych w `String`/`Vec`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Oferta pracy w ujednoliconym formacie (niezależnym od źródła).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Job {
    /// Stabilny klucz używany też w URL-ach dashboardu (= `fingerprint`).
    pub id: String,
    /// Nazwa źródła, np. `remotive`.
    pub source: String,
    /// ID oferty w źródle.
    pub external_id: String,
    pub title: String,
    pub company: String,
    pub location: String,
    pub remote: bool,
    pub url: String,
    /// Opis jako czysty tekst (HTML już wycięty).
    pub description: String,
    pub tags: Vec<String>,
    pub salary: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
}

impl Job {
    /// Tworzy ofertę i wylicza `id` na podstawie firmy + tytułu.
    ///
    /// Dzięki temu ta sama oferta z dwóch źródeł dostaje to samo `id`
    /// i baza ją zdeduplikuje.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: &str,
        external_id: impl Into<String>,
        title: impl Into<String>,
        company: impl Into<String>,
        location: impl Into<String>,
        remote: bool,
        url: impl Into<String>,
        description: impl Into<String>,
        tags: Vec<String>,
        salary: Option<String>,
        published_at: Option<DateTime<Utc>>,
    ) -> Self {
        let title = title.into();
        let company = company.into();
        Self {
            id: fingerprint(&company, &title),
            source: source.to_string(),
            external_id: external_id.into(),
            title,
            company,
            location: location.into(),
            remote,
            url: url.into(),
            description: description.into(),
            tags,
            salary,
            published_at,
        }
    }
}

/// Normalizuje tekst: małe litery, tylko znaki alfanumeryczne, pojedyncze spacje.
pub fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Odcisk palca oferty: SHA-256 z `firma|tytuł` po normalizacji.
pub fn fingerprint(company: &str, title: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalize(company).as_bytes());
    hasher.update(b"|");
    hasher.update(normalize(title).as_bytes());
    // `finalize()` zwraca bajty; zamieniamy na hex.
    hasher
        .finalize()
        .iter()
        .take(12) // 96 bitów wystarczy do dedup
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Wynik analizy oferty przez LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Analysis {
    /// Dopasowanie 0–100.
    pub score: u8,
    pub summary: String,
    #[serde(default)]
    pub matched_skills: Vec<String>,
    #[serde(default)]
    pub missing_skills: Vec<String>,
    #[serde(default)]
    pub red_flags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_punctuation_and_case() {
        assert_eq!(
            normalize("Senior C++ Engineer (Remote)!"),
            "senior c engineer remote"
        );
    }

    #[test]
    fn fingerprint_is_stable_across_formatting() {
        let a = fingerprint("ACME Corp.", "Senior Rust Engineer");
        let b = fingerprint("acme corp", "senior  rust engineer!");
        assert_eq!(a, b);
        assert_eq!(a.len(), 24);
    }

    #[test]
    fn fingerprint_differs_for_different_jobs() {
        assert_ne!(fingerprint("A", "Rust Dev"), fingerprint("A", "Python Dev"));
    }
}
