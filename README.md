# ⬡ ldash — HLedger Dashboard TUI

A terminal dashboard for [hledger](https://hledger.org) that gives you a real-time overview of your finances — crypto portfolio, net worth, and monthly income/expenses — all without leaving the terminal.

Built with [Ratatui](https://ratatui.rs) and Rust.

## Screenshots

| Portfolio | Accounts | Monthly |
|:---------:|:--------:|:-------:|
| ![Portfolio tab](screenshots/portfolio.png) | ![Accounts tab](screenshots/accounts.png) | ![Monthly tab](screenshots/monthly.png) |

<!--
  Add your screenshots to the screenshots/ directory:
    screenshots/portfolio.png
    screenshots/accounts.png
    screenshots/monthly.png
-->

## Features

- **Crypto Portfolio** — Holdings table with price, value, allocation %, and P/L tracking (invested vs price gain vs staking rewards)
- **Portfolio Chart** — Per-coin analysis showing investment cost basis, price growth, and staking income over time
- **Net Worth History** — Interactive chart with selectable time ranges (1Y / 2Y / 5Y / All)
- **Account Balances** — All asset accounts with EUR valuations and visual bar indicators
- **Account Drill-Down** — Select any account and view its recent transactions
- **Monthly Income & Expenses** — Bar chart overview with per-category breakdown, savings rate gauge, and year-over-year comparison
- **Background Refresh** — Non-blocking data loading with parallel hledger calls
- **Auto-Refresh** — Detects journal file changes and reloads automatically
- **Lazy Tab Loading** — Only loads data for the active tab on first visit

## Requirements

- [Rust](https://rustup.rs) (1.70+)
- [hledger](https://hledger.org/install.html) installed and available in `$PATH`
- An hledger journal file with a companion `prices.journal` for crypto price history

## Installation

```bash
git clone https://github.com/yourusername/ledger_dashboard.git
cd ledger_dashboard
cargo install --path .
```

## Usage

```bash
# Pass journal path directly
ldash /path/to/all.journal

# Or set the environment variable
export LEDGER_FILE=/path/to/all.journal
ldash

# Or run from a directory containing all.journal
cd ~/Finance && ldash
```

## Keybindings

| Key | Action |
|-----|--------|
| `1` / `2` / `3` | Switch tab |
| `Tab` / `Shift-Tab` | Next / previous tab |
| `↑` `k` / `↓` `j` | Scroll / select |
| `←` `h` / `→` `l` | Month navigation / net worth range |
| `Enter` | Drill into account / expense category |
| `c` | Toggle expense category colors |
| `r` | Force refresh |
| `?` | Toggle help overlay |
| `Esc` | Back / close |
| `q` | Quit |

## Configuration

On first launch, ldash creates a config file at `~/.config/ldash/config.toml` with all options commented out. Edit it to customize behavior.

```toml
# Path to hledger journal (overrides $LEDGER_FILE and CLI arg)
# journal = "/path/to/all.journal"

# Auto-refresh interval in seconds (default: 300)
# refresh_interval = 300

# Default tab on startup: "portfolio", "accounts", "monthly"
# default_tab = "portfolio"

# Number format: "eu" (1.000,00) or "us" (1,000.00)
# number_format = "eu"

# Currency symbol shown in UI
# currency_symbol = "€"

# Expense category color overrides
# Colors: red, green, blue, yellow, cyan, magenta, white, darkgray,
#         or RGB hex like "#B48CFF"
# [colors.expenses]
# Wohnen = "blue"
# Essen = "yellow"
```

Missing or partially filled config is fine — defaults fill any gaps.

## Journal Structure

ldash expects a standard hledger setup:

```
~/Finance/
├── all.journal          # main journal (includes others)
├── prices.journal       # P directives for crypto prices
├── 2025.journal
└── 2026.journal
```

Crypto accounts should live under `assets:crypto`, and price entries in `prices.journal` should follow the format:

```
P 2026-04-14 BTC 76543,21 €
P 2026-04-14 SOL 123,45 €
```

## License

MIT
