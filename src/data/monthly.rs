use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use std::path::Path;

use super::parse::parse_monthly_csv;
use super::{run_hledger, MonthlyData};

pub fn load_monthly_data(journal_path: &Path, currency_symbol: &str) -> Result<MonthlyData> {
    let now = chrono::Local::now().date_naive();
    let current_month = now.month() as usize;
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
    parse_monthly_csv(&text, current_month, currency_symbol)
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
    parse_monthly_csv(&text, 0, currency_symbol)
}

/// Load income statement for the current year with `--forecast` applied.
///
/// The forecast period is set to `NEXT_MONTH_FIRST_DAY..YEAR_END` so that:
/// - past months carry only actual transactions,
/// - the current (partial) month carries only actual transactions,
/// - future months carry periodic-rule projections.
///
/// Returns an empty `MonthlyData` when called in December (nothing left to
/// forecast) or when the journal has no periodic transaction rules (hledger
/// simply returns 0 for future months, which our parser filters out).
pub fn load_monthly_with_forecast(
    journal_path: &Path,
    currency_symbol: &str,
) -> Result<MonthlyData> {
    let today = chrono::Local::now().date_naive();

    let next_month_first = if today.month() == 12 {
        // Nothing to forecast – already in December.
        return Ok(MonthlyData::default());
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
    };
    let year_end = NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap();

    let forecast_arg = format!(
        "--forecast={}..{}",
        next_month_first.format("%Y-%m-%d"),
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

    parse_monthly_csv(&text, 0, currency_symbol)
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
    parse_monthly_csv(&text, 0, currency_symbol)
}
