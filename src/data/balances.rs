use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use std::path::Path;

type NetWorthPoints = (Vec<(usize, NaiveDate)>, Vec<f64>);

use super::parse::{parse_balance_csv, parse_eu_number};
use super::{
    currency_matches, run_hledger, AccountBalance, NetWorthBreakdownSeries, NetWorthSeries,
};

/// Per-liability-account debt-payoff analytics.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LiabilityProgress {
    pub account: String,
    /// Current balance (negative in hledger convention).
    pub current_balance: f64,
    /// Peak absolute debt seen in the monthly history (stored as negative).
    pub original_balance: f64,
    /// Average monthly debt reduction (absolute, positive value).
    pub avg_monthly_payment: f64,
    /// Percentage already paid off (0–100).
    pub pct_paid: f64,
    /// Estimated remaining months at current payment rate.
    pub months_to_payoff: Option<f64>,
}

/// Months between two dates, ignoring day-of-month.
fn months_between(from: NaiveDate, to: NaiveDate) -> f64 {
    let m = (to.year() - from.year()) * 12 + to.month() as i32 - from.month() as i32;
    m.max(0) as f64
}

/// Load per-liability payoff analytics by replaying monthly running-total
/// history **and** hledger periodic-transaction forecasts.
///
/// Uses `--forecast=NEXT_MONTH..+10y` with an explicit future end date so
/// that hledger generates month columns beyond today.  Without an explicit
/// future end in `-p`, hledger only produces columns up to the last actual
/// transaction and forecast months never appear.  Falls back to history-only
/// when `--forecast` is unsupported or no periodic rules exist.
pub fn load_liability_progress(
    journal_path: &Path,
    liabilities_account: &str,
    currency_symbol: &str,
) -> Result<Vec<LiabilityProgress>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let today = chrono::Local::now().date_naive();

    // Forecast window: next month → 10 years from now.
    // Using a finite end date keeps the column count bounded (~120 columns).
    let next_month_first = if today.month() == 12 {
        NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1).unwrap()
    };
    let forecast_end = NaiveDate::from_ymd_opt(today.year() + 10, today.month(), 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(today.year() + 10, 1, 1).unwrap());

    let forecast_arg = format!(
        "--forecast={}..{}",
        next_month_first.format("%Y-%m-%d"),
        forecast_end.format("%Y-%m-%d")
    );
    // The period must extend to the forecast end so hledger emits future columns.
    let period_arg = format!("monthly to {}", forecast_end.format("%Y-%m-%d"));

    // Try with forecast; fall back silently to history-only.
    let text = run_hledger(&[
        "-f",
        jp,
        "balance",
        liabilities_account,
        "-H",
        "-p",
        &period_arg,
        &forecast_arg,
        "-O",
        "csv",
        "--layout",
        "bare",
        "-V",
        "--no-total",
        "--empty",
    ])
    .or_else(|_| {
        run_hledger(&[
            "-f",
            jp,
            "balance",
            liabilities_account,
            "-H",
            "-p",
            "monthly",
            "-O",
            "csv",
            "--layout",
            "bare",
            "-V",
            "--no-total",
            "--empty",
        ])
    })?;

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr
        .headers()
        .context("No CSV headers from hledger balance (liability progress)")?
        .clone();

    // Build a map from CSV column index → (NaiveDate, is_future).
    // Columns: account[0] | commodity[1] | month…[2+].
    // Non-date header entries (account, commodity) are silently skipped by
    // the date parse failing.
    let month_cols: Vec<(usize, NaiveDate)> = headers
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, h)| {
            NaiveDate::parse_from_str(&format!("{}-01", h.trim()), "%Y-%m-%d")
                .ok()
                .map(|d| (i, d))
        })
        .collect();

    if month_cols.is_empty() {
        return Ok(Vec::new());
    }

    let mut results: Vec<LiabilityProgress> = Vec::new();

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 3 {
            continue;
        }
        let account = fields
            .get(0)
            .unwrap_or("")
            .trim()
            .trim_matches('"')
            .to_string();
        let commodity = fields.get(1).unwrap_or("").trim().trim_matches('"');

        if !currency_matches(currency_symbol, commodity) {
            continue;
        }

        // Collect (date, balance) for every month column.
        // Zero balances may come through as empty strings in --layout bare
        // output; treat them as 0.0 so post-payoff months are visible to the
        // scan below.
        let monthly: Vec<(NaiveDate, f64)> = month_cols
            .iter()
            .map(|&(col, date)| {
                let val = parse_eu_number(fields.get(col).unwrap_or("")).unwrap_or(0.0);
                (date, val)
            })
            .collect();

        if monthly.is_empty() {
            continue;
        }

        // Split into historical (≤ today) and forecast (> today).
        let historical: Vec<(NaiveDate, f64)> =
            monthly.iter().filter(|(d, _)| *d <= today).cloned().collect();
        let forecast: Vec<(NaiveDate, f64)> =
            monthly.iter().filter(|(d, _)| *d > today).cloned().collect();

        // current_balance = last historical value (or last overall if no history).
        let current_balance = historical
            .last()
            .or_else(|| monthly.last())
            .map(|(_, b)| *b)
            .unwrap_or(0.0);

        // Peak debt = max absolute value across *historical* months only.
        let max_abs = historical
            .iter()
            .map(|(_, b)| b.abs())
            .fold(0.0_f64, f64::max)
            .max(current_balance.abs());
        let original_balance = -max_abs;

        // Average monthly payment derived from historical windows.
        let payments: Vec<f64> = historical
            .windows(2)
            .filter_map(|w| {
                let reduction = w[0].1.abs() - w[1].1.abs();
                if reduction > 0.0 {
                    Some(reduction)
                } else {
                    None
                }
            })
            .collect();
        let avg_monthly_payment = if payments.is_empty() {
            0.0
        } else {
            payments.iter().sum::<f64>() / payments.len() as f64
        };

        let pct_paid = if max_abs > 0.0 {
            ((max_abs - current_balance.abs()) / max_abs * 100.0)
                .max(0.0)
                .min(100.0)
        } else {
            100.0
        };

        // Payoff estimate using forecast data:
        //
        // Find the forecast month with the smallest absolute balance — that is
        // where scheduled payments end (or the loan is closest to zero).
        // Extrapolate any remaining balance from that point using the
        // historical average payment.  This correctly handles:
        //   • Loans paid off exactly within the forecast window (min ≈ 0).
        //   • Loans where scheduled payments end mid-window leaving a residual
        //     (balance plateaus; we extrapolate from the first plateau month).
        //   • Loans extending beyond the 10-year window (min at last month).
        let months_to_payoff = if forecast.is_empty() {
            if avg_monthly_payment > 1e-6 {
                Some(current_balance.abs() / avg_monthly_payment)
            } else {
                None
            }
        } else {
            // First occurrence of the minimum absolute balance in the forecast.
            let (min_abs, min_date) = forecast
                .iter()
                .fold((f64::INFINITY, forecast[0].0), |(m_abs, m_date), &(d, b)| {
                    if b.abs() < m_abs {
                        (b.abs(), d)
                    } else {
                        (m_abs, m_date)
                    }
                });

            let months_to_min = months_between(today, min_date);

            if avg_monthly_payment > 1e-6 {
                Some(months_to_min + min_abs / avg_monthly_payment)
            } else if min_abs < 1.0 {
                Some(months_to_min)
            } else {
                None
            }
        };

        results.push(LiabilityProgress {
            account,
            current_balance,
            original_balance,
            avg_monthly_payment,
            pct_paid,
            months_to_payoff,
        });
    }

    // Fall back to simple balance when the monthly history returned nothing
    // (e.g. journal has no dated transactions yet).
    if results.is_empty() {
        let balances = load_liability_balances_eur(journal_path, liabilities_account)?;
        return Ok(balances
            .into_iter()
            .map(|b| LiabilityProgress {
                account: b.account,
                current_balance: b.amount,
                original_balance: b.amount,
                avg_monthly_payment: 0.0,
                pct_paid: 0.0,
                months_to_payoff: None,
            })
            .collect());
    }

    Ok(results)
}

pub fn load_crypto_balances(journal_path: &Path) -> Result<Vec<AccountBalance>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "balance",
        "--flat",
        "-O",
        "csv",
        "--no-total",
        "assets:crypto",
    ])?;
    parse_balance_csv(&text)
}

pub fn load_account_balances_eur(
    journal_path: &Path,
    assets_account: &str,
) -> Result<Vec<AccountBalance>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "balance",
        "--flat",
        "-O",
        "csv",
        "--no-total",
        "-V",
        assets_account,
    ])?;
    let result = parse_balance_csv(&text)?;
    // When the configured account prefix matches nothing (e.g. a benchmark
    // journal with non-standard account names), fall back to all accounts so
    // the UI shows something useful instead of a blank screen.
    if result.is_empty() {
        let text_all = run_hledger(&["-f", jp, "balance", "--flat", "-O", "csv", "--no-total"])?;
        return parse_balance_csv(&text_all);
    }
    Ok(result)
}

pub fn load_liability_balances_eur(
    journal_path: &Path,
    liabilities_account: &str,
) -> Result<Vec<AccountBalance>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "balance",
        "--flat",
        "-O",
        "csv",
        "--no-total",
        "-V",
        liabilities_account,
    ])?;
    parse_balance_csv(&text)
}

fn breakdown_category(account: &str, assets_root: &str) -> &'static str {
    let lower = account.to_lowercase();
    let root = assets_root.to_lowercase();
    if lower.starts_with(&format!("{root}:bank"))
        || lower.starts_with(&format!("{root}:cash"))
        || lower.starts_with(&format!("{root}:checking"))
    {
        "bank"
    } else if lower.starts_with(&format!("{root}:crypto")) {
        "crypto"
    } else {
        "investments"
    }
}

pub fn load_net_worth_breakdown(
    journal_path: &Path,
    period: &str,
    currency_symbol: &str,
    assets_account: &str,
) -> Result<NetWorthBreakdownSeries> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "balance",
        assets_account,
        "-H",
        "-p",
        period,
        "-O",
        "csv",
        "--layout",
        "bare",
        "-V",
        "--no-total",
        "--empty",
    ])?;

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
        .skip(1)
        .filter_map(|(i, h)| {
            NaiveDate::parse_from_str(&format!("{}-01", h.trim()), "%Y-%m-%d")
                .ok()
                .map(|d| (i, d))
        })
        .collect();

    if month_cols.is_empty() {
        return Ok(NetWorthBreakdownSeries::default());
    }

    let n = month_cols.len();
    let mut bank_sums = vec![0.0f64; n];
    let mut crypto_sums = vec![0.0f64; n];
    let mut invest_sums = vec![0.0f64; n];

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 2 {
            continue;
        }
        let account = fields.get(0).unwrap_or("").trim().trim_matches('"');
        let commodity = fields.get(1).unwrap_or("").trim().trim_matches('"');
        if !currency_matches(currency_symbol, commodity) {
            continue;
        }
        let category = breakdown_category(account, assets_account);
        for (idx, &(col, _)) in month_cols.iter().enumerate() {
            if col >= fields.len() {
                continue;
            }
            if let Some(amount) = parse_eu_number(&fields[col]) {
                match category {
                    "bank" => bank_sums[idx] += amount,
                    "crypto" => crypto_sums[idx] += amount,
                    _ => invest_sums[idx] += amount,
                }
            }
        }
    }

    let first_date = month_cols[0].1;
    let mut layer_investments = Vec::with_capacity(n);
    let mut layer_invest_crypto = Vec::with_capacity(n);
    let mut layer_total = Vec::with_capacity(n);
    let mut labels = Vec::with_capacity(n);

    for (i, &(_, date)) in month_cols.iter().enumerate() {
        let x = (date - first_date).num_days() as f64;
        let inv = invest_sums[i].max(0.0);
        let cry = crypto_sums[i].max(0.0);
        let bnk = bank_sums[i].max(0.0);
        layer_investments.push((x, inv));
        layer_invest_crypto.push((x, inv + cry));
        layer_total.push((x, inv + cry + bnk));
        labels.push((date, inv + cry + bnk));
    }

    Ok(NetWorthBreakdownSeries {
        layer_investments,
        layer_invest_crypto,
        layer_total,
        labels,
    })
}

/// Parse a `--layout bare` CSV from hledger into per-month sums, filtering
/// rows by `currency_symbol`.  Returns the month-column metadata and the sums
/// so the caller can decide whether to retry with different hledger args.
fn parse_nw_bare_csv(text: &str, currency_symbol: &str) -> Result<NetWorthPoints> {
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
        .skip(1)
        .filter_map(|(i, h)| {
            NaiveDate::parse_from_str(&format!("{}-01", h.trim()), "%Y-%m-%d")
                .ok()
                .map(|d| (i, d))
        })
        .collect();

    if month_cols.is_empty() {
        return Ok((vec![], vec![]));
    }

    let mut sums: Vec<f64> = vec![0.0; month_cols.len()];

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
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

    Ok((month_cols, sums))
}

pub fn load_net_worth_history(
    journal_path: &Path,
    period: &str,
    currency_symbol: &str,
    assets_account: &str,
    liabilities_account: &str,
) -> Result<NetWorthSeries> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let text = run_hledger(&[
        "-f",
        jp,
        "balance",
        assets_account,
        liabilities_account,
        "-H",
        "-p",
        period,
        "-O",
        "csv",
        "--layout",
        "bare",
        "-V",
        "--no-total",
        "--empty",
    ])?;

    let (month_cols, sums) = parse_nw_bare_csv(&text, currency_symbol)?;

    // When the account filters match nothing (e.g. a journal with non-standard
    // account names) or -V re-prices the display commodity away, all sums stay
    // zero.  Retry without account filters and without -V so raw amounts reach
    // the currency filter.
    let (month_cols, sums) = if month_cols.is_empty() || sums.iter().all(|&s| s == 0.0) {
        let text_all = run_hledger(&[
            "-f",
            jp,
            "balance",
            "-H",
            "-p",
            period,
            "-O",
            "csv",
            "--layout",
            "bare",
            "--no-total",
            "--empty",
        ])?;
        parse_nw_bare_csv(&text_all, currency_symbol)?
    } else {
        (month_cols, sums)
    };

    if month_cols.is_empty() {
        return Ok(NetWorthSeries::default());
    }

    let first_date = month_cols[0].1;
    let points: Vec<(f64, f64)> = month_cols
        .iter()
        .enumerate()
        .map(|(i, &(_, date))| {
            let x = (date - first_date).num_days() as f64;
            (x, sums[i])
        })
        .collect();

    let labels: Vec<(NaiveDate, f64)> = month_cols
        .iter()
        .enumerate()
        .map(|(i, &(_, date))| (date, sums[i]))
        .collect();

    Ok(NetWorthSeries { points, labels })
}
