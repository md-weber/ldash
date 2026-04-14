# Ledger Dashboard — Improvement Strategy

Ordered from highest-impact / lowest-risk to nice-to-haves.
Each step is self-contained: the app compiles and works after every step.

---

## Step 1 — Panic hook (safety net) ✅

**Why:** If anything panics today, the terminal is left in raw mode (no echo,
no cursor). The user has to blindly type `reset`. This is the single most
important fix because it affects every subsequent step — any bug introduced
later won't brick the terminal.

**Files:** `src/main.rs`

**Changes:**

- Between `enable_raw_mode()` and `Terminal::new()`, install a custom panic
  hook that calls `disable_raw_mode()` + `LeaveAlternateScreen` before
  forwarding to the original hook.
- Keep the existing cleanup after `run()` as-is (it handles the normal exit
  path).

**Scope:** ~10 lines added.

---

## Step 2 — hledger availability check ✅

**Why:** If `hledger` is not in `$PATH`, the app currently starts, enters raw
mode, and then every `Command::new("hledger")` silently fails or returns an
opaque IO error. Better to fail fast with a clear message *before* entering
the alternate screen.

**Files:** `src/main.rs`

**Changes:**

- Add a function `check_hledger()` that runs `hledger --version` and returns
  `Result<()>`.
- Call it right after `find_journal()`, before `enable_raw_mode()`.
- On failure, bail with: `"hledger not found in PATH. Install it from https://hledger.org"`.

**Scope:** ~12 lines added.

---

## Step 3 — Portfolio allocation % column ✅

**Why:** Quick win. The holdings table already shows EUR value per coin but
not what fraction of the portfolio each coin represents. A `%` column makes
relative sizing visible at a glance.

**Files:** `src/ui.rs` → `render_holdings_table()`

**Changes:**

- Compute `total` (already done on line 99).
- For each holding row, compute `pct = h.value_eur / total * 100.0`.
- Add a 5th cell: `format!("{:.1}%", pct)` styled in `FG`.
- Add a 5th width constraint: `Constraint::Length(7)`.
- Add `"Alloc"` to the header row.

**Scope:** ~8 lines changed.

---

## Step 4 — Net worth total on the Accounts tab ✅

**Why:** The Accounts tab lists every asset balance but never shows the sum.
Net worth is the single most important number in a personal finance dashboard.

**Files:** `src/app.rs`, `src/ui.rs`

**Changes:**

- In `App`, add a helper: `pub fn total_net_worth(&self) -> f64` that sums
  `account_balances.iter().map(|b| b.amount)`.
- In `render_accounts()`, add a summary row at the top (or a `Paragraph`
  above the table) showing `"Net Worth: {:.2} €"` in bold gold.

**Scope:** ~15 lines.

---

## Step 5 — Month navigation on the Monthly tab ✅

**Why:** Currently locked to the current calendar month. All the multi-month
data is already loaded from hledger (`-p "monthly this year"`), but
`parse_monthly_csv` discards every column except `current_month`. Enabling
left/right navigation is high value.

**Files:** `src/data.rs`, `src/app.rs`, `src/main.rs`, `src/ui.rs`

**Changes:**

### 5a — Store all months in data layer

- Change `MonthlyData` to hold a `Vec<SingleMonth>` where each `SingleMonth`
  has `month_name`, `income`, `expenses`, `total_income`, `total_expenses`.
- Rewrite `parse_monthly_csv` to iterate over all columns (1..=12) and build
  a `SingleMonth` for each that has non-zero data.
- Keep the existing `MonthlyData` struct as a wrapper:
  `pub months: Vec<SingleMonth>, pub selected: usize`.

### 5b — Navigation in App

- Add `selected_month: usize` to `App`.
- Add methods `pub fn month_left(&mut self)` and `pub fn month_right(&mut self)`
  that decrement / increment `selected_month` clamped to `0..months.len()`.
- Wire `KeyCode::Left / 'h'` and `KeyCode::Right / 'l'` in the `Monthly` tab
  arm of `scroll_up` / `scroll_down` (or as separate matches in `main.rs`).

### 5c — UI

- `render_monthly_summary` reads `app.monthly.months[app.selected_month]`
  instead of the single `app.monthly`.
- Show `"◀ March 2026 ▶"` in the title so the user sees navigation is
  available.

**Scope:** ~60–80 lines changed across 4 files. This is the largest step.

---

## Step 6 — Per-coin P/L column in portfolio table ✅

**Why:** The chart shows invested vs. current value, but the table should give
an at-a-glance gain/loss for each coin without having to select it.

**Files:** `src/data.rs`, `src/ui.rs`

**Changes:**

- In `CoinChartSeries`, add a convenience method or store `total_invested`
  (the last value of the `investment` series).
- In `render_holdings_table()`, look up the coin's `CoinChartSeries` from
  `app.coin_chart_cache` and compute:
  `pl = h.value_eur - series.total_invested`.
  `pl_pct = pl / series.total_invested * 100.0`.
- Add a 6th cell: `"+12.3%"` colored green/red.
- Update header and widths.

**Scope:** ~15 lines.

---

## Step 7 — Help popup (`?` key) ✅

**Why:** Standard TUI convention. The status bar keybindings get truncated on
narrow terminals.

**Files:** `src/app.rs`, `src/main.rs`, `src/ui.rs`

**Changes:**

- Add `pub show_help: bool` to `App`.
- In `main.rs`, add `KeyCode::Char('?')` → toggle `app.show_help`.
  When `show_help` is true, all other keys except `?` / `q` / `Esc` are
  ignored.
- In `ui.rs`, add `render_help_popup(f, area)` that renders a centered
  `Clear` + `Paragraph` overlay listing all keybindings.
- Call it at the end of `render()` when `app.show_help` is true.

**Scope:** ~40 lines.

---

## Step 8 — Dynamic coin ordering (sort by value) ✅

**Why:** The hardcoded `["SOL", "BTC", "ETH", "LINK", "TON", "AR"]` list in
`compute_portfolio` means new coins appear at the bottom regardless of value,
and removed coins silently disappear from the preferred order.

**Files:** `src/data.rs` → `compute_portfolio()`

**Changes:**

- Remove the `order` array.
- Collect all coins into a `Vec<CryptoHolding>`, then sort by `value_eur`
  descending.
- This is a 3-line change that replaces ~15 lines.

**Scope:** Net reduction of ~10 lines.

---

## Step 9 — Chart date labels with year ✅

**Why:** Price history can span multiple years. The current `%d.%m` format
makes Jan 2025 and Jan 2026 indistinguishable.

**Files:** `src/ui.rs` → `render_price_chart()`

**Changes:**

- Change the date format in `x_labels` from `"%d.%m"` to `"%b %y"`
  (e.g. "Dec 25", "Mar 26").
- Reduce label count from 5 to 3-4 if the chart area is narrow
  (check `area.width`).

**Scope:** ~5 lines changed.

---

## Step 10 — Handle terminal resize events

**Why:** On resize, the display may lag by up to 250ms (the tick interval).
Handling `Event::Resize` forces an immediate redraw.

**Files:** `src/main.rs`

**Changes:**

- In the event loop, add a match arm:
  `Event::Resize(_, _) => { /* just let the loop redraw */ }`
- This ensures we don't accidentally fall through or miss the event.

**Scope:** 2 lines.

---

## Step 11 — Use the `csv` crate for parsing

**Why:** The hand-rolled `parse_csv_line` doesn't handle escaped quotes
(`""`) inside fields. If hledger ever produces a description containing
`","` the parser will mispatch columns. The `csv` crate handles all edge
cases and is ~30 KB.

**Files:** `Cargo.toml`, `src/data.rs`

**Changes:**

- Add `csv = "1"` to `[dependencies]`.
- Replace `parse_csv_line` usages in `run_register`, `parse_balance_csv`,
  and `parse_monthly_csv` with `csv::ReaderBuilder` over the stdout bytes.
- Remove the `parse_csv_line` function.

**Scope:** ~40 lines changed, net reduction.

---

## Implementation Order Summary

| Step | Area        | Risk  | Effort | Value  |
|------|-------------|-------|--------|--------|
| 1    | Panic hook  | None  | Tiny   | Critical |
| 2    | hledger chk | None  | Tiny   | High   |
| 3    | Alloc %     | None  | Tiny   | Medium |
| 4    | Net worth   | None  | Small  | High   |
| 5    | Month nav   | Low   | Medium | High   |
| 6    | P/L column  | None  | Small  | High   |
| 7    | Help popup  | None  | Small  | Medium |
| 8    | Coin sort   | None  | Tiny   | Medium |
| 9    | Chart dates | None  | Tiny   | Low    |
| 10   | Resize      | None  | Tiny   | Low    |
| 11   | csv crate   | Low   | Medium | Medium |

Steps 1–4 can each be done in under 5 minutes.
Step 5 is the biggest change and should be done carefully.
Steps 6–11 are independent and can be done in any order.
