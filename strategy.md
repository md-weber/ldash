# Ledger Dashboard — Phase 2 Strategy: Trends & Drill-Down

The Phase 1 strategy (11 steps) is complete. The dashboard now shows solid
point-in-time snapshots across three tabs. Phase 2 focuses on **temporal
visibility** (how things change over time) and **interactive drill-down**
(exploring data without leaving the TUI).

Ordered from highest-impact / lowest-risk to nice-to-haves.
Each step is self-contained: the app compiles and works after every step.

---

## Step 1 — Net worth history chart (Accounts tab) ✅

**Why:** Net worth is the single most important number. Seeing it as a
time series reveals whether you're on track. Currently it's a single number
with no context — up or down this month? This year? No way to tell.

**Files:** `src/data.rs`, `src/app.rs`, `src/ui.rs`

**Changes:**

### 1a — Data layer

- Add `pub struct NetWorthSeries { pub points: Vec<(f64, f64)>, pub labels: Vec<(NaiveDate, f64)> }`.
- Add `load_net_worth_history(journal_path)` that runs:
  `hledger -f FILE balance assets -H -p "monthly from 2024" -O csv --layout bare`
  and sums all EUR-valued rows per month to produce a time series.
- Store in `App` as `pub net_worth_history: NetWorthSeries`.

### 1b — UI

- Split the Accounts tab vertically: top 40% = net worth chart, bottom 60% = existing table.
- Render a `Chart` with a single `Dataset` (line, braille markers, gold color).
- X-axis: month labels (`"Jan 25"`, `"Apr 26"`). Y-axis: EUR values.
- Keep the existing net worth number in a `Paragraph` between chart and table.

**Scope:** ~70 lines across 3 files.

---

## Step 2 — Monthly income vs expenses bar chart

**Why:** The Monthly tab shows numbers for one month at a time. An overview
chart comparing income/expenses across all months reveals seasonal patterns
and spending trajectory at a glance.

**Files:** `src/ui.rs`

**Changes:**

- Add `render_monthly_chart(f, app, area)` that builds a `BarChart` widget
  using `app.monthly.months` data.
- Each month gets two bars: income (green) and expenses (red).
- Highlight the currently selected month with a brighter shade.
- Split the Monthly tab: top = bar chart (height 12), bottom = existing
  summary + detail tables.
- The chart uses data already loaded — no new hledger commands needed.

**Scope:** ~50 lines in `ui.rs`.

---

## Step 3 — Year-to-date summary panel

**Why:** "Am I ahead or behind for the year?" is answered by aggregating all
months up to now. Show total income, total expenses, average savings rate,
and best/worst month — all derivable from existing `MonthlyData`.

**Files:** `src/ui.rs`, `src/app.rs`

**Changes:**

- Add `App::ytd_stats() -> YtdStats` that iterates `monthly.months` up to
  current month and computes: total_income, total_expenses, avg_savings_rate,
  best_month (highest net), worst_month (lowest net).
- Render as a compact `Paragraph` or small table in the Monthly tab summary
  area, next to or below the savings rate gauge.

**Scope:** ~40 lines.

---

## Step 4 — Account drill-down (recent transactions)

**Why:** When you see a suspicious balance, you want to know the last few
transactions without leaving the dashboard. Currently requires switching to
a terminal and running hledger manually.

**Files:** `src/data.rs`, `src/app.rs`, `src/main.rs`, `src/ui.rs`

**Changes:**

### 4a — Data layer

- Add `load_recent_transactions(journal_path, account, n) -> Vec<Transaction>`
  where `Transaction = { date, description, amount, running_total }`.
- Runs `hledger register ACCOUNT -O csv --count N`.

### 4b — App state

- Add `pub account_detail: Option<Vec<Transaction>>` and
  `pub detail_account_name: Option<String>` to `App`.
- On `Enter` key in Accounts tab → load transactions for selected account,
  store in `account_detail`.
- On `Esc` in detail view → clear `account_detail` (go back to list).

### 4c — UI

- When `account_detail.is_some()`, replace the accounts table with a
  transaction list table showing date, description, amount, running balance.
- Show account name in block title. Add "Esc to go back" hint.

**Scope:** ~80 lines across 4 files. Largest step in Phase 2.

---

## Step 5 — Expense category colors

**Why:** The expenses table is a wall of white text with red numbers. Distinct
colors per top-level category (housing, food, transport, etc.) makes scanning
faster. Also makes the bar chart from Step 2 more readable.

**Files:** `src/ui.rs`

**Changes:**

- Define a palette of 8-10 distinct colors mapped to common expense prefixes:
  `expenses:housing` → blue, `expenses:food` → yellow, `expenses:transport` →
  magenta, etc. Fallback to default for unknown categories.
- Apply in `render_monthly_expenses()` for the category name cell and bar.
- Same palette used in the bar chart (Step 2) if already implemented.

**Scope:** ~25 lines.

---

## Step 6 — Auto-refresh timer

**Why:** Leaving the dashboard open while editing the journal is a common
workflow. Currently requires pressing `r` manually. A background refresh
every 5 minutes keeps data current.

**Files:** `src/main.rs`, `src/app.rs`

**Changes:**

- Add `last_refresh: Instant` to `App`.
- In the event loop, after the tick check, add:
  ```
  if app.last_refresh.elapsed() >= Duration::from_secs(300) {
      app.refresh()?;
  }
  ```
- Update `last_refresh` in `refresh()`.
- Show "auto-refreshed at HH:MM:SS" in status bar to distinguish from manual.

**Scope:** ~10 lines.

---

## Step 7 — Portfolio total P/L summary

**Why:** The table shows per-coin P/L but never the total. "Am I up or down
overall on crypto?" requires mental arithmetic across 6 coins. One summary
line fixes this.

**Files:** `src/app.rs`, `src/ui.rs`

**Changes:**

- Add `App::total_portfolio_pl() -> (f64, f64)` returning (absolute EUR P/L,
  percentage P/L) by summing across all holdings and their chart series.
- Render in the holdings table total row, filling the currently-empty P/L
  columns with the aggregate values, styled green/red.

**Scope:** ~20 lines.

---

## Step 8 — Scrollable accounts table (proper TableState) ✅

**Why:** The current account_scroll is manual offset with `.skip()`. This
means no visual highlight of which row is selected, and no scroll indicator.
Using ratatui's `TableState` gives proper selection highlight and prepares
for the drill-down in Step 4.

**Files:** `src/app.rs`, `src/ui.rs`

**Changes:**

- Replace `pub account_scroll: usize` with `pub account_state: TableState`.
- In `render_accounts()`, use `f.render_stateful_widget(table, area, &mut state)`
  and set `.highlight_style()` on the table.
- Update `scroll_up/scroll_down` for Accounts tab to use `state.select()`.
- Same treatment for `expense_scroll` → `expense_state: TableState`.

**Scope:** ~30 lines changed.

---

## Step 9 — Configurable date range for net worth chart ✅

**Why:** Step 1 hardcodes `"from 2024"`. Users with longer histories want to
see more. Users who started recently don't want empty space. Let `←`/`→`
keys on the Accounts tab zoom the net worth chart.

**Files:** `src/app.rs`, `src/main.rs`, `src/data.rs`

**Changes:**

- Add `pub nw_range: NetWorthRange` enum: `Year1`, `Year2`, `Year5`, `All`.
- `←`/`→` on Accounts tab cycles through ranges.
- `load_net_worth_history` takes the range and adjusts the `-p` argument.
- Reload only the net worth series on range change (cheap operation).

**Scope:** ~25 lines.

---

## Step 10 — Monthly comparison (same month last year)

**Why:** "Is my spending this March higher than last March?" Seasonal
comparison is one of the most useful insights. Requires loading last year's
monthly data alongside this year's.

**Files:** `src/data.rs`, `src/app.rs`, `src/ui.rs`

**Changes:**

- In `load_monthly_data`, also run `hledger incomestatement -p "monthly last year"`.
- Store as `pub last_year: MonthlyData` in `App`.
- In the monthly summary, if a matching month exists in last year's data,
  show a "vs last year" line: `"Mar '25: 2340€  Mar '26: 1890€  ↓19%"`.

**Scope:** ~40 lines.

---

## Implementation Order Summary

| Step | Area             | Risk  | Effort | Value    |
|------|------------------|-------|--------|----------|
| 1    | Net worth chart  | Low   | Medium | Critical |
| 2    | Monthly bar chart| None  | Medium | High     |
| 3    | YTD summary      | None  | Small  | High     |
| 4    | Account drill    | Low   | Medium | High     |
| 5    | Category colors  | None  | Tiny   | Medium   |
| 6    | Auto-refresh     | None  | Tiny   | Medium   |
| 7    | Total P/L        | None  | Tiny   | High     |
| 8    | TableState       | None  | Small  | Medium   |
| 9    | NW date range    | None  | Small  | Low      |
| 10   | Year comparison  | Low   | Medium | Medium   |

Steps 1–3 form the "trends" core — do them first.
Step 4 is the "drill-down" centerpiece.
Steps 5–8 are independent polish.
Steps 9–10 are nice-to-haves that build on earlier steps.
