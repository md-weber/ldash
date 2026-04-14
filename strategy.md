
# Phase 4 Strategy: Feature Expansion

## Current State

Dashboard has 3 tabs (Portfolio, Accounts, Monthly) with parallel data loading,
background refresh, lazy tab loading, and mtime-based cache. UI is responsive
with progressive rendering. Core data: crypto portfolio P/L, net worth chart,
monthly income/expenses with YoY comparison.

---

## Feature Group A — Deeper Drill-Downs

### A1. Expense Category Drill-Down

**Problem:** Monthly tab shows expense categories but no way to see individual
transactions within a category. Account tab has drill-down, expenses don't.

**Changes:**
- Enter on selected expense category → show transactions for that category
  in selected month (reuse `load_recent_transactions` with date filter).
- Add date-range arg to `load_recent_transactions` (pass `-p "YYYY-MM"`).
- Render with same `render_account_detail` layout, Esc to go back.

**Impact:** Users can answer "why was Food so high in March?" without leaving TUI.

**Scope:** ~40 lines. Low risk.

---

### A2. Transaction Search

**Problem:** No way to find a specific transaction across all accounts/dates.
Users must drop to terminal `hledger register` commands.

**Changes:**
- New keybinding `/` → opens search input bar (bottom of screen).
- Run `hledger register -O csv` with description filter (`desc:QUERY`).
- Show results in a temporary overlay table (date, account, description, amount).
- Esc closes search. Support regex patterns.

**Impact:** Eliminates most reasons to leave the TUI.

**Scope:** ~80 lines. Needs text input widget. Medium risk.

---

## Feature Group B — Financial Intelligence

### B1. Budget Tracking

**Problem:** No visibility into whether spending stays within planned limits.
Users track budgets manually or not at all.

**Changes:**
- Config file (`~/.config/ldash/budgets.toml`) with per-category monthly limits:
  ```toml
  [budgets]
  "expenses:Essen" = 400.0
  "expenses:Freizeit" = 200.0
  ```
- Monthly tab: add progress bar or percentage next to each expense category
  showing budget usage (e.g. `[████████░░] 82%`).
- Color: green < 80%, yellow 80-100%, red > 100%.
- Summary widget: "3 categories over budget this month".

**Impact:** Core personal finance feature. Makes dashboard actionable.

**Scope:** ~60 lines + config loading. Low risk.

---

### B2. Cash Flow Sparklines

**Problem:** Monthly chart shows bars but no trend direction at a glance.
Hard to see if expenses are trending up or down over time.

**Changes:**
- Add sparkline widgets (ratatui `Sparkline`) next to YTD summary showing
  6-month trailing trend for income, expenses, and net.
- Compute from existing `monthly.months` data — no new hledger calls.

**Impact:** Instant trend visibility. Zero performance cost.

**Scope:** ~25 lines. No risk.

---

### B3. Savings Goal Tracker

**Problem:** Users have financial goals (emergency fund, house down payment)
but no way to track progress in dashboard.

**Changes:**
- Config file entry:
  ```toml
  [[goals]]
  name = "Emergency Fund"
  target = 15000.0
  account = "assets:bank:savings"
  
  [[goals]]
  name = "House Fund"
  target = 50000.0
  account = "assets:bank:house"
  ```
- Display as gauge widgets in Accounts tab or dedicated section.
- Pull current balance from existing `account_balances` data.

**Impact:** Makes long-term financial planning visible.

**Scope:** ~40 lines. Low risk.

---

## Feature Group C — Portfolio Enhancements

### C1. Portfolio Allocation Chart

**Problem:** Allocation percentages shown as text in table. Hard to visualize
portfolio balance at a glance.

**Changes:**
- Add horizontal stacked bar or mini bar chart below holdings table showing
  allocation split by coin (colored segments).
- Use ratatui `BarChart` with single group, one bar per coin.
- Data already available — no new hledger calls.

**Impact:** Visual portfolio balance check. Zero cost.

**Scope:** ~30 lines. No risk.

---

### C2. Portfolio Time Range Selection

**Problem:** Portfolio chart always shows full history. No way to zoom into
last 3 months or 1 year.

**Changes:**
- Add range selector (same pattern as net worth: 3M/6M/1Y/All).
- Filter `coin_chart_cache` data by date range before rendering.
- Keybinding `[` and `]` or reuse `←`/`→` on Portfolio tab.

**Impact:** Better analysis of recent performance vs long-term.

**Scope:** ~20 lines. No risk.

---

### C3. Price Alerts Display

**Problem:** No visibility into significant price movements since last session.

**Changes:**
- On startup, compare current prices to 24h-ago prices from price history.
- Show notification-style banner: "BTC +5.2%, SOL -3.1% since yesterday".
- Auto-dismiss after 5 seconds or on any keypress.

**Impact:** Quick market awareness on dashboard open.

**Scope:** ~30 lines. Low risk.

---

## Feature Group D — Configuration & Polish

### D1. Config File Support

**Problem:** No configuration. Expense color mapping, account groupings, and
display preferences are hardcoded.

**Changes:**
- Load `~/.config/ldash/config.toml` on startup.
- Options: journal path, refresh interval, default tab, number format (EU/US),
  currency symbol, expense color overrides.
- Fallback to current hardcoded defaults when config missing.

**Impact:** Foundation for all config-dependent features (budgets, goals).
Must implement before B1/B3.

**Scope:** ~50 lines + toml dependency. Low risk.

---

### D2. Liability Tracking

**Problem:** Net worth chart shows assets only. Users with mortgages or debts
see inflated net worth. `hledger balance assets liabilities` gives true picture.

**Changes:**
- Extend `load_account_balances_eur` to also query `liabilities`.
- Show liabilities in Accounts tab below assets, colored red, separated.
- Net worth calculation: assets + liabilities (liabilities are negative in hledger).
- Optional: net worth chart includes liabilities in calculation.

**Impact:** Accurate financial picture for users with debt.

**Scope:** ~30 lines. Low risk.

---

### D3. Multi-Year Monthly Comparison

**Problem:** Only current year vs last year. No way to see 3-year expense trends
for a category.

**Changes:**
- New keybinding (e.g. `y`) in Monthly tab cycles through years: 2026, 2025, 2024.
- Or: overlay mode showing selected month across multiple years as grouped bars.
- Reuse `load_monthly_data` with different `-p` period argument.

**Impact:** Long-term trend analysis.

**Scope:** ~40 lines. Low risk.

---

### D4. Responsive Terminal Layout

**Problem:** UI assumes wide terminal. Narrow terminals (< 100 cols) break
table layouts and truncate data.

**Changes:**
- Detect terminal width in render functions.
- < 100 cols: stack Portfolio panels vertically (table above, chart below).
- < 80 cols: hide bar charts in tables, abbreviate column headers.
- Accounts table: truncate long account names with ellipsis.

**Impact:** Usable on laptop screens and split panes.

**Scope:** ~40 lines across `ui.rs`. Low risk.

---

## Feature Group E — Data Export & Integration

### E1. Clipboard Export

**Problem:** No way to extract data from TUI without going to terminal.

**Changes:**
- Keybinding `y` (yank) copies current view data to clipboard.
- Portfolio: CSV of holdings. Accounts: selected account balance.
  Monthly: current month income/expenses.
- Use `clipboard` crate or pipe to `pbcopy`/`xclip`.

**Impact:** Bridges TUI and other tools (spreadsheets, reports).

**Scope:** ~30 lines. Low risk.

---

### E2. Journal File Watcher

**Problem:** Auto-refresh checks mtime every 5 minutes. Edits to journal
not reflected for up to 5 min.

**Changes:**
- Use `notify` crate to watch journal file + included files for changes.
- Trigger refresh within 1 second of file save.
- Replace polling interval with event-driven refresh.

**Impact:** Near-realtime updates when editing journal in another pane.

**Scope:** ~40 lines + `notify` dependency. Low risk.

---

## Implementation Priority

| Priority | Feature | Group | Depends On | Effort | Impact |
|----------|---------|-------|------------|--------|--------|
| 1        | D1 Config file | Config | — | Medium | Foundation |
| 2        | A1 Expense drill-down | Drill-down | — | Small | High |
| 3        | B2 Cash flow sparklines | Intelligence | — | Tiny | Medium |
| 4        | D2 Liability tracking | Config | — | Small | High |
| 5        | B1 Budget tracking | Intelligence | D1 | Medium | High |
| 6        | C1 Allocation chart | Portfolio | — | Small | Medium |
| 7        | C2 Portfolio time range | Portfolio | — | Tiny | Medium |
| 8        | D4 Responsive layout | Polish | — | Medium | Medium |
| 9        | A2 Transaction search | Drill-down | — | Medium | High |
| 10       | B3 Savings goals | Intelligence | D1 | Small | Medium |
| 11       | E2 File watcher | Integration | — | Small | Medium |
| 12       | D3 Multi-year comparison | Config | — | Small | Medium |
| 13       | C3 Price alerts | Portfolio | — | Small | Low |
| 14       | E1 Clipboard export | Integration | — | Small | Low |

### Recommended batches for plan files:

- **Plan 1:** D1 + A1 (config foundation + first drill-down)
- **Plan 2:** B2 + C1 + C2 (visual enhancements, no new data loading)
- **Plan 3:** D2 + D4 (accuracy + usability)
- **Plan 4:** B1 + B3 (actionable financial intelligence, needs D1)
- **Plan 5:** A2 + E2 (search + live updates)
- **Plan 6:** D3 + C3 + E1 (nice-to-haves)
