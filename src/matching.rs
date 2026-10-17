//! Szybkie, deterministyczne dopasowanie oferty do profilu (bez LLM).
//!
//! Służy jako tani pre-filtr: LLM (drogi) analizuje tylko oferty, które
//! przeszły ten etap.
//!
//! 🦀 Nauka: iteratory (`iter().filter().map().sum()`), referencje, `u32` vs `u8`, testy.

use crate::config::Profile;
use crate::model::Job;

/// Czy `term` występuje w `haystack` jako osobne słowo?
///
/// Zwykłe `contains` łapałoby „rust” w „trust”. Sprawdzamy, że znaki po obu stronach
/// trafienia nie są alfanumeryczne (więc `c++` w „c++,” działa, a „rust” w „trust” nie).
/// Oba argumenty muszą być już zapisane małymi literami.
pub fn contains_word(haystack: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    haystack.match_indices(term).any(|(start, matched)| {
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let after_ok = haystack[start + matched.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric());
        before_ok && after_ok
    })
}

/// Zwraca wynik 0–100.
pub fn score(job: &Job, profile: &Profile) -> u8 {
    if profile.remote_only && !job.remote {
        return 0;
    }

    let title = job.title.to_lowercase();
    let tags = job.tags.join(" ").to_lowercase();
    let body = job.description.to_lowercase();
    let haystack = format!("{title} {tags} {body}");

    if profile
        .exclude
        .iter()
        .any(|bad| contains_word(&haystack, &bad.to_lowercase()))
    {
        return 0;
    }

    let total: u32 = profile
        .keywords
        .iter()
        .map(|kw| {
            let term = kw.term.to_lowercase();
            if contains_word(&title, &term) {
                kw.weight * 2 // tytuł jest mocnym sygnałem
            } else if contains_word(&tags, &term) || contains_word(&body, &term) {
                kw.weight
            } else {
                0
            }
        })
        .sum();

    total.min(100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Keyword;

    fn profile() -> Profile {
        Profile {
            summary: String::new(),
            keywords: vec![
                Keyword {
                    term: "rust".into(),
                    weight: 30,
                },
                Keyword {
                    term: "python".into(),
                    weight: 20,
                },
            ],
            exclude: vec!["internship".into()],
            remote_only: true,
            search_terms: vec![],
            match_threshold: 60,
        }
    }

    fn job(title: &str, desc: &str, remote: bool) -> Job {
        Job::new(
            "test",
            "1",
            title,
            "ACME",
            "Worldwide",
            remote,
            "https://x",
            desc,
            vec![],
            None,
            None,
        )
    }

    #[test]
    fn matches_whole_words_only() {
        assert!(contains_word("senior rust engineer", "rust"));
        assert!(contains_word("we use c++, python", "c++"));
        assert!(contains_word("rust", "rust"));
        assert!(!contains_word("we build trust", "rust"));
        assert!(!contains_word("rusty", "rust"));
        assert!(!contains_word("anything", ""));
    }

    #[test]
    fn title_hit_counts_double() {
        let p = profile();
        assert_eq!(score(&job("Rust Engineer", "", true), &p), 60);
        assert_eq!(score(&job("Engineer", "we use rust", true), &p), 30);
    }

    #[test]
    fn score_is_capped_at_100() {
        let p = profile();
        assert_eq!(score(&job("Rust Python Engineer", "", true), &p), 100);
    }

    #[test]
    fn excluded_and_non_remote_score_zero() {
        let p = profile();
        assert_eq!(score(&job("Rust Internship", "", true), &p), 0);
        assert_eq!(score(&job("Rust Engineer", "", false), &p), 0);
    }
}
