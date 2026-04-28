# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://codeberg.org/md-weber/ldash/compare/v1.1.0...HEAD
[1.1.0]: https://codeberg.org/md-weber/ldash/compare/v1.0.0...v1.1.0
[1.0.0]: https://codeberg.org/md-weber/ldash/compare/v0.1.0...v1.0.0
