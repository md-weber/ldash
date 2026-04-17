# Ledger Dashboard — Fix Strategy

## 1. P/L Calculation: Harden Timelines

### Problem
P/L numbers shift unpredictably across range changes. Root causes:

- `holding_pl()` in `ui.rs:434-455` uses `portfolio_range_min_x()` to get the start boundary, but the **FIFO basis timeline** (`basis_timeline` in `data.rs:768-816`) only snapshots on transaction dates. Between transactions, `series_interp()` returns the *previous* snapshot — so for ranges starting between two buy events, basis and value can be misaligned (basis is stale but price has moved).
- For "All" range: `pl = value - invested`. For sub-ranges (3M/6M/YTD): `pl = current_pl - pl_at_start`. The `pl_at_start` uses `series_value_at_day()` which sums investment + price_growth + staking_growth — but `price_growth` is `bought_coin * price - cost`, meaning it already includes basis. Double-subtracting basis when `value_at_start - basis_at_start` is computed from these series can produce wrong numbers when coin amounts changed within the range.
- The total P/L row sums per-coin P/L but uses summed bases for the percentage. If one coin has zero basis (staking-only), the denominator is wrong.

### Fix Plan

**A. Align sample points with basis snapshots**
In `load_all_coin_chart_series()` (`data.rs`), when sampling every `step` days:
- Also force a sample on every basis snapshot date (i.e., every transaction date).
- This ensures `series_interp()` returns exact values at transaction boundaries.

**B. Keep current sub-range formula, fix interpolation alignment**
The current formula in `holding_pl()` (`ui.rs:434-455`) is actually correct in principle:
```
pl_range = (value_now - basis_now) - (value_at_start - basis_at_start)
         = change in unrealized P/L over the period
```
This properly handles buys (new basis cancels new value in the delta).

**DO NOT** change to `value_now - value_at_start` — that would conflate capital flows with gains (a buy would look like profit, a sell like a loss).

The real fix: ensure `series_interp()` returns accurate values at arbitrary points by forcing sample alignment (see fix A above). The interpolation misalignment is what causes the P/L numbers to look wrong across range changes.

**B2. Realized gains from sells are invisible**
Sells remove coins from both value and basis — the realized gain disappears from the portfolio view entirely. It shows up as EUR in the Accounts tab bank balance, but the Portfolio tab gives no indication a profitable (or unprofitable) sell happened.

**Decision: Option 2 — Annotation only.** Don't change P/L math, but add a footnote in the holdings table: "P/L is unrealized only. Sells reflected in Accounts tab."

**C. Handle zero-basis coins in total row**
In `render_holdings_table()` (`ui.rs:599-621`):
- When summing total P/L, skip coins with zero basis from the percentage denominator.
- Or: use `value_at_start` as denominator for sub-ranges (already non-zero if coin existed).

**D. Add sanity checks**
- If `invested == 0` and `value > 0` → show "∞" or "N/A" for P/L % (already partially done, but verify all paths).
- Clamp extreme percentages (>10000%) to avoid UI overflow.

---

## 2. Portfolio Chart: BTC Clipping & Price Gain Readability

### Problem
- Chart Y-axis range computed from `y_min_raw`/`y_max_raw` in `render_price_chart()` (`ui.rs:807-813`) uses `f64::min` with seed `0.0`. If all three series (investment, price_growth, staking) are positive, `y_min` stays at 0 — BUT `nice_y_axis` can set `lo` *below* 0 via floor rounding, wasting chart space. Meanwhile `y_max` may not account for the **sum** of the series at any point — individual max values don't reflect stacked visual height if user expects stacked interpretation.
- BTC staking line gets clipped because staking values are small compared to investment/price_growth scale, and the Y bounds are driven by the larger series. Staking line hugs the bottom and falls below visible area.
- Price gain line is independent of invested line — it shows `bought_coin * price - cost` which oscillates around zero. Hard to visually compare against invested because they're on completely different Y positions.

### Fix Plan

**A. Fix Y-axis bounds to include all data**
In `render_price_chart()` (`ui.rs:807-813`):
- Compute `y_min_raw` and `y_max_raw` using `f64::INFINITY` / `f64::NEG_INFINITY` as seeds instead of `0.0`:
  ```rust
  let y_min_raw = all_y.clone().fold(f64::INFINITY, f64::min);
  let y_max_raw = all_y.fold(f64::NEG_INFINITY, f64::max);
  ```
- This ensures negative price_growth values expand the bottom bound, and small staking values aren't clipped.

**B. Change price gain to show total value (invested + gain)**
Instead of plotting raw `price_growth = bought_coin * price - cost` (which hovers around zero), plot `invested + price_growth` = total market value of purchased coins. This makes the price gain line visually move **up or down from the invested line**, showing what the purchased portion is worth vs what was paid.

In `render_price_chart()` (`ui.rs:800-802`):
- Build `filtered_value` where each point is `(day, invested_at_day + price_growth_at_day)`.
- Rename dataset from "Price gain" to "Market value" or "Current value".
- Keep invested line as-is (gold). The gap between the two lines = unrealized P/L, visually obvious.

Alternatively (simpler, preserves 3-line setup):
- In `data.rs` `sample_day` closure (~line 866-894): change `price_growth` to emit `(days, bought_coin * price)` instead of `(days, bought_coin * price - cost)`. Label it "Value" in the chart. The visual delta from the invested line = P/L.

**C. Ensure staking line is visible**
- If staking values are very small relative to investment, consider plotting staking as a **separate Y-axis** or as an **additive line on top of investment**: `invested + staking_value`. This stacks it visually.
- Simpler approach: plot staking as `invested + price_growth + staking_growth` (total portfolio value including staking). Then the three lines become:
  1. **Invested** (cost basis) — gold
  2. **Purchased value** (market value of bought coins) — cyan  
  3. **Total value** (including staking rewards) — green
- Gap between 1→2 = price P/L. Gap between 2→3 = staking value. All three share the same Y scale and nothing clips.

---

## 3. Accounts Tab: Net Worth Chart Timeline Issues

### Problem
- Range cycling order is `1Y → 2Y → 5Y → All` (`NetWorthRange` in `app.rs:166-210`). Default is `Year2`. User expects default `All`, then cycle to `1Y → 2Y → 5Y`.
- "1Y" uses hledger period `"monthly from 1 year ago"` which starts from the first journal entry + 1 year, not from today minus 1 year. This is because hledger's `from X ago` is relative to the report start, not end.
- No YTD option (current calendar year).
- X-axis labels look odd — dates jump inconsistently because label indices are evenly spaced across the *data points* array, but data points may not be evenly spaced in time (months with no data get skipped by hledger).

### Fix Plan

**A. Add YTD range, reorder cycle**

In `app.rs`, change `NetWorthRange`:
```rust
enum NetWorthRange {
    All,    // default
    Ytd,
    Year1,
    Year2,
    Year5,
}
```

Cycle order: `All → 1Y → 2Y → 5Y → All` (left/right arrows). Add YTD between All and 1Y:
`All ←→ YTD ←→ 1Y ←→ 2Y ←→ 5Y`

**B. Fix period_arg to use absolute dates instead of relative**

Instead of `"monthly from 1 year ago"`, compute the actual date:
```rust
fn period_arg(self) -> String {
    let today = Local::now().date_naive();
    match self {
        Self::Ytd => {
            let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
            format!("monthly from {}", jan1.format("%Y-%m-%d"))
        }
        Self::Year1 => {
            let start = today - chrono::Duration::days(365);
            format!("monthly from {}", start.format("%Y-%m-%d"))
        }
        Self::Year2 => {
            let start = today - chrono::Duration::days(730);
            format!("monthly from {}", start.format("%Y-%m-%d"))
        }
        Self::Year5 => {
            let start = today - chrono::Duration::days(1825);
            format!("monthly from {}", start.format("%Y-%m-%d"))
        }
        Self::All => "monthly".to_string(),
    }
}
```

This ensures "1Y" means "from 365 days ago until now", not "from first entry + 1 year".

**C. Change default to All**

In `App::new()` (`app.rs:353`):
```rust
nw_range: NetWorthRange::All,
```

**D. Fix `period_arg` return type**

Currently returns `&'static str`. Change to `String` to support dynamic date computation. Update all callers (`load_all_data`, `start_refresh_tabs`, `reload_net_worth`).

**E. Improve X-axis label spacing**

In `render_net_worth_chart()` (`ui.rs:901-907`): labels are evenly sampled from `series.labels` by index. Since the underlying data is monthly from hledger, spacing should be consistent *if* hledger returns all months. Verify hledger `--empty` flag is set to include zero-balance months. If not, add `--empty` to the `load_net_worth_history` hledger call (`data.rs:379-397`):
```
"--empty"
```
This fills in months with no changes, giving uniform X spacing.

---

## Implementation Order

1. **3B + 3C + 3D** — Fix net worth ranges (quick, most user-visible pain)
2. **3A** — Add YTD range
3. **2A** — Fix Y-axis bounds (one-line fix)
4. **2B** — Change price gain to value-relative line
5. **2C** — Stack staking on top
6. **1A + 1B** — Align sample points with basis snapshots (fixes interpolation-caused P/L drift)
7. **1B2** — Decide on realized gains visibility (option 1/2/3)
8. **1C + 1D** — Edge cases and sanity checks
9. **3E** — Add `--empty` for uniform X spacing
