# ldash Bug + Refactor Strategy

Date: 2026-04-23

## Quick audit scope

- Ran `cargo test` (all green).
- Ran ignored integration tests (`cargo test --test integration -- --ignored`, all green).
- Ran `cargo clippy --all-targets --all-features` (1 style warning).
- Reviewed data loading, parsing, refresh concurrency, UI state, export path.

## Priority fixes (bugs / correctness)

### P0 - Price parser breaks on extra whitespace in `prices.journal`

- Location: `src/data/prices.rs`
- Problem: parser uses `splitn(5, ' ')`. Multiple spaces/tabs create empty tokens and valid `P` lines can be skipped.
- Impact: missing price history -> zero/incorrect portfolio valuation and empty charts.
- Fix:
  - replace parser with `split_whitespace()` or tokenizer that tolerates mixed spacing.
  - add fixture-style unit tests for:
    - single spaces
    - multiple spaces
    - tabs
    - trailing comments

### P0 - Currency matching too strict across loaders

- Locations: `src/data/parse.rs`, `src/data/balances.rs`
- Problem: code compares commodity with exact `currency_symbol` (`==`). Inputs like `EUR` vs `€` (or `USD` vs `$`) can silently drop rows.
- Impact: empty/partial monthly and net-worth views for valid ledgers.
- Fix:
  - introduce normalized currency matcher (alias set per configured currency).
  - reuse matcher in monthly parser and net-worth/breakdown loaders.
  - add integration tests with mixed currency spellings.

### P0 - Export CSV is not CSV-safe

- Location: `src/app/export.rs`
- Problem: CSV rows are assembled with `format!("{},...")`; account/category strings containing comma/quote/newline break output.
- Impact: malformed export files and incorrect downstream import.
- Fix:
  - write CSV with `csv::Writer` (proper escaping).
  - add tests for names containing comma, quote, and newline.

## Priority robustness improvements

### P1 - Silent failure on unreadable `prices.journal`

- Location: `src/data/prices.rs`
- Problem: `read_to_string(...).unwrap_or_default()` hides IO errors.
- Impact: app shows no crypto prices without clear error reason.
- Fix:
  - return `Result<Vec<PriceEntry>>` from `load_price_history`.
  - surface message in status bar when file unreadable (permissions/path issue).

### P1 - Background channel disconnect can wedge loading state

- Locations: `src/app/refresh.rs` (`check_refresh`, `check_background`)
- Problem: only handles `try_recv().ok()`. `Disconnected` path is ignored, receiver fields stay set forever.
- Impact: stuck spinner / blocked future refresh after worker failure.
- Fix:
  - handle `TryRecvError::Disconnected` explicitly.
  - clear corresponding `*_rx` fields and set status message.
  - add unit tests simulating disconnected channels.

### P1 - Non-UTF8 journal paths fallback to wrong file

- Locations: many loaders use `journal_path.to_str().unwrap_or("all.journal")`
- Problem: non-UTF8 path loses original file path and silently switches target.
- Impact: incorrect data source on valid Unix paths.
- Fix:
  - pass `Path`/`OsStr` directly into command args (`Command::arg` accepts OsStr).
  - remove `"all.journal"` fallback.

## Refactors with clear payoff

### P2 - Avoid compiling app modules twice (lib + bin)

- Locations: `src/main.rs`, `src/lib.rs`
- Problem: `main.rs` declares full module tree again, so tests run twice (lib test + bin test).
- Impact: slower CI/dev feedback, larger compile surface.
- Fix:
  - move runtime entry to library function (`ldash::run_cli()`).
  - keep `main.rs` as thin wrapper calling library entrypoint.
  - keep tests in library only.

### P2 - Reduce render-time cloning in monthly detail view

- Location: `src/ui/monthly.rs`
- Problem: `income_detail` / `expense_detail` are cloned during render path.
- Impact: avoidable allocations per frame.
- Fix:
  - refactor borrow pattern to avoid cloning (`as_ref` + narrower mutable borrow scopes).

### P2 - Strengthen config validation

- Location: `src/config.rs`
- Problem: validation currently checks only `refresh_interval`.
- Impact: invalid values (`default_tab`, `export_format`, `chart_mode`, `number_format`) degrade behavior silently.
- Fix:
  - add semantic validation for constrained enums/strings.
  - return warning/errors with accepted values.

## Test strategy additions

- Add focused unit tests:
  - `prices` parser whitespace/comment handling.
  - currency alias matching logic.
  - CSV export escaping.
  - disconnected background channel handling.
- Add one integration test:
  - ledger fixtures with `EUR` but UI currency configured as `€` (and USD/$ variant).

## Suggested implementation order

1. P0 parser + currency + export fixes.
2. P1 error surfacing + channel disconnect handling.
3. P1 path/OsStr cleanup.
4. P2 compile-structure and render clone refactor.
5. Expand tests, then run full suite + clippy again.
