# Nauka Rusta i budowy AI SaaS na RemoteJob AI

Każdy moduł ilustruje inny obszar Rusta (patrz komentarze `🦀 Nauka:` na górze plików).
Poniżej: co przeczytać, co samodzielnie zmienić, i jak rozwinąć projekt do SaaS.

## Etap 1 – Podstawy (czytaj + ćwicz)

| Plik | Pojęcia | Ćwiczenie |
|---|---|---|
| `src/text.rs` | `&str` vs `String`, iteratory `chars()`, testy | Dodaj dekodowanie encji numerycznych (`&#8211;`) |
| `src/model.rs` | `struct`, `derive`, `Option`, `impl` | Dodaj pole `seniority` wykrywane z tytułu |
| `src/matching.rs` | iteratory, closures, referencje | Dodaj ujemne wagi (np. `php` = -20) |
| `src/config.rs` | `serde`, `Default`, `anyhow::Context` | Dodaj do profilu `min_salary` i filtruj po nim |

## Etap 2 – Błędy, własność, trait'y

| Plik | Pojęcia | Ćwiczenie |
|---|---|---|
| `src/db.rs` | `Result`, `?`, `Mutex`, mapowanie wierszy | Zamień `anyhow` na własny `enum DbError` z `thiserror` |
| `src/sources/` | `trait`, `dyn`, `Box`, `async_trait` | Dodaj nowe źródło (np. Himalayas, We Work Remotely RSS) |
| `src/llm.rs` | walidacja niezaufanych danych, `serde_json` | Dodaj retry z exponential backoff przy 429/5xx |

## Etap 3 – Asynchroniczność i sieć

| Plik | Pojęcia | Ćwiczenie |
|---|---|---|
| `src/pipeline.rs` | `async`/`await`, pożyczanie w pętlach | Analizuj oferty współbieżnie (`buffer_unordered(3)`) |
| `src/web.rs` | `Arc`, extractory axum, `IntoResponse` | Dodaj stronę `/job/{id}` z pełnym opisem |
| `src/main.rs` | `clap`, `tokio::main`, graceful shutdown | Dodaj `serve --refresh-hours 6` (zadanie w tle `tokio::spawn`) |

## Etap 4 – Z narzędzia do produktu SaaS

Kolejne kroki (każdy to osobny, realny temat produktowy):

1. **Konta użytkowników** – osobny profil na użytkownika, uwierzytelnianie (sesje / OAuth), tabela `users`.
2. **Migracja na Postgres** – `sqlx` z migracjami i `async` zamiast `Mutex<Connection>`.
3. **Embeddingi zamiast słów kluczowych** – wektory (pgvector / `qdrant`), semantyczne dopasowanie CV ↔ oferta.
4. **Kolejka zadań** – cykliczny fetch i analiza w workerze, limity kosztów LLM na użytkownika.
5. **Billing** – plany (np. Stripe), limity analiz AI na plan.
6. **Obserwowalność** – `tracing` + metryki, śledzenie kosztu tokenów.
7. **Deploy** – `Dockerfile` (multi-stage), fly.io / VPS, sekrety w secret managerze.
8. **Prywatność (RODO)** – CV i profil to dane osobowe: minimalizacja, usuwanie na żądanie, brak logowania treści.

## Sugerowany rytm

- Przeczytaj moduł → uruchom jego testy (`cargo test <moduł>`) → zrób ćwiczenie → dopisz test.
- Uzupełniająco: [The Rust Book](https://doc.rust-lang.org/book/) i `rustlings` (masz już w `~/projects/rustlings`).
- Po każdym ćwiczeniu: `cargo fmt && cargo clippy --all-targets -- -D warnings` – clippy to świetny nauczyciel idiomów.
