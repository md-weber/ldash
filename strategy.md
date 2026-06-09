# ldash 1.4.0 — Feature Strategy

Status: draft for discussion. Target version: **1.4.0** (minor — additive, no breaking config/CLI).

## Where we are (1.3.0)

`ldash` is a read-only hledger TUI with 3 tabs:

- **Portfolio** — holdings, allocation %, P/L (invested vs price vs staking), per-coin chart (stacked/unstacked), price alerts.
- **Accounts** — asset balances, net worth history (YTD/1Y/2Y/5Y/All), stacked breakdown, account drill-down, savings goals, liability tracking + est. time-to-repay.
- **Monthly** — income/expense bars, per-category breakdown, savings-rate gauge, YoY compare, cash-flow forecast (`--forecast`), recurring-expense detection, budgets, payee screen (`p`).

Cross-cutting: background/parallel hledger calls, auto-refresh on file change, lazy tab loading, multi-journal switch (`Ctrl-O`/`Ctrl-S`), theming + hot-reload, EU/US number format, export (`e`), clipboard (`Y`), global search (`/`).

## Design constraints

- **Read-only.** ldash never writes the journal. Keep it that way (writing = different trust/risk profile).
- **hledger is the engine.** Prefer pushing computation into hledger queries over reimplementing accounting logic.
- **TUI-first.** Every feature must work over SSH in a plain terminal. No mouse-required flows.
- **Config additive.** New keys default-off / sensible-default; partial config must keep working.
- **Snapshot tests.** UI changes need `insta` snapshot updates; new pub fns need unit tests (see `.cursor/rules/tests.mdc`).

## Candidate features

Ranked rough priority. Each: what, why, effort (S/M/L), risk.

### A. Transaction register / search tab — **M**
A real ledger view: filter by account/payee/date/amount, paginated, drill from anywhere. Today drill-down is shallow (account → recent txns). A dedicated register with query input (`hledger reg`) closes the biggest gap vs the hledger CLI.
- Why: most-requested pattern for "where did money go". Builds on existing global search + drill.
- Risk: large journals — must paginate/stream, not load all.

### B. Budget tab / envelope view — **M**
Promote budgets from inline bars to a first-class screen: month-over-month adherence, rollover, remaining-per-day, projected end-of-month overspend. Optionally read `hledger bal --budget` / periodic budget rules.
- Why: budgets exist but are buried; this is sticky daily-use surface.
- Risk: reconciling our config budgets vs hledger-native budget directives — pick one source of truth.

### C. Net-worth projection / FIRE view — **M/L**
Project net worth forward from avg savings rate + portfolio growth assumptions. Show coast/FIRE target, years-to-target. Configurable return/inflation assumptions.
- Why: differentiator; pairs with existing net-worth history + cash-flow forecast.
- Risk: assumption-heavy; must label clearly as projection, avoid implying advice.

### D. Multi-currency / base-currency awareness — **M**
Today EUR-centric (`price_eur`, `€`). Generalize valuation to a configurable base currency, convert via price db. Show original + converted.
- Why: unblocks non-EUR users; portfolio code already normalizes prices.
- Risk: touches portfolio + net-worth + balances valuation paths broadly.

### E. Account reconciliation / cleared status — **S/M**
Surface cleared (`*`) vs pending (`!`) vs uncleared txns; balance assertions check. Flag accounts where last assertion failed.
- Why: cheap, high-signal for daily hledger users.
- Risk: low — read-only, parse status flag.

### F. Trends & sparklines on category breakdown — **S**
Inline 6-/12-month sparkline per expense category so you see direction, not just current month.
- Why: small, visual, high perceived value. Reuses monthly data already loaded.
- Risk: low.

### G. Export/report upgrades — **S**
Current export is current-view. Add: full report bundle (CSV/JSON), scheduled snapshot, Markdown summary for sharing.
- Why: incremental on existing export infra.
- Risk: low.

### H. Configurable dashboard / tab order + custom default range — **S**
Let users reorder/hide tabs, set default net-worth range, default monthly focus.
- Why: low-effort personalization, builds on existing `show_portfolio` / `default_tab`.
- Risk: low.

### I. Command palette — **S/M**
`:`-driven palette for actions (switch journal, jump tab, set range, export) — discoverability for the growing keymap.
- Why: keymap is getting dense; palette scales it.
- Risk: low/medium.

## Decided scope (1.4.0)

Decisions: theme = **Dig deeper**, size = **one flagship feature, polished**, audience = **personal**, currency = **EUR only**.

→ **Flagship: A. Transaction Register tab.** Multi-currency (D) dropped. Reconcile (E) and sparklines (F) are backlog/stretch, only if flagship lands early.

### Flagship spec — Transaction Register

A 4th top-level tab (`4`) backed by `hledger reg`/`aregister`, focused on answering "where did this money go / find this txn".

**Core (must-have):**
- New tab `Register`, key `4`; extend `Tab` enum + `TabFlags` + lazy-load path.
- Query input bar (reuse `/` search UX): free-text hledger query — account, payee, `desc:`, `cur:`, date (`date:2026-04`).
- Result table: date | description/payee | account | amount | running balance. Scroll/page/Home/End like other tables.
- Pagination/cap for large journals — fetch bounded window (e.g. `--depth`/date-limited or `head`-style cap), never load entire register into memory.
- Drill: `Enter` on a row → full transaction detail (all postings), reusing `app/detail.rs`.
- Deep-link in: drilling from Accounts account / Monthly category / global search lands here pre-filtered.
- Export (`e`) the current register view, consistent with other tabs.

**Polish (this release, since "one feature polished"):**
- Date-range scoping tied to existing range selectors (reuse `NetWorthRange`-style control or `period:` in query).
- Empty/error/loading states mirroring Portfolio/Monthly empty-screen pattern.
- Status-flag column or styling: cleared `*` / pending `!` / uncleared (cheap reconcile signal, folds in part of E).
- Help overlay (`?`) + README keybind table + man page updated.

**Tests/quality gate:**
- Unit tests for any new pub fn (query build, register CSV parse) per `.cursor/rules/tests.mdc`.
- `insta` snapshots for the new tab render (empty, populated, drill).
- Integration test for the new hledger-calling fn, hledger-availability guarded.
- CHANGELOG `Unreleased` entry; bump `Cargo.toml` to 1.4.0 at release.

### Backlog (post-flagship / 1.5.0 candidates)
- E. Reconciliation / balance-assertion checks (beyond the status column).
- F. Category sparklines.
- B. Budget tab. C. Projection/FIRE. G. Export bundle. H. Dashboard config. I. Command palette.

## Open questions (remaining)

1. **Register as 4th tab vs overlay** — spec assumes 4th tab. Object?
2. **Query syntax exposure** — raw hledger query string (power), or guided fields (account/payee/date pickers)? Spec leans raw with examples in help.
3. **Default register window** — last 30 days? current month? last N=200 txns? Need a sensible default before first paint.
4. **Running balance** — per-account running balance (`areg`) or global (`reg`)? Affects which hledger cmd backs it.

## Out of scope for 1.4.0

- Writing/editing the journal (read-only stays hard).
- Multi-currency / base-currency conversion (deferred, EUR-only confirmed).
- Live brokerage/exchange API price fetching (keep prices.journal as source).
- Mobile/web frontend.
