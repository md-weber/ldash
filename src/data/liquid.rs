use anyhow::{Context, Result};
use chrono::{Datelike, Local, NaiveDate};
use std::path::Path;

use super::parse::parse_eu_number;
use super::{currency_matches, month_name, run_hledger};

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
/// Computed via `hledger balance` over `monthly this year`, applying
/// `--forecast` from today through year-end so future months carry
/// periodic-rule projections and the current (partial) month carries actuals
/// plus the projected remainder.
///
/// Returns an empty `Vec` when `liquid_prefixes` is empty (feature off) or
/// when no matching accounts have any activity this year.
pub fn load_liquid_cash_monthly(
    journal_path: &Path,
    liquid_prefixes: &[String],
    currency_symbol: &str,
) -> Result<Vec<(String, f64)>> {
    if liquid_prefixes.is_empty() {
        return Ok(Vec::new());
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
        &forecast_arg,
    ];
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
fn parse_liquid_bare_csv(text: &str, currency_symbol: &str) -> Result<Vec<(String, f64)>> {
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
        return Ok(Vec::new());
    }

    let mut sums = vec![0.0f64; month_cols.len()];

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 2 {
            continue;
        }
        let commodity = fields.get(1).unwrap_or("").trim().trim_matches('"');
        if !currency_matches(currency_symbol, commodity) {
            continue;
        }
        for (idx, &(col, _)) in month_cols.iter().enumerate() {
            if col >= fields.len() {
                continue;
            }
            if let Some(amount) = parse_eu_number(&fields[col]) {
                sums[idx] += amount;
            }
        }
    }

    Ok(month_cols
        .iter()
        .enumerate()
        .map(|(idx, &(_, date))| (month_name(date.month() as usize).to_string(), sums[idx]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_prefixes_returns_empty() {
        let path = std::path::Path::new("/nonexistent.journal");
        let got = load_liquid_cash_monthly(path, &[], "€").unwrap();
        assert!(got.is_empty(), "empty whitelist should disable feature");
    }

    #[test]
    fn parses_bare_csv_sums_matching_currency_rows() {
        let csv = "account,commodity,2026-01,2026-02\n\
                   assets:cash,€,\"200,00\",\"-50,00\"\n\
                   assets:raisin,€,\"100,00\",\"25,00\"\n\
                   assets:crypto:btc,BTC,\"0,01\",\"0,00\"\n";
        let result = parse_liquid_bare_csv(csv, "€").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "January");
        assert!((result[0].1 - 300.0).abs() < 1e-9);
        assert_eq!(result[1].0, "February");
        assert!((result[1].1 - (-25.0)).abs() < 1e-9);
    }

    #[test]
    fn parses_bare_csv_ignores_wrong_currency() {
        let csv = "account,commodity,2026-01\nassets:cash,$,\"200,00\"\n";
        let result = parse_liquid_bare_csv(csv, "€").unwrap();
        assert_eq!(result.len(), 1);
        assert!((result[0].1 - 0.0).abs() < 1e-9);
    }

    #[test]
    fn parses_bare_csv_empty_headers_returns_empty() {
        let result = parse_liquid_bare_csv("account,commodity\n", "€").unwrap();
        assert!(result.is_empty());
    }
}
