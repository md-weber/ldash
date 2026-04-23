use anyhow::Result;
use chrono::NaiveDate;
use std::path::Path;

use super::parse::parse_amount_str;
use super::{run_hledger, Transaction};

pub fn load_recent_transactions(
    journal_path: &Path,
    account: &str,
    n: usize,
    period: Option<&str>,
    currency_symbol: &str,
) -> Result<Vec<Transaction>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let mut args = vec!["-f", jp, "register", account, "-X", currency_symbol, "-O", "csv"];
    if let Some(p) = period {
        args.push("-p");
        args.push(p);
    }
    let text = run_hledger(&args)?;
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    let mut txns = Vec::new();
    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 7 {
            continue;
        }

        let date = match NaiveDate::parse_from_str(fields[1].trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let description = fields[3].trim().to_string();
        let amount = parse_amount_str(fields[5].trim())
            .map(|(a, _)| a)
            .unwrap_or(0.0);
        let running_total = parse_amount_str(fields[6].trim())
            .map(|(a, _)| a)
            .unwrap_or(0.0);

        txns.push(Transaction {
            date,
            description,
            amount,
            running_total,
        });
    }

    if txns.len() > n {
        txns = txns.split_off(txns.len() - n);
    }

    Ok(txns)
}

pub fn search_transactions(journal_path: &Path, query: &str) -> Result<Vec<Transaction>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let query_arg = format!("desc:{}", query);
    let text = run_hledger(&["-f", jp, "register", "-O", "csv", &query_arg])?;
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    let mut txns = Vec::new();
    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 7 {
            continue;
        }

        let date = match NaiveDate::parse_from_str(fields[1].trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let account = fields[4].trim().to_string();
        let description = fields[3].trim().to_string();
        let amount = parse_amount_str(fields[5].trim())
            .map(|(a, _)| a)
            .unwrap_or(0.0);
        let running_total = parse_amount_str(fields[6].trim())
            .map(|(a, _)| a)
            .unwrap_or(0.0);

        txns.push(Transaction {
            date,
            description: format!("{} ({})", description, account),
            amount,
            running_total,
        });
    }

    if txns.len() > 100 {
        txns = txns.split_off(txns.len() - 100);
    }

    Ok(txns)
}
