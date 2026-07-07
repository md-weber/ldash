# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.4.0] - 2026-07-07

### Added

- **Automatic price fetching** — ldash now fetches spot prices from the
  [CoinGecko](https://www.coingecko.com) free API and appends them to
  `prices.journal` automatically. On startup it checks whether today's prices
  are already present; if not, a background thread fetches them immediately so
  you no longer need to run a separate script. Press `P` to trigger a manual
  fetch at any time.
- New `[price_fetch]` config section with `currency` (default `"eur"`) and a
  `[[price_fetch.tokens]]` array mapping commodity symbols (e.g. `BTC`) to
  CoinGecko coin IDs (e.g. `"bitcoin"`). The feature is opt-in: it is a no-op
  until at least one token is configured.
- `P` keybinding — fetch prices on demand from anywhere in the TUI; status bar
  shows progress and result. A full data refresh is triggered automatically once
  new prices are written.

## [1.3.1] - 2026-06-10

### Fixed

- Prefixed commodity symbols (`Eur 100`, `USD 50`) are now parsed correctly.
  `parse_amount_str` previously only handled suffix layout (`100 EUR`); it now
  falls back to prefix detection when the left-hand token is not a number,
  covering `Eur 100`, `Eur -100`, and `-Eur 100` forms emitted by hledger.
- Display format follows the journal's commodity notation: when hledger reports
  amounts in prefix style, ldash auto-detects this at startup and renders all
  amounts with the symbol before the number (e.g. `Eur 1.234,56` instead of
  `1.234,56 €`). The raw commodity name from the journal (e.g. `Eur`) is
  preserved. A new `currency_prefix` config key allows manual override.

### Added

- `examples/prefixed-commodity-comments.journal` — reference journal exercising
  prefix commodities (`Eur`, `USD`), multiple account types, three months of
  income/expense data, and inline comments.

## [1.3.0] - 2026-06-09

### Added

- Persistent journal switch: press `Ctrl-S` inside the `Ctrl-O` prompt to write
  the typed (or currently active) journal path to `~/.config/ldash/config.toml`
  as the new `journal = "..."` key. Existing comments and unrelated keys are
  preserved; the file is written atomically via a sibling temp file.
- Year-over-year diagram on the monthly tab comparing earnings and expenses across recent years
- Liability tracking screen with estimated time to repay based on current payment rates
- Payee screen accessible with `p` in the monthly tab showing top contributors to payments
- Global search with drill-down option to navigate directly to matching entries

### Fixed

- Removed version string from title bar

### Changed

- Refactored `monthly.rs` into per-section submodules for maintainability

## [1.2.0] - 2026-05-04

### Added

- Portfolio tab always visible; centered empty-state screen when holdings are absent (mirrors Monthly tab pattern)
- `Ctrl-O` opens an inline journal-switch prompt; validates path, reloads all tabs on confirm (session-only, config not written)
- Tab-completion in the journal-switch prompt: expands `~/`, completes to longest common prefix, appends `/` for directories
- `journals` config array for a persistent quick-switch list; `↑`/`↓` cycles entries in the prompt picker
- Session history tracks switched-from/switched-to journals (max 10 entries); tip shown when `config.journals` is empty

## [1.1.0] - 2026-04-28

### Added

- Cash flow forecast on monthly tab via `hledger --forecast`
- Recurring/subscription expense detection on monthly tab
- Stacked asset breakdown chart on accounts tab (toggle with `s`)
- Auto-currency detection and configurable account roots
- Monthly tab: jump-to-month navigation
- Support for non-standard / large journals (e.g. 1k+ transactions, 100+ accounts)

### Fixed

- Price normalization for multi-currency portfolios
- Alert threshold now configurable via config file

### Changed

- Refactored app, data, UI, and event modules into per-concern files for maintainability

## [1.0.0] - 2026-04-22

### Added

- Crypto portfolio tab with holdings table, allocation %, and P/L tracking
- Portfolio bar chart with stacked/unstacked modes (toggle with `s`)
- Net worth history chart with selectable time ranges (YTD / 1Y / 2Y / 5Y / All)
- Account balances tab with EUR valuations and visual bar indicators
- Account drill-down: select any account to view recent transactions
- Monthly income & expenses bar chart with per-category breakdown
- Savings rate gauge and year-over-year comparison on monthly tab
- Budget tracking: monthly limits per expense category with progress bars
- Savings goals: target amounts per account prefix shown on accounts tab
- Background refresh with parallel hledger calls (non-blocking)
- Auto-refresh on journal file change via `notify`
- Lazy tab loading: only fetches data for the active tab on first visit
- Config file at `~/.config/ldash/config.toml` with hot-reload
- Three built-in themes: `dark`, `light`, `solarized`
- Per-field color overrides in config
- EU/US number format toggle
- `--file` CLI flag and `LEDGER_FILE` env var support
- Forgejo CI workflow (lint, build, test)
- Forgejo release workflow with cross-compilation via `cargo-zigbuild`

[Unreleased]: https://codeberg.org/md-weber/ldash/compare/v1.4.0...HEAD
[1.4.0]: https://codeberg.org/md-weber/ldash/compare/v1.3.1...v1.4.0
[1.3.1]: https://codeberg.org/md-weber/ldash/compare/v1.3.0...v1.3.1
[1.3.0]: https://codeberg.org/md-weber/ldash/compare/v1.2.0...v1.3.0
[1.2.0]: https://codeberg.org/md-weber/ldash/compare/v1.1.0...v1.2.0
[1.1.0]: https://codeberg.org/md-weber/ldash/compare/v1.0.0...v1.1.0
[1.0.0]: https://codeberg.org/md-weber/ldash/compare/v0.1.0...v1.0.0
