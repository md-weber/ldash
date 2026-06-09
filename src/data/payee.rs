use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;
use std::path::Path;

use super::parse::parse_amount_str;
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

/// Load payee analytics for all expense accounts.
///
/// Uses `hledger register type:x` (expense account type) so it works with any
/// journal regardless of naming convention — matching how `incomestatement`
/// auto-detects expenses without needing an explicit account name.
///
/// Makes two calls:
/// 1. `register type:x -p "this year"` → aggregate by description for YTD totals.
/// 2. `register type:x -p "last 6 months"` → aggregate by (description, month)
///    for sparklines.
///
/// Returns payees sorted by total descending.
pub fn load_payee_analytics(
    journal_path: &Path,
    _expenses_account: &str,
    _currency_symbol: &str,
) -> Result<Vec<PayeeSummary>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");

    // One call covers both YTD totals and monthly sparklines (last 6 buckets).
    let ytd_text = run_hledger(&["-f", jp, "register", "type:x", "-p", "this year", "-O", "csv"])?;
    let ytd_map = aggregate_register_by_description(&ytd_text);

    if ytd_map.is_empty() {
        return Ok(Vec::new());
    }

    let spark_map = aggregate_register_by_description_monthly(&ytd_text);

    let months_elapsed = chrono::Local::now().date_naive().month() as f64;

    let mut result: Vec<PayeeSummary> = ytd_map
        .into_iter()
        .filter(|(_, total)| total.abs() > 0.01)
        .map(|(name, total)| {
            let sparkline = spark_map.get(&name).cloned().unwrap_or_default();
            PayeeSummary {
                monthly_avg: total / months_elapsed.max(1.0),
                sparkline,
                name,
                total,
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

/// Parse `hledger register -O csv` and return total absolute amount per description.
/// Uses fields[3]=description, fields[5]=amount as established by the rest of the codebase.
fn aggregate_register_by_description(text: &str) -> HashMap<String, f64> {
    let mut map: HashMap<String, f64> = HashMap::new();
    if text.trim().is_empty() {
        return map;
    }
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());
    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 6 {
            continue;
        }
        let desc = fields[3].trim().to_string();
        if desc.is_empty() {
            continue;
        }
        let amount = parse_amount_str(fields[5].trim())
            .map(|(a, _)| a.abs())
            .unwrap_or(0.0);
        *map.entry(desc).or_insert(0.0) += amount;
    }
    map
}

/// Parse `hledger register -O csv` and return per-description sparkline data.
/// Each entry is a Vec of monthly totals (oldest → newest, up to 6 buckets).
fn aggregate_register_by_description_monthly(text: &str) -> HashMap<String, Vec<f64>> {
    if text.trim().is_empty() {
        return HashMap::new();
    }
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    // First pass: collect (description, year_month, amount).
    let mut rows: Vec<(String, (i32, u32), f64)> = Vec::new();
    let mut months_seen: Vec<(i32, u32)> = Vec::new();

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 6 {
            continue;
        }
        let date = match NaiveDate::parse_from_str(fields[1].trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let desc = fields[3].trim().to_string();
        if desc.is_empty() {
            continue;
        }
        let amount = parse_amount_str(fields[5].trim())
            .map(|(a, _)| a.abs())
            .unwrap_or(0.0);
        let ym = (date.year(), date.month());
        if !months_seen.contains(&ym) {
            months_seen.push(ym);
        }
        rows.push((desc, ym, amount));
    }

    months_seen.sort_unstable();
    // Keep only the last 6 months.
    if months_seen.len() > 6 {
        months_seen = months_seen[months_seen.len() - 6..].to_vec();
    }

    let mut map: HashMap<String, Vec<f64>> = HashMap::new();
    for (desc, ym, amount) in rows {
        if let Some(col) = months_seen.iter().position(|&m| m == ym) {
            let entry = map
                .entry(desc)
                .or_insert_with(|| vec![0.0; months_seen.len()]);
            if col < entry.len() {
                entry[col] += amount;
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    // register CSV cols (matching codebase convention):
    // txnidx,date,X,description,account,amount,total
    fn reg_row(date: &str, desc: &str, amount: &str) -> String {
        format!("1,{date},,{desc},expenses:food,{amount},0")
    }

    #[test]
    fn aggregate_register_by_description_basic() {
        let header = "txnidx,date,X,description,account,amount,total\n";
        let csv = format!(
            "{header}{}\n{}\n{}\n",
            reg_row("2026-01-05", "Amazon", "-50 EUR"),
            reg_row("2026-02-10", "Amazon", "-100 EUR"),
            reg_row("2026-01-20", "Rewe", "-200 EUR"),
        );
        let map = aggregate_register_by_description(&csv);
        assert!((map["Amazon"] - 150.0).abs() < 0.01);
        assert!((map["Rewe"] - 200.0).abs() < 0.01);
    }

    #[test]
    fn aggregate_register_by_description_eu_amounts() {
        // Exact format produced by hledger 1.52 with European locale amounts.
        let csv = "\"txnidx\",\"date\",\"code\",\"description\",\"account\",\"amount\",\"total\"\n\
                   \"673\",\"2026-01-01\",\"\",\"FAMILIENHOTEL |\",\"expenses:urlaub\",\"4529,00 €\",\"4529,00 €\"\n\
                   \"674\",\"2026-01-02\",\"\",\"AXA | Kfz\",\"expenses:transport\",\"362,88 €\",\"4891,88 €\"\n\
                   \"675\",\"2026-01-02\",\"\",\"FAMILIENHOTEL |\",\"expenses:urlaub\",\"100,00 €\",\"4991,88 €\"\n";
        let map = aggregate_register_by_description(csv);
        assert!(!map.is_empty(), "map must not be empty for EU-format amounts");
        let hotel = map.get("FAMILIENHOTEL |").expect("FAMILIENHOTEL should be present");
        assert!((hotel - 4629.0).abs() < 0.01, "FAMILIENHOTEL total: {hotel}");
        let axa = map.get("AXA | Kfz").expect("AXA should be present");
        assert!((axa - 362.88).abs() < 0.01, "AXA total: {axa}");
    }

    #[test]
    fn aggregate_register_by_description_empty() {
        let map = aggregate_register_by_description("");
        assert!(map.is_empty());
    }

    #[test]
    fn aggregate_register_monthly_buckets() {
        let header = "txnidx,date,X,description,account,amount,total\n";
        let csv = format!(
            "{header}{}\n{}\n{}\n",
            reg_row("2026-01-05", "Amazon", "-50 EUR"),
            reg_row("2026-02-10", "Amazon", "-100 EUR"),
            reg_row("2026-01-20", "Amazon", "-25 EUR"),
        );
        let map = aggregate_register_by_description_monthly(&csv);
        let spark = map.get("Amazon").unwrap();
        // Jan bucket = 50+25 = 75, Feb bucket = 100
        assert_eq!(spark.len(), 2);
        assert!((spark[0] - 75.0).abs() < 0.01, "Jan: {}", spark[0]);
        assert!((spark[1] - 100.0).abs() < 0.01, "Feb: {}", spark[1]);
    }

    #[test]
    fn aggregate_register_monthly_empty() {
        let map = aggregate_register_by_description_monthly("");
        assert!(map.is_empty());
    }
}
