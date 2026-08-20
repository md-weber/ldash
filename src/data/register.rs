use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use std::path::Path;

use super::parse::parse_amount_str;
use super::run_hledger;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterQuery {
    pub year: i32,
    pub month: u8, // 1..=12
    pub account: Option<String>,
    pub description: Option<String>,
}

impl RegisterQuery {
    pub fn current_month() -> Self {
        let today = Local::now().date_naive();
        Self {
            year: today.year(),
            month: today.month() as u8,
            account: None,
            description: None,
        }
    }

    pub fn period_arg(&self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }

    pub fn prev_month(&mut self) {
        if self.month <= 1 {
            self.year -= 1;
            self.month = 12;
        } else {
            self.month -= 1;
        }
    }

    pub fn next_month(&mut self) {
        if self.month >= 12 {
            self.year += 1;
            self.month = 1;
        } else {
            self.month += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnStatus {
    Unmarked,
    Pending,
    Cleared,
}

impl TxnStatus {
    pub fn as_cell(self) -> &'static str {
        match self {
            Self::Cleared => "*",
            Self::Pending => "!",
            Self::Unmarked => " ",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterPosting {
    pub txnidx: u32,
    pub date: NaiveDate,
    pub status: TxnStatus,
    pub description: String,
    pub account: String,
    pub amount: f64,
    pub running_total: f64,
}

#[derive(Debug, Clone)]
pub struct RegisterTxn {
    pub txnidx: u32,
    pub date: NaiveDate,
    pub status: TxnStatus,
    pub description: String,
    pub postings: Vec<RegisterPosting>,
}

/// One table row on the Register tab.
///
/// Unfiltered months emit one row per posting (`continuation` blanks the
/// date/status/description on later legs). An account filter collapses each
/// matching transaction to a single row.
#[derive(Debug, Clone)]
pub struct RegisterViewRow {
    pub txnidx: u32,
    pub date: NaiveDate,
    pub status: TxnStatus,
    pub description: String,
    pub account: String,
    pub amount: f64,
    pub running_total: f64,
    pub continuation: bool,
}

pub fn load_register_page(
    journal_path: &Path,
    query: &RegisterQuery,
    currency_symbol: &str,
) -> Result<Vec<RegisterTxn>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let period = query.period_arg();
    // Account and description are applied in memory so every loaded
    // transaction still has all of its legs for drill-down.
    let args = [
        "-f",
        jp,
        "register",
        "-O",
        "csv",
        "-X",
        currency_symbol,
        "-p",
        period.as_str(),
    ];
    let text = run_hledger(&args)?;
    Ok(group_register_txns(&parse_register_csv(&text)))
}

fn parse_txn_status(date: &str, code: &str) -> TxnStatus {
    if date.contains('*') || code.contains('*') {
        TxnStatus::Cleared
    } else if date.contains('!') || code.contains('!') {
        TxnStatus::Pending
    } else {
        TxnStatus::Unmarked
    }
}

fn parse_csv_date(raw: &str) -> Option<NaiveDate> {
    // hledger may put `*` / `!` in the date column; take YYYY-MM-DD only.
    let trimmed = raw.trim();
    let prefix = trimmed.get(..10)?;
    NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()
}

fn parse_register_csv(text: &str) -> Vec<RegisterPosting> {
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for record in rdr.records() {
        let fields = match record {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 7 {
            continue;
        }
        let txnidx = fields[0].trim().parse().unwrap_or(0);
        let date_raw = fields[1].trim();
        let date = match parse_csv_date(date_raw) {
            Some(d) => d,
            None => continue,
        };
        let code = fields[2].trim();
        rows.push(RegisterPosting {
            txnidx,
            date,
            status: parse_txn_status(date_raw, code),
            description: fields[3].trim().to_string(),
            account: fields[4].trim().to_string(),
            amount: parse_amount_str(fields[5].trim())
                .map(|(a, _)| a)
                .unwrap_or(0.0),
            running_total: parse_amount_str(fields[6].trim())
                .map(|(a, _)| a)
                .unwrap_or(0.0),
        });
    }
    rows
}

pub fn group_register_txns(postings: &[RegisterPosting]) -> Vec<RegisterTxn> {
    let mut txns: Vec<RegisterTxn> = Vec::new();
    for p in postings {
        if let Some(last) = txns.last_mut() {
            if last.txnidx == p.txnidx {
                last.postings.push(p.clone());
                continue;
            }
        }
        txns.push(RegisterTxn {
            txnidx: p.txnidx,
            date: p.date,
            status: p.status,
            description: p.description.clone(),
            postings: vec![p.clone()],
        });
    }
    txns
}

fn account_matches(account: &str, filter: &str) -> bool {
    let a = account.to_ascii_lowercase();
    let f = filter.to_ascii_lowercase();
    a == f || a.starts_with(&format!("{f}:"))
}

fn description_matches(description: &str, filter: &str) -> bool {
    description
        .to_ascii_lowercase()
        .contains(&filter.to_ascii_lowercase())
}

pub fn build_register_view(
    txns: &[RegisterTxn],
    account: Option<&str>,
    description: Option<&str>,
) -> Vec<RegisterViewRow> {
    let desc = description
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let acct = account
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    match acct {
        Some(filter) => collapsed_account_rows(txns, &filter, desc.as_deref()),
        None => grouped_posting_rows(txns, desc.as_deref()),
    }
}

fn grouped_posting_rows(txns: &[RegisterTxn], desc: Option<&str>) -> Vec<RegisterViewRow> {
    let mut rows = Vec::new();
    for txn in txns {
        if let Some(d) = desc {
            if !description_matches(&txn.description, d) {
                continue;
            }
        }
        for (i, p) in txn.postings.iter().enumerate() {
            let continuation = i > 0;
            rows.push(RegisterViewRow {
                txnidx: txn.txnidx,
                date: txn.date,
                status: txn.status,
                description: if continuation {
                    String::new()
                } else {
                    txn.description.clone()
                },
                account: p.account.clone(),
                amount: p.amount,
                running_total: p.running_total,
                continuation,
            });
        }
    }
    rows
}

fn collapsed_account_rows(
    txns: &[RegisterTxn],
    filter: &str,
    desc: Option<&str>,
) -> Vec<RegisterViewRow> {
    let mut rows = Vec::new();
    let mut running = 0.0;
    for txn in txns {
        if let Some(d) = desc {
            if !description_matches(&txn.description, d) {
                continue;
            }
        }
        let matching: Vec<&RegisterPosting> = txn
            .postings
            .iter()
            .filter(|p| account_matches(&p.account, filter))
            .collect();
        if matching.is_empty() {
            continue;
        }
        let amount: f64 = matching.iter().map(|p| p.amount).sum();
        running += amount;
        let account = if matching.len() == 1 {
            matching[0].account.clone()
        } else {
            format!("{} ({} legs)", matching[0].account, matching.len())
        };
        rows.push(RegisterViewRow {
            txnidx: txn.txnidx,
            date: txn.date,
            status: txn.status,
            description: txn.description.clone(),
            account,
            amount,
            running_total: running,
            continuation: false,
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_arg_zero_pads_month() {
        let q = RegisterQuery {
            year: 2026,
            month: 8,
            account: None,
            description: None,
        };
        assert_eq!(q.period_arg(), "2026-08");
    }

    #[test]
    fn period_arg_keeps_four_digit_year() {
        let q = RegisterQuery {
            year: 1999,
            month: 12,
            account: None,
            description: None,
        };
        assert_eq!(q.period_arg(), "1999-12");
    }

    #[test]
    fn status_cleared_from_date() {
        assert_eq!(parse_txn_status("2026-01-20 *", ""), TxnStatus::Cleared);
    }

    #[test]
    fn status_cleared_from_code() {
        assert_eq!(parse_txn_status("2026-01-20", "*"), TxnStatus::Cleared);
    }

    #[test]
    fn status_pending_from_date() {
        assert_eq!(parse_txn_status("2026-01-20 !", ""), TxnStatus::Pending);
    }

    #[test]
    fn status_pending_from_code() {
        assert_eq!(parse_txn_status("2026-01-20", "!"), TxnStatus::Pending);
    }

    #[test]
    fn status_unmarked_when_neither_marker() {
        assert_eq!(parse_txn_status("2026-01-20", ""), TxnStatus::Unmarked);
    }

    #[test]
    fn status_star_wins_over_bang() {
        assert_eq!(parse_txn_status("2026-01-20 *", "!"), TxnStatus::Cleared);
    }

    const FIXTURE_CSV: &str = "\
txnidx,date,code,description,account,amount,total
1,2026-01-15,,Salary,income:salary,-2000.00 EUR,-2000.00 EUR
1,2026-01-15,,Salary,assets:bank:checking,2000.00 EUR,0
2,2026-01-20 *,,Groceries,expenses:food,150.00 EUR,150.00 EUR
2,2026-01-20 *,,Groceries,assets:bank:checking,-150.00 EUR,0
3,2026-01-22,!,Rent,expenses:housing,800.00 EUR,950.00 EUR
bad-row
4,not-a-date,,Skip,assets:bank,1.00 EUR,1.00 EUR
";

    fn fixture_txns() -> Vec<RegisterTxn> {
        group_register_txns(&parse_register_csv(FIXTURE_CSV))
    }

    #[test]
    fn parse_csv_fixture_rows_and_status() {
        let rows = parse_register_csv(FIXTURE_CSV);
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[0].txnidx, 1);
        assert_eq!(rows[0].date, NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(rows[0].status, TxnStatus::Unmarked);
        assert_eq!(rows[0].description, "Salary");
        assert_eq!(rows[0].account, "income:salary");
        assert!((rows[0].amount - (-2000.0)).abs() < 1e-9);

        assert_eq!(rows[2].txnidx, 2);
        assert_eq!(rows[2].status, TxnStatus::Cleared);
        assert_eq!(rows[2].description, "Groceries");
        assert!((rows[2].amount - 150.0).abs() < 1e-9);

        assert_eq!(rows[4].status, TxnStatus::Pending);
        assert_eq!(rows[4].description, "Rent");
    }

    #[test]
    fn group_register_txns_joins_legs() {
        let txns = fixture_txns();
        assert_eq!(txns.len(), 3);
        assert_eq!(txns[0].description, "Salary");
        assert_eq!(txns[0].postings.len(), 2);
        assert_eq!(txns[1].postings.len(), 2);
        assert_eq!(txns[2].postings.len(), 1);
    }

    #[test]
    fn unfiltered_view_blanks_continuation_legs() {
        let rows = build_register_view(&fixture_txns(), None, None);
        assert_eq!(rows.len(), 5);
        assert!(!rows[0].continuation);
        assert_eq!(rows[0].description, "Salary");
        assert!(rows[1].continuation);
        assert!(rows[1].description.is_empty());
        assert_eq!(rows[1].account, "assets:bank:checking");
        assert!(!rows[2].continuation);
        assert_eq!(rows[2].description, "Groceries");
        assert!(rows[3].continuation);
    }

    #[test]
    fn account_filter_collapses_to_one_row_per_txn() {
        let rows = build_register_view(&fixture_txns(), Some("assets:bank:checking"), None);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "Salary");
        assert!((rows[0].amount - 2000.0).abs() < 1e-9);
        assert!((rows[0].running_total - 2000.0).abs() < 1e-9);
        assert_eq!(rows[1].description, "Groceries");
        assert!((rows[1].amount - (-150.0)).abs() < 1e-9);
        assert!((rows[1].running_total - 1850.0).abs() < 1e-9);
        assert!(!rows.iter().any(|r| r.continuation));
    }

    #[test]
    fn account_filter_matches_children() {
        let rows = build_register_view(&fixture_txns(), Some("assets:bank"), None);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].account, "assets:bank:checking");
    }

    #[test]
    fn account_filter_does_not_match_sibling_prefix() {
        let rows = build_register_view(&fixture_txns(), Some("assets:ban"), None);
        assert!(rows.is_empty());
    }

    #[test]
    fn description_filter_keeps_all_legs() {
        let rows = build_register_view(&fixture_txns(), None, Some("groc"));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].description, "Groceries");
        assert!(rows[1].continuation);
    }

    #[test]
    fn month_nav_rolls_year_without_invalid_months() {
        let mut q = RegisterQuery {
            year: 2026,
            month: 1,
            account: None,
            description: None,
        };
        q.prev_month();
        assert_eq!(q.year, 2025);
        assert_eq!(q.month, 12);
        q.next_month();
        assert_eq!(q.year, 2026);
        assert_eq!(q.month, 1);

        q.month = 12;
        q.next_month();
        assert_eq!(q.year, 2027);
        assert_eq!(q.month, 1);
    }
}
