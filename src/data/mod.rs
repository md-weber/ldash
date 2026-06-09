use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::Path;
use std::process::Command;

mod balances;
mod monthly;
mod parse;
mod payee;
mod portfolio;
mod prices;
mod transactions;

pub use balances::{
    load_account_balances_eur, load_crypto_balances, load_liability_balances_eur,
    load_liability_progress, load_net_worth_breakdown, load_net_worth_history, LiabilityProgress,
};
pub use monthly::{
    load_last_year_monthly, load_monthly_data, load_monthly_for_period, load_monthly_with_forecast,
};
#[allow(unused_imports)]
pub use parse::parse_eu_number;
#[allow(unused_imports)]
pub(crate) use parse::{month_index, month_name, MONTH_NAMES};
pub use portfolio::{compute_portfolio, load_all_coin_chart_series};
pub use prices::{latest_prices, load_price_history};
pub use payee::{load_payee_analytics, PayeeSummary};
pub use transactions::{load_recent_transactions, search_transactions};

pub(crate) fn commodity_key(raw: &str) -> String {
    raw.trim().trim_matches('"').to_ascii_uppercase()
}

/// Known fiat currencies (canonical uppercase forms), in detection priority order.
const KNOWN_FIAT: &[&str] = &[
    "EUR", "USD", "GBP", "JPY", "CHF", "CAD", "AUD", "NZD", "SEK", "NOK", "DKK",
];

/// Detect the primary fiat currency used in a journal by running `hledger commodities`.
///
/// Prefers known fiat currencies (EUR, USD, …). Falls back to the first
/// commodity found so that non-standard journals (e.g. single-letter benchmark
/// files) still get a usable display currency instead of silently defaulting to
/// the config value and showing nothing.
///
/// Returns `None` when hledger fails or the journal has no commodities at all.
pub fn detect_journal_currency(journal_path: &Path) -> Option<String> {
    let jp = journal_path.to_str()?;
    let output = run_hledger(&["-f", jp, "commodities"]).ok()?;
    let mut first_commodity: Option<String> = None;
    for line in output.lines() {
        let c = canonical_currency(line.trim());
        if c.is_empty() {
            continue;
        }
        if first_commodity.is_none() {
            first_commodity = Some(c.clone());
        }
        if KNOWN_FIAT.contains(&c.as_str()) {
            return Some(c);
        }
    }
    first_commodity
}

pub(crate) fn canonical_currency(raw: &str) -> String {
    let s = raw.trim();
    if s.is_empty() {
        return String::new();
    }
    if s == "€" || s.eq_ignore_ascii_case("EUR") {
        return "EUR".to_string();
    }
    if s == "$" || s.eq_ignore_ascii_case("USD") || s.eq_ignore_ascii_case("US$") {
        return "USD".to_string();
    }
    if s == "£" || s.eq_ignore_ascii_case("GBP") {
        return "GBP".to_string();
    }
    if s == "¥" || s.eq_ignore_ascii_case("JPY") {
        return "JPY".to_string();
    }
    s.to_ascii_uppercase()
}

pub(crate) fn currency_matches(configured: &str, commodity: &str) -> bool {
    let a = canonical_currency(configured);
    let b = canonical_currency(commodity);
    !a.is_empty() && a == b
}

#[derive(Debug, Clone)]
pub struct PriceEntry {
    pub date: NaiveDate,
    pub commodity: String,
    pub price_eur: f64,
}

#[derive(Debug, Clone)]
pub struct CryptoHolding {
    pub commodity: String,
    pub amount: f64,
    pub price_eur: f64,
    pub value_eur: f64,
}

#[derive(Debug, Clone)]
pub struct AccountBalance {
    pub account: String,
    pub amount: f64,
    pub commodity: String,
}

#[derive(Debug, Default, Clone)]
pub struct SingleMonth {
    pub month_name: String,
    pub income: Vec<(String, f64)>,
    pub expenses: Vec<(String, f64)>,
    pub total_income: f64,
    pub total_expenses: f64,
}

#[derive(Debug, Default, Clone)]
pub struct MonthlyData {
    pub months: Vec<SingleMonth>,
}

#[derive(Debug, Clone, Default)]
pub struct NetWorthSeries {
    pub points: Vec<(f64, f64)>,
    pub labels: Vec<(NaiveDate, f64)>,
}

/// Stacked asset-composition breakdown over time.
///
/// Each layer is cumulative (suitable for a stacked area chart):
/// - `layer_investments`: investments portion alone
/// - `layer_invest_crypto`: investments + crypto
/// - `layer_total`: investments + crypto + bank (total assets)
///
/// Account classification:
/// - `assets:bank*`    → bank
/// - `assets:crypto*`  → crypto
/// - everything else   → investments
#[derive(Debug, Clone, Default)]
pub struct NetWorthBreakdownSeries {
    pub layer_investments: Vec<(f64, f64)>,
    pub layer_invest_crypto: Vec<(f64, f64)>,
    pub layer_total: Vec<(f64, f64)>,
    pub labels: Vec<(NaiveDate, f64)>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub date: NaiveDate,
    pub description: String,
    pub amount: f64,
    pub running_total: f64,
    /// Account this posting belongs to. `None` for account/expense/income
    /// detail loads (where the account is already implicit); `Some` for global
    /// search results so drill-down can navigate to the owning account.
    pub account: Option<String>,
}

/// Three EUR-denominated time series for the portfolio analysis chart.
#[derive(Debug, Clone, Default)]
pub struct CoinChartSeries {
    /// Cumulative EUR invested (cost basis) over time.
    pub investment: Vec<(f64, f64)>,
    /// EUR gain/loss from price movement on purchased coins.
    pub price_growth: Vec<(f64, f64)>,
    /// EUR value of coins received via staking rewards.
    pub staking_growth: Vec<(f64, f64)>,
    /// EUR price per coin at each sample point.
    pub price: Vec<(f64, f64)>,
}

impl CoinChartSeries {
    pub fn total_invested(&self) -> f64 {
        self.investment.last().map(|p| p.1).unwrap_or(0.0)
    }
}

/// Return the calendar year of the last transaction in the journal, or `None`
/// when hledger fails or produces no output.
pub fn last_journal_year(journal_path: &Path) -> Option<i32> {
    let jp = journal_path.to_str()?;
    let out = run_hledger(&["-f", jp, "print", "-1"]).ok()?;
    // `hledger print -1` outputs the last transaction.  The first non-empty
    // line starts with the date in YYYY-MM-DD or YYYY/MM/DD format.
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let date_part = trimmed.split([' ', '\t']).next()?;
        let year_str = date_part.split(['-', '/']).next()?;
        if let Ok(y) = year_str.parse::<i32>() {
            return Some(y);
        }
    }
    None
}

/// Run hledger with the given args. Returns stdout on success.
/// On non-zero exit, returns an Err whose message includes the captured stderr.
pub(crate) fn run_hledger(args: &[&str]) -> Result<String> {
    let output = Command::new("hledger")
        .args(args)
        .output()
        .context("Failed to spawn hledger — is it installed and on PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let msg = if stderr.is_empty() {
            format!("hledger exited with {}", output.status)
        } else {
            stderr
        };
        return Err(anyhow::anyhow!(msg));
    }

    // Surface non-UTF-8 stdout as an error rather than silently substituting
    // U+FFFD — invalid bytes here usually mean a corrupted journal or a
    // platform-encoding mismatch we want to know about.
    String::from_utf8(output.stdout).map_err(|e| {
        anyhow::anyhow!(
            "hledger produced non-UTF-8 output (invalid byte at offset {})",
            e.utf8_error().valid_up_to()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{commodity_key, currency_matches};

    #[test]
    fn currency_matches_eur_aliases() {
        assert!(currency_matches("€", "EUR"));
        assert!(currency_matches("EUR", "€"));
        assert!(currency_matches("eur", "EUR"));
    }

    #[test]
    fn currency_matches_usd_aliases() {
        assert!(currency_matches("$", "USD"));
        assert!(currency_matches("usd", "$"));
    }

    #[test]
    fn currency_matches_rejects_mismatch() {
        assert!(!currency_matches("€", "$"));
    }

    #[test]
    fn commodity_key_normalizes_case_and_quotes() {
        assert_eq!(commodity_key("\"btc\""), "BTC");
        assert_eq!(commodity_key(" Eth "), "ETH");
    }
}
