use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::Path;

use super::parse::{parse_balance_csv, parse_eu_number};
use super::{
    currency_matches, run_hledger, AccountBalance, NetWorthBreakdownSeries, NetWorthSeries,
};

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
    parse_balance_csv(&text)
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
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr
        .headers()
        .context("No CSV headers from hledger balance")?
        .clone();
    // --layout bare produces: "account", "commodity", "2024-01", "2024-02", …
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
        return Ok(NetWorthSeries::default());
    }

    let mut sums: Vec<f64> = vec![0.0; month_cols.len()];

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        // Column 1 is the commodity in bare layout — only sum rows in the
        // configured display currency.
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
