use anyhow::Result;
use chrono::Datelike;
use std::collections::HashMap;
use std::path::Path;

use super::parse::{parse_balance_csv, parse_eu_number};
use super::run_hledger;

#[derive(Debug, Clone)]
pub struct PayeeSummary {
    pub name: String,
    /// YTD total spend with this payee.
    pub total: f64,
    /// Average monthly spend (total / months elapsed this year).
    pub monthly_avg: f64,
    /// Spend per month for the last 6 months (oldest → newest).
    pub sparkline: Vec<f64>,
}

/// Load payee analytics for `expenses_account` for the current year.
///
/// Makes two hledger calls:
/// 1. `balance --pivot payee … -p "this year"` → YTD totals per payee.
/// 2. `balance --pivot payee … -p "monthly last 6 months" --layout bare` →
///    per-month amounts for the sparkline.
///
/// Returns payees sorted by total descending. Empty on any hledger error so
/// the rest of the UI is unaffected.
pub fn load_payee_analytics(
    journal_path: &Path,
    expenses_account: &str,
    currency_symbol: &str,
) -> Result<Vec<PayeeSummary>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");

    let totals_text = run_hledger(&[
        "-f",
        jp,
        "balance",
        "--pivot",
        "payee",
        expenses_account,
        "-p",
        "this year",
        "-O",
        "csv",
        "--no-total",
    ])?;

    let totals = parse_balance_csv(&totals_text)?;
    if totals.is_empty() {
        return Ok(Vec::new());
    }

    let spark_text = run_hledger(&[
        "-f",
        jp,
        "balance",
        "--pivot",
        "payee",
        expenses_account,
        "-p",
        "monthly last 6 months",
        "-O",
        "csv",
        "--no-total",
        "--layout",
        "bare",
    ])?;

    let spark_map = parse_payee_monthly_bare_csv(&spark_text, currency_symbol);

    let months_elapsed = chrono::Local::now().date_naive().month() as f64;

    let mut result: Vec<PayeeSummary> = totals
        .into_iter()
        .filter(|b| b.amount.abs() > 0.01)
        .map(|b| {
            let sparkline = spark_map.get(&b.account).cloned().unwrap_or_default();
            let monthly_avg = b.amount / months_elapsed.max(1.0);
            PayeeSummary {
                name: b.account,
                total: b.amount,
                monthly_avg,
                sparkline,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        b.total
            .partial_cmp(&a.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(result)
}

/// Parse `hledger balance --pivot payee … --layout bare -O csv` output into a
/// map of payee → per-month amounts (oldest → newest).
fn parse_payee_monthly_bare_csv(text: &str, currency_symbol: &str) -> HashMap<String, Vec<f64>> {
    let mut map: HashMap<String, Vec<f64>> = HashMap::new();
    if text.trim().is_empty() {
        return map;
    }

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return map,
    };

    // Layout bare: "account","commodity","YYYY-MM","YYYY-MM",...
    let n_month_cols = headers.len().saturating_sub(2);
    if n_month_cols == 0 {
        return map;
    }

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 3 {
            continue;
        }
        let payee = fields
            .get(0)
            .unwrap_or("")
            .trim()
            .trim_matches('"')
            .to_string();
        if payee.is_empty() {
            continue;
        }

        // Skip rows whose commodity doesn't match (e.g. crypto side-effects).
        let commodity = fields.get(1).unwrap_or("").trim().trim_matches('"');
        if !super::currency_matches(currency_symbol, commodity) {
            continue;
        }

        let monthly: Vec<f64> = (2..fields.len())
            .map(|i| {
                parse_eu_number(fields.get(i).unwrap_or("0"))
                    .unwrap_or(0.0)
                    .abs()
            })
            .collect();

        let entry = map
            .entry(payee)
            .or_insert_with(|| vec![0.0; monthly.len()]);
        for (i, v) in monthly.iter().enumerate() {
            if i < entry.len() {
                entry[i] += v;
            } else {
                entry.push(*v);
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_payee_monthly_bare_csv_basic() {
        let csv = "account,commodity,2026-01,2026-02,2026-03\n\
                   Amazon,EUR,50,100,75\n\
                   Rewe,EUR,200,180,190\n";
        let map = parse_payee_monthly_bare_csv(csv, "€");
        let amazon = map.get("Amazon").unwrap();
        assert_eq!(amazon.len(), 3);
        assert!((amazon[0] - 50.0).abs() < 0.01);
        assert!((amazon[1] - 100.0).abs() < 0.01);
        assert!((amazon[2] - 75.0).abs() < 0.01);
    }

    #[test]
    fn parse_payee_monthly_bare_csv_skips_wrong_currency() {
        let csv = "account,commodity,2026-01\n\
                   SomePayee,USD,100\n\
                   OtherPayee,EUR,50\n";
        let map = parse_payee_monthly_bare_csv(csv, "€");
        assert!(!map.contains_key("SomePayee"), "USD row should be skipped");
        assert!(map.contains_key("OtherPayee"));
    }

    #[test]
    fn parse_payee_monthly_bare_csv_accumulates_same_payee() {
        // Two rows for the same payee (different sub-commodities) → summed.
        let csv = "account,commodity,2026-01\n\
                   Amazon,EUR,50\n\
                   Amazon,EUR,25\n";
        let map = parse_payee_monthly_bare_csv(csv, "EUR");
        let amazon = map.get("Amazon").unwrap();
        assert!((amazon[0] - 75.0).abs() < 0.01);
    }

    #[test]
    fn parse_payee_monthly_bare_csv_empty_input() {
        let map = parse_payee_monthly_bare_csv("", "€");
        assert!(map.is_empty());
    }
}
