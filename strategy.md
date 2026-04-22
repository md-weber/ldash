# ldash v1.0 — Release Roadmap

## What We Have

Three-tab TUI with crypto portfolio (FIFO P/L, stacked/unstacked charts, allocation bars),
accounts (net worth chart, liabilities, savings goals, drill-down), and monthly
(income/expenses, budgets, YoY, YTD sparklines). Plus: search, file watcher,
hot-reload config, clipboard export, price alerts, lazy loading, background refresh.

Solid foundation. Below = what's missing before shipping 1.0.

---

## Should Have (P1)

### File export

- `Y` copies to clipboard — add `e` to export current view as CSV/JSON file
- Configurable export directory

### Account filtering

- `/` search exists globally but no filter-as-you-type within Accounts tab
- Type to filter account list by name prefix

---

## Nice to Have (P2)

### Recurring transaction detection

- Identify subscriptions/recurring expenses automatically
- Show in Monthly tab: "3 recurring: Netflix, Spotify, Gym = 45€/mo"

### Cash flow forecast

- Based on recurring income/expenses, project next 3-6 months
- Simple line chart on Monthly or Accounts tab

### Custom date ranges

- Monthly tab locked to calendar years
- Allow arbitrary ranges: "2025-06 to 2026-03"

### Net worth breakdown chart

- Current: single net worth line
- Add: stacked area chart showing asset composition over time (bank, crypto, investments)

### Multi-journal support

- Some users split journals per year or per entity
- `journals = ["/path/a.journal", "/path/b.journal"]` in config

### Notification/webhook on budget exceed

- Desktop notification or webhook when a budget crosses threshold
- Useful for users who leave ldash running in tmux

---

## Release Engineering (P0)

### Packaging

- [ ] Publish to crates.io (`cargo publish`)
- [ ] Homebrew formula
- [ ] AUR package
- [ ] Nix flake
- [ ] GitHub Releases with pre-built binaries (Linux x86_64, macOS aarch64/x86_64)

### CI

- [ ] GitHub Actions: `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check`
- [ ] Release workflow: tag → build matrix → upload artifacts

### Documentation

- [ ] Man page (`ldash.1`)
- [ ] CHANGELOG.md
- [ ] Real screenshots in README (not placeholder paths)
- [ ] Example journal for new users to try

---

## Priority Order

1. CLI polish + version flag — table stakes for any release
2. Multi-currency rendering — broken promise in current config
3. Error resilience — crashes kill trust
4. Tests — gate for all future changes
5. Non-crypto fallback — most hledger users don't have crypto
6. CI + packaging — can't release without distribution
7. Mouse + PgUp/PgDn — quick UX wins
8. Income drill-down — feature symmetry
9. Theming — personalization drives adoption
10. Everything else
