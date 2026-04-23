# ldash — Codebase Improvement Strategy

A review of the current state (~10k LOC, post-Codex feature wave) with concrete refactor, cleanup, hardening and file-split proposals. Items are roughly grouped by impact and ordered to minimise churn.

---

## 1. Files Too Big — Split

### 1.1 `src/app/mod.rs` (2422 lines) — ✅ DONE

The `App` impl block has become a kitchen sink: state, refresh orchestration, mouse hit-testing, HTML/JSON export, recurring-expense detection, cash-flow forecast, fixtures and tests all share one file. Suggested split:

```
src/app/
  mod.rs          struct App + new() + small accessors (visible_tabs, crypto_enabled, total_*)
  refresh.rs      load_all_data, RefreshResult plumbing, apply_refresh, auto_refresh,
                  start_refresh*, check_refresh, check_config_reload
  scroll.rs       scroll_up/down, scroll_page_up/down, scroll_home/end, page_size_for
  mouse.rs        handle_mouse, on_left_click, on_table_click, on_monthly_chart_click,
                  select_range, rect_contains
  export.rs       render_html, render_json, json_str, export_to_file,
                  ensure_all_tabs_for_export, export_current_view, open/cancel_export_prompt
  insights.rs     cash_flow_forecast, recurring_expenses, ytd_stats,
                  budget_status, goal_progress, compute_price_alerts, check_alert_timeout
  detail.rs       open_*/close_* detail (account, expense, income),
                  filter (account_filter_*, close_account_filter),
                  search (open_/close_/execute_search, search_scroll_*)
  combined.rs     rebuild_combined_months, current_month, last_year_match,
                  current_month_is_forecast, month navigation, year cycle,
                  reload_monthly_year, reload_net_worth, range navigation
  fixtures.rs     #[cfg(test)] fixture_empty, fixture_with_accounts, fixture_with_monthly
  tests.rs        #[cfg(test)] unit tests
```

### 1.2 `src/data.rs` (1412 lines) — ✅ DONE

Mixes hledger CLI wrapping, CSV parsing, balance loading, monthly statements and the FIFO portfolio engine. Suggested split:

```
src/data/
  mod.rs          shared types (PriceEntry, AccountBalance, SingleMonth, MonthlyData,
                  Transaction, NetWorthSeries, NetWorthBreakdownSeries, CryptoHolding,
                  CoinChartSeries) + run_hledger
  parse.rs        parse_eu_number, parse_amount_str, parse_balance_csv, parse_monthly_csv,
                  month_name
  balances.rs     load_crypto_balances, load_account_balances_eur, load_liability_balances_eur,
                  load_net_worth_history, load_net_worth_breakdown, breakdown_category
  monthly.rs      load_monthly_data, load_monthly_for_period, load_monthly_with_forecast,
                  load_last_year_monthly
  transactions.rs load_recent_transactions, search_transactions
  portfolio.rs    compute_portfolio, load_all_coin_chart_series, FIFO Lot logic,
                  RegisterEntry, run_register_full
  prices.rs       load_price_history, latest_prices
```

### 1.3 `src/main.rs` event handler (lines ~290–465) ✅ DONE

The nested key-handling match is unmaintainable. Extract `src/events.rs`:

```rust
pub enum Action { Quit, Continue }
pub fn handle_key(app: &mut App, key: KeyEvent) -> Action { ... }
```

Sub-functions per modal mode (`handle_export_prompt_key`, `handle_search_key`, `handle_help_key`, `handle_filter_key`, `handle_normal_key`).

### 1.4 `src/ui/mod.rs` (687 lines) - ✅ DONE

Move overlay renderers (`render_search_overlay`, `render_help_popup`, `render_loading_overlay`, `render_price_alerts`, `render_status`) into `src/ui/overlays.rs`. Keep theme + entry point + tab/title/content dispatch in `mod.rs`.

---

## 2. Refactor (Behaviour-Preserving)

1. **`apply_refresh` boilerplate** — ✅ DONE. `apply_field!` / `apply_field_silent!` macros in `src/app/refresh.rs` collapse the 9 near-identical `match TabData<T>` arms.

2. **`load_all_data` 8-tuple thread join** — ✅ DONE. Replaced with `LoadHandles` struct + `spawn_all` + `into_tab_data` helper. Adding a new source = one field, one spawn line, one mapping.

3. **Group `App` fields** — ⚠️ PARTIAL. `geometry: Geometry` extracted (all `Rect`/`Vec<Rect>` fields written by the render pass). Remaining substructs (`monthly_view`, `accounts_view`, `portfolio_view`, `overlay`, `data`) deferred to a follow-up PR per the PR-ordering guidance below — touches every render call site, best done as its own dedicated change.

4. **Deduplicate `MONTH_NAMES`** — ✅ DONE. Single `pub(crate) const MONTH_NAMES: [&str; 12]` in `src/data/parse.rs` plus `month_index(name) -> Option<usize>`. `month_name`, `rebuild_combined_months`, `cash_flow_forecast` and `month_name_to_period` all consume the shared const.

5. **Test fixtures fragility** — ✅ DONE. `impl Default for App` lives in `src/app/mod.rs`; `fixture_empty` now uses `..Self::default()` with only the test-mode overrides spelled out.

6. **`Tab::index()` array indexing** — ✅ DONE. `[bool; 3]` replaced with `TabFlags { portfolio, accounts, monthly }` (named accessors + `get`/`set`/`any` helpers). `Tab::index()` removed.

7. **`chart_stacked: bool`** — ✅ DONE. Promoted to `enum ChartMode { Stacked, Unstacked }` with `from_config`, `toggle`, `label`, `is_stacked` helpers.

8. **Per-tab dispatch repetition** — ✅ DONE. `src/app/scroll.rs` collapsed via private `FocusedList` enum + `current_selected` / `current_len` / `set_current_selected` / `current_page_size` helpers. The 8+ near-identical `match self.tab` blocks are now one-line bodies.

9. **Style helper** — ✅ DONE. `fg!`, `bg!`, `fg_bg!` macros added in `src/ui/style_macros.rs` (with optional `, bold` suffix). Available crate-wide via `#[macro_export]`.

---

## 3. Hardening (Bugs & Risks) — ✅ DONE

1. **HTML export XSS** — ✅ DONE. `html_escape` helper in `src/app/export.rs` (covers `& < > " '`); `render_html` runs every interpolated string (account names, coin commodities, month names, currency symbol) through it.

2. **JSON export incomplete escape** — ✅ DONE. `serde_json = "1"` added to `Cargo.toml`; `render_json` now builds a `serde_json::Value` tree and serialises via `to_string_pretty`. All escaping (control chars, unicode, `\` etc.) handled by the library.

3. **Thread join `.unwrap()`** — ✅ DONE. `join_or_panic_err` helper in `src/app/refresh.rs` and an inline equivalent in `load_all_coin_chart_series` (`src/data/portfolio.rs`) downgrade panics to `Err(anyhow!("worker panicked"))`.

4. **Stale candidate path in `find_journal`** — ✅ DONE. `tmp/Finance/all.journal` removed from the candidate list in `src/main.rs`.

5. **`run_hledger` lossy decode** — ✅ DONE. Replaced `String::from_utf8_lossy` with `String::from_utf8` and surface invalid bytes as a clear error including the offset.

6. **`parse_balance_csv` error heuristic** — ✅ DONE. Header is now read explicitly via `csv::Reader::headers` and validated case-insensitively (`"account"` first column). Bails immediately with a clear error otherwise.

7. **`watcher.rs`** — ✅ DONE. Switched to `RecursiveMode::NonRecursive`; added a comment documenting why `last_event` mutation in the `FnMut` handler is sound under notify v7.

8. **`compute_price_alerts` O(N×M)** — ✅ DONE. Single pre-pass buckets `price_history` by commodity into a `HashMap`; per-coin "previous price" lookup uses `partition_point` on the already-sorted slice (O(log M)).

9. **Detail loads block the UI** — ✅ DONE. `open_account_detail`, `open_expense_detail`, `open_income_detail` now spawn a worker via `spawn_detail_load`; results land via per-kind `Option<mpsc::Receiver<DetailLoad>>` channels drained by the new `App::check_background()` helper.

10. **`reload_monthly_year` / `reload_net_worth` block the UI** — ✅ DONE. Both functions spawn workers and send `MonthlyYearLoad` / `NetWorthLoad` messages; results are applied by `check_background` (with a stale-result guard for `monthly_year_offset`).

11. **Refresh requests dropped while one is in flight** — ✅ DONE. Added `pending_refresh: Option<TabFlags>`. `start_refresh_tabs` merges into the pending request when busy; `check_refresh` re-fires it after `apply_refresh`.

12. **Hardcoded year cycle limit** — ✅ DONE. `cycle_year_back` no longer clamps to `> -3`.

13. **`tabs_loaded` set on `TabData::Err`** — ✅ DONE. `apply_refresh` captures per-tab `Ok` status before consuming the `TabData` and only flips `tabs_loaded` for tabs whose primary source returned `Ok`.

14. **CLI `parse_tab` case-sensitive** — ✅ DONE. `parse_tab` lowercases the input before matching.

15. **Mouse hit-test desync risk** — ✅ DONE. `MONTHLY_BAR_WIDTH`, `MONTHLY_BAR_GAP`, `MONTHLY_GROUP_GAP`, `MONTHLY_BARS_PER_GROUP` and `MONTHLY_GROUP_WIDTH` consts in `src/ui/monthly.rs` are the single source of truth; `BarChart` builder and `on_monthly_chart_click` both consume them via `crate::ui::MONTHLY_GROUP_WIDTH`.

16. **`load_recent_transactions` / `search_transactions` parse-then-truncate** — ✅ DONE. Both stream into a `VecDeque` capped via the new `push_capped` helper (`SEARCH_RESULTS_CAP = 100`, caller-supplied `n` for register).

17. **`Config::refresh_interval = 0` silently falls back** — ✅ DONE. Added `Config::validate()`; `Config::load` extends its warnings with the result, surfacing the bogus value at startup before the silent 300s fallback kicks in.

18. **`pl_pct` clamp/format redundancy** — ✅ DONE. Single `pct.abs() > 9999.0` branch in `src/ui/portfolio.rs`; clamping happens only inside that branch.

---

## 4. Cleanup

1. Commit the `strategy.md` deletion (or update it — this file replaces it).
2. `RecurringExpense.occurrences` is `#[allow(dead_code)]`. Either show it in the UI (e.g. "Netflix · 6/12 months") or drop the field.
3. `json_str` floats at the top of `app/mod.rs` — belongs in the new `app/export.rs`.
4. `expense_color` hardcodes German + English category lists. Already overridable via `[colors.expenses]`. Consider trimming the built-ins to English defaults and documenting overrides.
5. `App::has_watcher()` is only used once in the main loop — inline it.
6. `parse_color` doesn't accept `lightred`, `lightgreen`, etc. that `expense_color` returns. Round-trip via config is impossible for those. Add them.
7. `fmt_amount_compact` always appends the currency symbol. Chart axes occasionally want symbol-less compact numbers — split into `fmt_compact(amount, decimals)` and `fmt_amount_compact = fmt_compact + symbol`.
8. Remove the `selected: usize` field on `MonthlyData` — `App.combined_selected` is now the source of truth and `selected` is only read once during `rebuild_combined_months` for migration.

---

## 5. Suggested PR Order

To keep reviews small and risk low:

1. **Pure mechanical split** (no behaviour change): split `src/app/mod.rs` and `src/data.rs` per §1.1 and §1.2. Largest readability win, easiest review.
2. **Event handler extraction** (§1.3): `src/events.rs`.
3. **Hardening pass**: HTML/JSON escaping (§3.1, §3.2), thread join handling (§3.3), drop `tmp/Finance/all.journal` (§3.4), case-insensitive `--tab` (§3.14).
4. **Async detail and reload** (§3.9, §3.10): unify all hledger calls behind the background-refresh channel.
5. **State grouping** (§2.3): introduce per-area state structs in `App`. Touches everything — do it after the split when files are small.
6. **Polish**: ChartMode enum (§2.7), EnumMap for tab flags (§2.6), MONTH_NAMES dedup (§2.4), drop dead `occurrences` (§4.2).

---

## 6. Out of Scope (For Now)

- Replacing the manual JSON writer with `serde_json` is mentioned under hardening but pulls in a noticeable dep tree — confirm with a `cargo bloat` check first.
- The FIFO cost-basis logic in `load_all_coin_chart_series` is dense but correct and well-commented. Don't refactor without a property-based test guard first.
- ratatui rendering allocations per frame are acceptable at the 250ms tick — no need to optimise.
