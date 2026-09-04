use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use std::collections::HashMap;
use std::path::Path;

use super::parse::parse_eu_number;
use super::{currency_matches, month_name, run_hledger};

/// Per-month liquid-cash activity for the current calendar year.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiquidMonthlyData {
    /// Sum of all whitelisted accounts per month.
    pub total: Vec<(String, f64)>,
    /// Per-account monthly net change (same month names as `total`).
    pub by_account: Vec<(String, Vec<(String, f64)>)>,
}

/// Per-month net change in liquid cash (whitelisted accounts only) for the
/// current calendar year.
///
/// `liquid_prefixes` are account-name prefixes (e.g. `"assets:bank:checking"`)
/// whitelisted as freely spendable. Only matching accounts are summed, so
/// non-liquid assets — AFA / depreciation, bounded mortgage savings
/// (Tilgungsaussetzung), investments — are excluded simply by not listing
/// them.
///
/// Unlike a running balance, this reports the *change* within each month
/// (deposits minus withdrawals across the whitelisted accounts), so months
/// with a big one-off expense or mortgage principal payment show a dip even
/// though the running total stays positive.
///
/// Computed via `hledger balance` over `monthly this year`. When
/// `with_forecast` is true, `--forecast` runs from today through year-end so
/// future months carry periodic-rule projections and the current (partial)
/// month carries actuals plus the projected remainder. When false, only
/// posted transactions are included.
///
/// Returns empty vectors when `liquid_prefixes` is empty (feature off) or
/// when no matching accounts have any activity this year.
pub fn load_liquid_cash_monthly(
    journal_path: &Path,
    liquid_prefixes: &[String],
    currency_symbol: &str,
) -> Result<LiquidMonthlyData> {
    load_liquid_cash_monthly_inner(journal_path, liquid_prefixes, currency_symbol, false)
}

/// Same as [`load_liquid_cash_monthly`], with `--forecast=TODAY..YEAR_END`.
pub fn load_liquid_cash_monthly_with_forecast(
    journal_path: &Path,
    liquid_prefixes: &[String],
    currency_symbol: &str,
) -> Result<LiquidMonthlyData> {
    load_liquid_cash_monthly_inner(journal_path, liquid_prefixes, currency_symbol, true)
}

fn load_liquid_cash_monthly_inner(
    journal_path: &Path,
    liquid_prefixes: &[String],
    currency_symbol: &str,
    with_forecast: bool,
) -> Result<LiquidMonthlyData> {
    if liquid_prefixes.is_empty() {
        return Ok(LiquidMonthlyData::default());
    }

    let today = Local::now().date_naive();
    let year_end = NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap();
    let forecast_arg = format!(
        "--forecast={}..{}",
        today.format("%Y-%m-%d"),
        year_end.format("%Y-%m-%d")
    );

    let jp = journal_path.to_str().unwrap_or("all.journal");

    let mut args: Vec<&str> = vec![
        "-f",
        jp,
        "balance",
        "-O",
        "csv",
        "--layout",
        "bare",
        "--no-total",
        "--empty",
        "-V",
        "-p",
        "monthly this year",
    ];
    if with_forecast {
        args.push(&forecast_arg);
    }
    for p in liquid_prefixes {
        args.push(p.as_str());
    }

    let text = run_hledger(&args)?;
    parse_liquid_bare_csv(&text, currency_symbol)
}

/// Parse a `balance --layout bare -O csv` table (rows: account, commodity,
/// month1, month2, …) into per-month sums, filtering rows by
/// `currency_symbol` and mapping each `YYYY-MM` column header to its full
/// English month name.
fn parse_liquid_bare_csv(text: &str, currency_symbol: &str) -> Result<LiquidMonthlyData> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr
        .headers()
        .context("No CSV headers from hledger balance")?
        .clone();

    let month_cols: Vec<(usize, NaiveDate)> = headers
        .iter()
        .enumerate()
        .skip(2)
        .filter_map(|(i, h)| {
            NaiveDate::parse_from_str(&format!("{}-01", h.trim()), "%Y-%m-%d")
                .ok()
                .map(|d| (i, d))
        })
        .collect();

    if month_cols.is_empty() {
        return Ok(LiquidMonthlyData::default());
    }

    let mut sums = vec![0.0f64; month_cols.len()];
    let mut accounts: HashMap<String, Vec<f64>> = HashMap::new();

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 2 {
            continue;
        }
        let account = fields[0].trim().trim_matches('"').to_string();
        let commodity = fields.get(1).unwrap_or("").trim().trim_matches('"');
        if !currency_matches(currency_symbol, commodity) {
            continue;
        }
        let entry = accounts
            .entry(account)
            .or_insert_with(|| vec![0.0; month_cols.len()]);
        for (idx, &(col, _)) in month_cols.iter().enumerate() {
            if col >= fields.len() {
                continue;
            }
            if let Some(amount) = parse_eu_number(&fields[col]) {
                entry[idx] += amount;
                sums[idx] += amount;
            }
        }
    }

    let total = month_cols
        .iter()
        .enumerate()
        .map(|(idx, &(_, date))| (month_name(date.month() as usize).to_string(), sums[idx]))
        .collect();

    let mut by_account: Vec<(String, Vec<(String, f64)>)> = accounts
        .into_iter()
        .map(|(name, amounts)| {
            let series = month_cols
                .iter()
                .enumerate()
                .map(|(idx, &(_, date))| {
                    (
                        month_name(date.month() as usize).to_string(),
                        amounts[idx],
                    )
                })
                .collect();
            (name, series)
        })
        .collect();
    by_account.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(LiquidMonthlyData { total, by_account })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prefixes_returns_empty() {
        let path = std::path::Path::new("/nonexistent.journal");
        let got = load_liquid_cash_monthly(path, &[], "€").unwrap();
        assert!(got.total.is_empty());
        assert!(got.by_account.is_empty());
    }

    #[test]
    fn parses_bare_csv_sums_matching_currency_rows() {
        let csv = "account,commodity,2026-01,2026-02\n\
                   assets:cash,€,\"200,00\",\"-50,00\"\n\
                   assets:raisin,€,\"100,00\",\"25,00\"\n\
                   assets:crypto:btc,BTC,\"0,01\",\"0,00\"\n";
        let result = parse_liquid_bare_csv(csv, "€").unwrap();
        assert_eq!(result.total.len(), 2);
        assert!((result.total[0].1 - 300.0).abs() < 1e-9);
        assert_eq!(result.total[1].0, "February");
        assert!((result.total[1].1 - (-25.0)).abs() < 1e-9);
        assert_eq!(result.by_account.len(), 2);
        let cash = result
            .by_account
            .iter()
            .find(|(n, _)| n == "assets:cash")
            .unwrap();
        assert!((cash.1[0].1 - 200.0).abs() < 1e-9);
    }

    #[test]
    fn parses_bare_csv_ignores_wrong_currency() {
        let csv = "account,commodity,2026-01\nassets:cash,$,\"200,00\"\n";
        let result = parse_liquid_bare_csv(csv, "€").unwrap();
        assert_eq!(result.total.len(), 1);
        assert!((result.total[0].1 - 0.0).abs() < 1e-9);
        assert!(result.by_account.is_empty());
    }

    #[test]
    fn parses_bare_csv_empty_headers_returns_empty() {
        let result = parse_liquid_bare_csv("account,commodity\n", "€").unwrap();
        assert!(result.total.is_empty());
        assert!(result.by_account.is_empty());
    }
}
