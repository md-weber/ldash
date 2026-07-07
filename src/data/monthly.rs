use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use std::path::Path;

use super::parse::parse_monthly_csv;
use super::{run_hledger, MonthlyData};

pub fn load_monthly_data(journal_path: &Path, currency_symbol: &str) -> Result<MonthlyData> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "incomestatement",
        "-O",
        "csv",
        "--no-total",
        "-p",
        "monthly this year",
    ])?;
    parse_monthly_csv(&text, currency_symbol)
}

pub fn load_monthly_for_period(
    journal_path: &Path,
    period: &str,
    currency_symbol: &str,
) -> Result<MonthlyData> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "incomestatement",
        "-O",
        "csv",
        "--no-total",
        "-p",
        period,
    ])?;
    parse_monthly_csv(&text, currency_symbol)
}

/// Load income statement for the current year with `--forecast` applied.
///
/// The forecast period is set to `TODAY..YEAR_END` so that:
/// - past months carry only actual transactions,
/// - the current (partial) month carries actual transactions plus periodic-rule
///   projections from today to end of month,
/// - future months carry only periodic-rule projections.
///
/// Returns an empty `MonthlyData` when the journal has no periodic transaction
/// rules (hledger simply returns 0 for future months, which our parser filters
/// out).
pub fn load_monthly_with_forecast(
    journal_path: &Path,
    currency_symbol: &str,
) -> Result<MonthlyData> {
    let today = chrono::Local::now().date_naive();
    let year_end = NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap();

    let forecast_arg = format!(
        "--forecast={}..{}",
        today.format("%Y-%m-%d"),
        year_end.format("%Y-%m-%d")
    );

    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "incomestatement",
        "-O",
        "csv",
        "--no-total",
        "-p",
        "monthly this year",
        &forecast_arg,
    ])?;

    parse_monthly_csv(&text, currency_symbol)
}

pub fn load_last_year_monthly(journal_path: &Path, currency_symbol: &str) -> Result<MonthlyData> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "incomestatement",
        "-O",
        "csv",
        "--no-total",
        "-p",
        "monthly last year",
    ])?;
    parse_monthly_csv(&text, currency_symbol)
}
