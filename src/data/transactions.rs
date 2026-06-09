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

const SEARCH_RESULTS_CAP: usize = 100;

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
                account: None,
            },
        );
    }

    Ok(txns.into_iter().collect())
}

/// Ranks an account for search-result display: lower = more interesting.
/// Prefer expenses/income over assets/liabilities so drilling down lands on
/// the meaningful side of a transaction rather than the balancing entry.
fn account_rank(account: &str) -> u8 {
    let a = account.to_ascii_lowercase();
    if a.starts_with("expenses:") || a.starts_with("income:") || a.starts_with("revenues:") {
        0
    } else if a.starts_with("liabilities:") {
        1
    } else if a.starts_with("assets:") {
        2
    } else {
        3
    }
}

pub fn search_transactions(journal_path: &Path, query: &str) -> Result<Vec<Transaction>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let query_arg = format!("desc:{}", query);
    let text = run_hledger(&["-f", jp, "register", "-O", "csv", &query_arg])?;
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    // Collect all postings, then pick the best account per transaction.
    // key = txnidx, value = (insertion_order, Transaction, rank)
    let mut by_txn: std::collections::HashMap<String, (usize, Transaction, u8)> =
        std::collections::HashMap::new();
    let mut order: usize = 0;

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 7 {
            continue;
        }

        let txnidx = fields[0].trim().to_string();
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
        let rank = account_rank(&account);

        let txn = Transaction {
            date,
            description: format!("{} ({})", description, account),
            amount,
            running_total,
            account: Some(account),
        };

        by_txn
            .entry(txnidx)
            .and_modify(|e| {
                // Replace if this posting has a better (lower) rank.
                if rank < e.2 {
                    e.1 = txn.clone();
                    e.2 = rank;
                }
            })
            .or_insert_with(|| {
                let idx = order;
                order += 1;
                (idx, txn, rank)
            });
    }

    // Re-sort by original insertion order so results stay chronological.
    let mut entries: Vec<(usize, Transaction)> = by_txn
        .into_values()
        .map(|(ord, txn, _)| (ord, txn))
        .collect();
    entries.sort_unstable_by_key(|(ord, _)| *ord);

    let mut txns: VecDeque<Transaction> = VecDeque::with_capacity(SEARCH_RESULTS_CAP);
    for (_, txn) in entries {
        push_capped(&mut txns, SEARCH_RESULTS_CAP, txn);
    }

    Ok(txns.into_iter().collect())
}
