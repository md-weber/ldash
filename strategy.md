
# Phase 3 Strategy: Startup Performance

## Problem

`App::refresh()` runs **5 + 3N** sequential hledger subprocess calls (N = number
of crypto holdings). With 6 coins that's 23 invocations, each re-parsing the
full journal from scratch. All blocking — no UI rendered until every call
finishes. Perceived startup: several seconds of blank screen.

### Call inventory (current)

| Call                          | Count | Depends on     |
|-------------------------------|-------|----------------|
| `load_price_history`          | 1     | (file read)    |
| `load_crypto_balances`        | 1     | —              |
| `compute_portfolio`           | 1     | prices, crypto |
| `load_coin_chart_series`      | 3×N   | portfolio      |
| `load_account_balances_eur`   | 1     | —              |
| `load_net_worth_history`      | 1     | —              |
| `load_monthly_data`           | 1     | —              |
| `load_last_year_monthly`      | 1     | —              |

Only `compute_portfolio` and `load_coin_chart_series` have data dependencies.
Everything else is independent and parallelisable.

---

## Step 1 — Progressive rendering (show UI immediately)

**Why:** Even if data takes 3s to load, showing the TUI frame with
"Loading…" in each panel feels instant. Users tolerate latency when they see
progress; they don't tolerate a frozen terminal.

**Files:** `src/main.rs`, `src/app.rs`

**Changes:**

- `App::new()` returns immediately with empty/default data and `loading: true`.
- Move `refresh()` call to after the first `terminal.draw()` in `run()`.
- UI already handles empty data (renders empty tables/charts). Add a centered
  "Loading data…" overlay when `app.loading` is true.

**Impact:** Perceived startup drops to ~50ms (time to first frame). Actual data
load time unchanged but user sees the app immediately.

**Scope:** ~15 lines.

---

## Step 2 — Parallel hledger calls (std::thread)

**Why:** The 5 independent hledger calls + the 3×N coin series calls run
sequentially. Each takes 100-300ms (journal parse + query). Running them
in parallel on separate threads cuts wall-clock time from sum to max.

**Files:** `src/data.rs`, `src/app.rs`

**Changes:**

### 2a — Parallel independent calls

- Use `std::thread::scope` to run these concurrently:
  - `load_crypto_balances`
  - `load_account_balances_eur`
  - `load_net_worth_history`
  - `load_monthly_data`
  - `load_last_year_monthly`
- Collect results, apply to `App` after all threads join.

### 2b — Parallel coin chart series

- After portfolio is computed, spawn one thread per coin for
  `load_coin_chart_series` (each coin does 3 hledger calls internally).
- Cap thread count with a semaphore if holdings > 8.

**Impact:** With 6 coins: 23 sequential calls → ~4 parallel groups.
Estimated 3-4× speedup (e.g. 4s → 1s).

**Scope:** ~40 lines.

---

## Step 3 — Background refresh with channel

**Why:** Steps 1+2 still block the event loop during refresh. Moving data
loading to a background thread lets the UI remain responsive (animations,
tab switching) while data loads.

**Files:** `src/main.rs`, `src/app.rs`

**Changes:**

- Spawn `refresh()` on `std::thread::spawn`, send results back via
  `std::sync::mpsc::channel`.
- Event loop checks channel each tick; applies new data when ready.
- Manual refresh (`r` key) and auto-refresh both use the channel.
- Guard against double-refresh with an `is_refreshing: bool` flag.

**Impact:** UI never freezes during refresh. Combined with Step 2, user
sees data appear ~1s after launch with zero perceived lag.

**Scope:** ~50 lines.

---

## Step 4 — Lazy tab loading

**Why:** Portfolio tab loads first (default tab), but account/monthly data
loads too even though it's not visible. Deferring non-visible tab data
until tab switch makes the first visible tab appear faster.

**Files:** `src/app.rs`

**Changes:**

- Track `tabs_loaded: [bool; 3]` per tab.
- On startup, only load Portfolio data.
- On first switch to Accounts/Monthly tab, trigger load for that tab's data.
- Show per-tab "Loading…" state while data arrives.

**Impact:** Initial data display ~2× faster (only load 1 tab worth of data).
Subsequent tab switches add ~200ms one-time cost.

**Scope:** ~30 lines.

---

## Step 5 — Coin chart series batching

**Why:** `load_coin_chart_series` runs 3 `hledger register` calls per coin,
each re-parsing the full journal. Combining into one `register assets:crypto
--cost -O csv` call for all coins at once eliminates N×3 → 3 calls total.

**Files:** `src/data.rs`

**Changes:**

- Add `load_all_coin_chart_series(journal_path, price_history, coins)` that:
  - Runs 3 bulk register queries (all coins at once).
  - Splits results by commodity in-memory.
  - Builds `CoinChartSeries` per coin from the split data.
- Replace the per-coin loop in `refresh()`.

**Impact:** 18 hledger calls → 3. Combined with Step 2, total calls drop
from 23 to 8, all parallel. Estimated 5-8× total speedup.

**Scope:** ~60 lines (refactor + new parser logic).

---

## Step 6 — Journal mtime cache

**Why:** Auto-refresh every 5 min re-runs everything even if journal didn't
change. Checking file mtime before refresh avoids redundant work.

**Files:** `src/app.rs`

**Changes:**

- Store `last_journal_mtime: SystemTime` in `App`.
- Before auto-refresh, check `fs::metadata(journal_path).modified()`.
- Skip refresh if mtime unchanged. Still allow manual `r` to force reload.

**Impact:** Zero-cost auto-refresh when journal unchanged. Saves 1-4s every
5 minutes for users who leave the dashboard open.

**Scope:** ~10 lines.

---

## Implementation Order Summary

| Step | Area                | Risk  | Effort | Impact      |
|------|---------------------|-------|--------|-------------|
| 1    | Progressive render  | None  | Tiny   | High (UX)   |
| 2    | Parallel calls      | Low   | Medium | High (3-4×) |
| 3    | Background refresh  | Low   | Medium | High (UX)   |
| 4    | Lazy tab loading    | None  | Small  | Medium      |
| 5    | Batch coin queries  | Med   | Medium | High (5-8×) |
| 6    | Mtime cache         | None  | Tiny   | Medium      |

Steps 1-2 give biggest bang: instant UI + parallel loading.
Step 3 makes refresh non-blocking.
Steps 4-6 are incremental optimizations, order flexible.
