# ldash v1.0 — Release Roadmap

## What We Have

Three-tab TUI with crypto portfolio (FIFO P/L, stacked/unstacked charts, allocation bars),
accounts (net worth chart, liabilities, savings goals, drill-down), and monthly
(income/expenses, budgets, YoY, YTD sparklines). Plus: search, file watcher,
hot-reload config, clipboard export, price alerts, lazy loading, background refresh.

Solid foundation. Below = what's missing before shipping 1.0.

---

## v1.0 Release Blockers (P0)

Found during pre-release audit (2026-04-22). All quick fixes — nothing
architectural, but each one breaks the install / packaging story.

### Wrong upstream URL in user-facing places

- `README.md` install command: `git clone https://github.com/yourusername/ledger_dashboard.git`
  → should be `https://codeberg.org/md-weber/ldash.git`
- `src/config.rs` default config template (line 78) embeds the same broken
  `github.com/yourusername` URL — it gets written to every new user's
  `~/.config/ldash/config.toml` on first launch
- Grep `yourusername` across the tree before tagging

### Version bump to 1.0.0

Three files still pinned to `0.1.0`:

- `Cargo.toml` → `version = "1.0.0"`
- `flake.nix` (3 occurrences across `ldash`, clippy check, test check) →
  derive from `Cargo.toml` via `cargoLock` or bump in lockstep
- `packaging/aur/ldash-bin/PKGBUILD` and `packaging/aur/ldash/PKGBUILD`:
  `pkgver=1.0.0` + regenerate `.SRCINFO` (`makepkg --printsrcinfo > .SRCINFO`)

### Homebrew formula missing

`release-engineering.md` called for `Formula/ldash.rb` in a `homebrew-ldash`
tap repo. AUR and Nix shipped, Homebrew didn't. Either:

- (a) create the formula now and host it in a tap so macOS users can
  `brew tap md-weber/ldash && brew install ldash`, or
- (b) explicitly defer to a 1.0.x patch release and document the gap in
  README's installation section

### Man page not in release tarballs

`man/ldash.1` exists but `release.yml` only packs the `ldash` binary.
The man page never reaches users. Update the Package step:

```bash
mkdir -p stage && cp "$BIN" stage/ && cp man/ldash.1 stage/
tar -czf "${{ matrix.artifact }}.tar.gz" -C stage .
```

Then update AUR `package()` and Homebrew formula to install
`man1.install "ldash.1"`.

### Integration tests skipped in CI

`tests/integration.rs` has 3 tests; 2 are `#[ignore]` because they need
`hledger` on PATH. CI never runs them, so the data-layer regressions they
were designed to catch slip through. Add to `ci.yml` build-test job:

```yaml
- run: |
    sudo apt-get update && sudo apt-get install -y hledger
- run: cargo test -- --include-ignored
```

### `cargo publish --dry-run` proves only one thing

Current CI runs it on every push. Confirm it has been green at least once
on `main` after the version bump — the dry-run validates `Cargo.toml`
metadata against crates.io rules and is the cheapest way to catch a
broken first publish.

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
