//! RemoteJob AI – inteligentne wyszukiwanie i analiza ofert pracy.
//!
//! Przepływ danych:
//!
//! ```text
//! sources ──► model::Job ──► matching (reguły) ──► db (dedup) ──► llm (analiza) ──► web / notify
//! ```

pub mod config;
pub mod db;
pub mod llm;
pub mod matching;
pub mod model;
pub mod notify;
pub mod pipeline;
pub mod sources;
pub mod text;
pub mod web;
