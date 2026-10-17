//! Profil użytkownika (`profile.toml`) i konfiguracja z env.
//!
//! 🦀 Nauka: `serde` + `toml`, `Default`, obsługa błędów z `anyhow::Context`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Słowo kluczowe z wagą (ile punktów dodaje do dopasowania).
#[derive(Debug, Clone, Deserialize)]
pub struct Keyword {
    pub term: String,
    pub weight: u32,
}

/// Profil, względem którego oceniamy oferty.
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    /// Krótki opis kandydata – trafia do promptu LLM.
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    /// Jeśli oferta zawiera którąś z fraz, dostaje wynik 0.
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default = "default_true")]
    pub remote_only: bool,
    /// Frazy wyszukiwania wysyłane do źródeł, które to wspierają.
    #[serde(default)]
    pub search_terms: Vec<String>,
    /// Od jakiego wyniku oferta liczy się jako „dopasowana”.
    #[serde(default = "default_threshold")]
    pub match_threshold: u8,
}

fn default_true() -> bool {
    true
}

fn default_threshold() -> u8 {
    60
}

impl Profile {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("nie można odczytać profilu: {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("niepoprawny TOML w {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_profile_with_defaults() {
        let p: Profile = toml::from_str("").unwrap();
        assert!(p.remote_only);
        assert_eq!(p.match_threshold, 60);
        assert!(p.keywords.is_empty());
    }

    #[test]
    fn bundled_example_profile_is_valid() {
        let raw = include_str!("../profile.toml");
        let p: Profile = toml::from_str(raw).unwrap();
        assert!(!p.keywords.is_empty());
    }
}
