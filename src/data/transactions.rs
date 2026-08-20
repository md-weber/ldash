use anyhow::Result;
use chrono::NaiveDate;
use std::collections::VecDeque;
use std::path::Path;

use super::parse::parse_amount_str;
use super::{run_hledger, Transaction};

/// Push `txn` into a bounded ring buffer keeping only the last `cap` entries.
/// Used by the loaders below to avoid materialising the full register history
/// when only the tail is needed.
fn push_capped(buf: &mut VecDeque<Transaction>, cap: usize, txn: Transaction) {
    if cap == 0 {
        return;
    }
    if buf.len() == cap {
        buf.pop_front();
    }
    buf.push_back(txn);
}

pub fn load_recent_transactions(
    journal_path: &Path,
    account: &str,
    n: usize,
    period: Option<&str>,
    currency_symbol: &str,
) -> Result<Vec<Transaction>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let mut args = vec![
        "-f",
        jp,
        "register",
        account,
        "-X",
        currency_symbol,
        "-O",
        "csv",
    ];
    if let Some(p) = period {
        args.push("-p");
        args.push(p);
    }
    let text = run_hledger(&args)?;
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    let mut txns: VecDeque<Transaction> = VecDeque::with_capacity(n);
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

        push_capped(
            &mut txns,
            n,
            Transaction {
                date,
                description,
                amount,
                running_total,
            },
        );
    }

    Ok(txns.into_iter().collect())
}
