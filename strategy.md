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

1. **`apply_refresh` boilerplate**. 9 nearly-identical `match TabData<T>` arms. Extract a macro:

   ```rust
   macro_rules! apply_field { ($field:expr, $src:expr, $errs:expr) => {
       match $src {
           TabData::Ok(v) => $field = v,
           TabData::Err(e) => $errs.push(e),
           TabData::NotRequested => {}
       }
   }}
   ```

2. **`load_all_data` 8-tuple thread join**. Replace ad-hoc tuple with `LoadHandles` struct (or per-tab loader function returning a `RefreshResult` slice). Easier to extend when adding a new data source.

3. **Group `App` fields**. Currently ~50 flat fields. Group into:
   - `monthly_view: MonthlyViewState` — focus, year_offset, combined_months/selected, all `*_detail`/`detail_*_name` fields
   - `accounts_view: AccountsViewState` — filter state, table state, detail
   - `portfolio_view: PortfolioViewState` — selected_holding, range, scroll_offset, chart_stacked
   - `overlay: OverlayState` — search, help, export prompt, alerts
   - `geometry: Geometry` — all `Rect` fields written by render pass
   - `data: AppData` — holdings, balances, monthly, forecasts, price history

4. **Deduplicate `MONTH_NAMES`**. Defined 3× (`rebuild_combined_months`, `cash_flow_forecast`, mirrored by `data::month_name`). Single `pub(crate) const MONTH_NAMES: [&str; 12]` plus `month_index(name) -> Option<usize>` to replace the 12-arm match in `month_name_to_period`.

5. **Test fixtures fragility**. `fixture_empty` enumerates every field — breaks on each new `App` field. Either implement `Default for App` and use `..Default::default()` in fixtures, or build via `App::new` + targeted overrides.

6. **`Tab::index()` array indexing**. Replace `[bool; 3]` indexed by enum with `EnumMap<Tab, bool>` (via `enum-map` crate) or a typed `TabFlags { portfolio, accounts, monthly }` struct with named accessors. Removes a class of off-by-index bugs.

7. **`chart_stacked: bool`**. Promote to `enum ChartMode { Stacked, Unstacked }` to match the config string and the toggle key.

8. **Per-tab dispatch repetition**. The `match self.tab { Tab::Portfolio => ..., Tab::Accounts => ..., Tab::Monthly => ... }` block appears 8+ times in scroll/mouse code. Consider a per-tab state struct with a small trait (`fn scroll_up(&mut self)`, `fn page_size(&self, area: Rect) -> usize`).

9. **Style helper**. Frequent `Style::default().fg(theme.muted)` etc. — a tiny helper or macro `fg!(theme.muted)` reduces noise in render code.

---

## 3. Hardening (Bugs & Risks)

1. **HTML export XSS**. `render_html` interpolates account names, coin commodities, month names raw. An account named `<script>alert(1)</script>` executes in the browser. Add an `html_escape` helper (replace `& < > " '`).

2. **JSON export incomplete escape**. `json_str` only escapes `\` and `"`. Newlines, tabs, control chars (< 0x20) and unicode outside BMP all break parsers. Replace with `serde_json` (add the dep — you already use `serde`).

3. **Thread join `.unwrap()`**. Both `load_all_data` and `load_all_coin_chart_series` call `.join().unwrap()`. A panicked worker takes the whole app down. Wrap with `.unwrap_or(Err(anyhow::anyhow!("worker panicked")))` or propagate as `TabData::Err`.

4. **Stale candidate path in `find_journal`**. `main.rs:121` lists `"tmp/Finance/all.journal"` — a personal-machine leftover. Drop it.

5. **`run_hledger` lossy decode**. `String::from_utf8_lossy(&output.stdout)` silently substitutes U+FFFD for invalid bytes. Fine for normal hledger output, but worth surfacing as an error if it ever happens.

6. **`parse_balance_csv` error heuristic**. Returns `Err` only when `text.len() > 20 && result empty && !first_line.contains("account")`. Fragile — parse the header explicitly and bail with a clear error.

7. **`watcher.rs`**:
   - Watching the parent dir with `RecursiveMode::Recursive` is noisy on big home dirs. Switch to `NonRecursive`.
   - Debounce uses `last_event` captured by `move` — works because `EventHandler` is `FnMut` in notify v7, but worth a comment.

8. **`compute_price_alerts` is O(N×M)**. Walks `price_history` per coin per refresh. Pre-index by commodity once (already sorted by date) and binary-search.

9. **Detail loads block the UI**. `open_account_detail`, `open_expense_detail`, `open_income_detail` call hledger synchronously on the event thread. The loading spinner never shows. Move to the same background-thread pattern as `start_refresh_tabs`.

10. **`reload_monthly_year` and `reload_net_worth` block the UI** for the same reason. Make them async via the existing refresh channel.

11. **Refresh requests dropped while one is in flight**. Pressing `r` during a refresh is silently ignored. Add a `pending_refresh: Option<[bool; 3]>` flag and re-fire on completion.

12. **Hardcoded year cycle limit**. `cycle_year_back` clamps `monthly_year_offset > -3`. No real reason for the limit — remove or make configurable.

13. **`tabs_loaded[i] = true` set even on `TabData::Err`** in `apply_refresh`. Means a tab that errored never auto-retries. Only mark loaded on `Ok`.

14. **CLI `parse_tab` is case-sensitive**. `--tab Portfolio` fails with a confusing message. Lowercase before match.

15. **Mouse hit-test desync risk**. `on_monthly_chart_click` hardcodes `group_width = 8u16`, computed from `bar_width(3) + bar_gap(0) + group_gap(2) + 2 bars`. If render config changes, click positions silently desync. Move to a shared const used by both render and hit-test.

16. **`load_recent_transactions` / `search_transactions` parse-then-truncate**. `split_off(len - n)` after parsing every record. For large journals this is wasted memory. Stream into a `VecDeque` with a cap.

17. **`Config::refresh_interval = 0`** silently falls back to 300s (`refresh_duration`). Add a validation warning at load time.

18. **`pl_pct` clamp/format**. `portfolio.rs:152–162` clamps to ±9999% but separately tests `pct.abs() > 9999.0` to switch format. Logic is redundant — simplify.

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
