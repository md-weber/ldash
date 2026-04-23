# ldash v1.0 — Release Roadmap

## What We Have

Three-tab TUI with crypto portfolio (FIFO P/L, stacked/unstacked charts, allocation bars),
accounts (net worth chart, liabilities, savings goals, drill-down), and monthly
(income/expenses, budgets, YoY, YTD sparklines). Plus: search, file watcher,
hot-reload config, clipboard export, price alerts, lazy loading, background refresh.

Solid foundation. Below = what's missing before shipping 1.0
---

## Nice to Have (P2)

### Recurring transaction detection

- Identify subscriptions/recurring expenses automatically
- Show in Monthly tab: "3 recurring: Netflix, Spotify, Gym = 45€/mo"

### Cash flow forecast

- Based on recurring income/expenses, project next 3-6 months or current year (monthly tab shows always all 12 months)
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
- Works already when they link the all.journal (where all journals and more information is combined)

### Notification/webhook on budget exceed

- Desktop notification or webhook when a budget crosses threshold
- Useful for users who leave ldash running in tmux

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

---

## Release Recommendation

Ship **v1.0.0 without the P2 nice-to-haves** once the blockers above are
fixed. Reasoning:

- The feature set already covers the original scope and then some
  (price alerts, exports, themes, hot-reload were not in the initial 1.0
  brief)
- SemVer makes 1.1, 1.2, 1.3 the natural home for the P2 features —
  each one is independent and big enough to anchor a minor release
- Shipping 1.0 first surfaces real-user feedback that should reshape
  P2 priorities (e.g. notifications design depends on how people
  actually run ldash in tmux)
- The blockers are a half-day of work; the P2 list is multi-week

Suggested 1.x cadence:

| Version | Headline feature |
|---------|------------------|
| 1.1 | Custom date ranges + multi-journal (low-risk plumbing) |
| 1.2 | Net worth breakdown (stacked area chart) |
| 1.3 | Recurring detection + cash flow forecast (paired) |
| 1.4 | Notifications / webhooks |
