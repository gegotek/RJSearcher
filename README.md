# RemoteJob AI (RJSearcher)

Inteligentna aplikacja do wyszukiwania, filtrowania i analizowania ofert pracy zdalnej
dla programistów **C++ / Rust, Python i AI**. Napisana w Ruście – jednocześnie projekt
do nauki Rusta i prototyp produktu typu AI SaaS.

## Co potrafi

- **Zbieranie ofert z dozwolonych źródeł** – publiczne API: Remotive, RemoteOK, Arbeitnow (bez scrapowania HTML).
- **Eliminacja duplikatów** – odcisk SHA-256 z `firma + tytuł`, także między źródłami.
- **Dopasowanie do profilu** – tani pre-filtr regułowy (`profile.toml`), słowa kluczowe z wagami.
- **Analiza wymagań przez LLM** – wynik 0–100, podsumowanie, brakujące umiejętności, red flags.
  Działa z OpenAI i lokalnie (Ollama / LM Studio).
- **Dashboard** WWW (Oferty / Dopasowane / Nowe) + JSON API.
- **Powiadomienia** webhookiem (Discord / Slack) o nowych dopasowanych ofertach.

## Szybki start

```bash
cargo run -- fetch            # pobierz i oceń oferty
cargo run -- list --min 50    # zobacz najlepsze w terminalu
cargo run -- serve            # dashboard: http://127.0.0.1:3000
```

Z analizą LLM:

```bash
export OPENAI_API_KEY=sk-...          # albo RJ_LLM_BASE_URL=http://localhost:11434/v1 (Ollama)
cargo run -- fetch --analyze 10       # przeanalizuj 10 najlepszych ofert
cargo run -- analyze --limit 5        # analiza zaległych
```

Edytuj `profile.toml`, aby dopasować słowa kluczowe, wykluczenia i próg dopasowania.
Zmienne środowiskowe: patrz `.env.example`.

## Architektura

```text
sources ──► model::Job ──► matching ──► db (SQLite, dedup) ──► llm ──► web / notify
```

| Moduł | Rola |
|---|---|
| `src/sources/` | trait `JobSource` + implementacje źródeł |
| `src/model.rs` | `Job`, `Analysis`, fingerprint do deduplikacji |
| `src/matching.rs` | scoring regułowy (dopasowanie całych słów) |
| `src/db.rs` | SQLite (`rusqlite`) |
| `src/llm.rs` | klient LLM zgodny z OpenAI, walidacja JSON z modelu |
| `src/pipeline.rs` | fetch → score → store → analyze |
| `src/web.rs` | dashboard (axum + maud) i API |
| `src/notify.rs` | webhook |

## Bezpieczeństwo i etyka

- Dashboard domyślnie nasłuchuje tylko na `127.0.0.1`. Nie wystawiaj go publicznie bez uwierzytelniania
  (przycisk „Analizuj AI” zużywa kredyty LLM).
- Treść ofert jest traktowana jako niezaufana: HTML jest escapowany, linki ograniczone do `http(s)`,
  a prompt LLM instruuje model, by ignorował instrukcje z oferty; wyjście modelu jest walidowane.
- Szanuj regulaminy źródeł: rzadkie odpytywanie (np. `fetch` kilka razy dziennie) i linkowanie do oryginału.
- Sekrety tylko w zmiennych środowiskowych – `.env` jest w `.gitignore`.

## Rozwój

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
```

Plan nauki Rusta na tym projekcie i pomysły na rozwój do wersji SaaS: [`docs/LEARNING.md`](docs/LEARNING.md).
