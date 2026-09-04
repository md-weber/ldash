# haushalt-demo

Fictional household journal that mirrors a typical German ldash setup: C24/N26 bank accounts, crypto on Bitbox/Bitvavo/Exodus, kids savings depots, mortgage and kitchen loans, forecast rules, and recurring expenses.

**All amounts are invented.** Account names match a real ldash layout but no personal data is included.

## Quick start

From the `ledger_dashboard` repo root:

```bash
ldash -f examples/haushalt-demo/all.journal -c examples/haushalt-demo/config.toml
```

Or with an absolute path:

```bash
ldash -f /path/to/ledger_dashboard/examples/haushalt-demo/all.journal \
      -c /path/to/ledger_dashboard/examples/haushalt-demo/config.toml
```

Validate without launching the TUI:

```bash
ldash --check -f examples/haushalt-demo/all.journal -c examples/haushalt-demo/config.toml
hledger -f examples/haushalt-demo/all.journal check
```

## What it exercises

| ldash feature | Demo data |
|---------------|-----------|
| **Dashboard** | Income/expense breakdown, recurring split, donut chart, liquid cash lines, YTD |
| **Accounts** | Bank, ETF, kids, property, liabilities, savings goals |
| **Monthly** | Budgets, sparklines (20+ months of history), forecast overlay |
| **Register** | Salary, groceries, crypto trades, mortgage payments |
| **Portfolio** | BTC, ETH, SOL, LINK, TON with cost basis and staking rewards |
| **Liquid cash** | 10 cash accounts from `config.toml` |

## File layout

```
haushalt-demo/
├── all.journal              # main include file
├── accounts.journal         # full account tree (from real layout)
├── prices.journal           # fake P directives
├── 2025.journal             # opening balances + 2025 flow
├── 2026.journal             # Jan–Sep 2026
├── 2026-crypto.journal      # crypto trades & rewards
├── forecast.journal         # salary + loan forecasts
├── recurring.journal        # recurring expense rules
├── kids.journal             # kids depot balance updates
├── kredit_wohnung/
│   └── kredit_forecast.journal
└── config.toml              # ldash config tuned for this journal
```

## Notes

- EU number format (`1.234,56 €`) throughout.
- Periodic rules (`~`) drive forecast on the Dashboard and Monthly tabs. Press `F` to toggle forecast overlay.
- `price_fetch` is configured but will only append to `prices.journal` if you press `P` or prices for today are missing.
