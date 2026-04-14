use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct PriceEntry {
    pub date: NaiveDate,
    pub commodity: String,
    pub price_eur: f64,
}

#[derive(Debug, Clone)]
pub struct CryptoHolding {
    pub commodity: String,
    pub amount: f64,
    pub price_eur: f64,
    pub value_eur: f64,
}

#[derive(Debug, Clone)]
pub struct AccountBalance {
    pub account: String,
    pub amount: f64,
    pub commodity: String,
}

#[derive(Debug, Default, Clone)]
pub struct MonthlyData {
    pub month_name: String,
    pub income: Vec<(String, f64)>,
    pub expenses: Vec<(String, f64)>,
    pub total_income: f64,
    pub total_expenses: f64,
}

/// Parse a European-formatted number like "1.524,00" or "74,52" or "0,02352448"
pub fn parse_eu_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let negative = s.starts_with('-');
    let s = if negative { &s[1..] } else { s };

    let cleaned = if s.contains('.') && s.contains(',') {
        // e.g. "1.524,00" → dot is thousands sep, comma is decimal
        s.replace('.', "").replace(',', ".")
    } else if s.contains(',') {
        // e.g. "74,52"
        s.replace(',', ".")
    } else {
        s.to_string()
    };

    let val: f64 = cleaned.parse().ok()?;
    Some(if negative { -val } else { val })
}

/// Parse an amount string like "4,40140000 SOL" or "1430,15 €" into (amount, commodity)
fn parse_amount_str(s: &str) -> Option<(f64, String)> {
    let s = s.trim().trim_matches('"');
    if s == "0" || s.is_empty() {
        return None;
    }

    // Find last space separating number from commodity
    let pos = s.rfind(' ')?;
    let num_str = s[..pos].trim();
    let commodity = s[pos + 1..].trim().to_string();

    let amount = parse_eu_number(num_str)?;
    Some((amount, commodity))
}

pub fn load_price_history(journal_dir: &Path) -> Vec<PriceEntry> {
    let prices_file = journal_dir.join("prices.journal");
    let content = std::fs::read_to_string(&prices_file).unwrap_or_default();

    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with('P') {
            continue;
        }

        // Format: P DATE COMMODITY PRICE CURRENCY
        let parts: Vec<&str> = line.splitn(5, ' ').collect();
        if parts.len() < 4 {
            continue;
        }

        let date = match NaiveDate::parse_from_str(parts[1], "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let commodity = parts[2].to_string();
        let price = match parse_eu_number(parts[3]) {
            Some(p) => p,
            None => continue,
        };

        entries.push(PriceEntry {
            date,
            commodity,
            price_eur: price,
        });
    }

    entries.sort_by_key(|e| e.date);
    entries
}

pub fn latest_prices(price_history: &[PriceEntry]) -> HashMap<String, f64> {
    let mut latest: HashMap<String, f64> = HashMap::new();
    for entry in price_history {
        latest.insert(entry.commodity.clone(), entry.price_eur);
    }
    latest
}

pub fn load_crypto_balances(journal_path: &Path) -> Result<Vec<AccountBalance>> {
    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "balance",
            "--flat",
            "-O",
            "csv",
            "--no-total",
            "assets:crypto",
        ])
        .output()
        .context("Failed to run hledger")?;

    let text = String::from_utf8_lossy(&output.stdout);
    parse_balance_csv(&text)
}

pub fn load_account_balances_eur(journal_path: &Path) -> Result<Vec<AccountBalance>> {
    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "balance",
            "--flat",
            "-O",
            "csv",
            "--no-total",
            "-V",
            "assets",
        ])
        .output()
        .context("Failed to run hledger")?;

    let text = String::from_utf8_lossy(&output.stdout);
    parse_balance_csv(&text)
}

fn parse_balance_csv(text: &str) -> Result<Vec<AccountBalance>> {
    let mut result = Vec::new();

    for line in text.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Lines look like: "assets:crypto:exodus:SOL","4,40140000 SOL"
        // Split on the `","` separator
        if let Some(sep_pos) = line.find("\",\"") {
            let account = line[1..sep_pos].to_string(); // strip leading "
            let balance = &line[sep_pos + 3..line.len() - 1]; // strip trailing "

            if balance == "0" {
                continue;
            }

            if let Some((amount, commodity)) = parse_amount_str(balance) {
                if amount.abs() > 1e-10 {
                    result.push(AccountBalance {
                        account,
                        amount,
                        commodity,
                    });
                }
            }
        }
    }

    Ok(result)
}

pub fn load_monthly_data(journal_path: &Path) -> Result<MonthlyData> {
    let now = chrono::Local::now().date_naive();
    let current_month = now.month() as usize;

    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "incomestatement",
            "-O",
            "csv",
            "--no-total",
            "-p",
            "monthly this year",
        ])
        .output()
        .context("Failed to run hledger incomestatement")?;

    let text = String::from_utf8_lossy(&output.stdout);
    parse_monthly_csv(&text, current_month)
}

fn parse_monthly_csv(text: &str, current_month: usize) -> Result<MonthlyData> {
    // Columns: 0=Account, 1=Jan, 2=Feb, 3=Mar, ...
    // current_month is 1-based, so March (3) is at column index 3
    let col_idx = current_month;

    let mut data = MonthlyData {
        month_name: month_name(current_month).to_string(),
        ..Default::default()
    };

    let mut in_revenues = false;
    let mut in_expenses = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let fields = parse_csv_line(line);
        if fields.is_empty() {
            continue;
        }

        let account = fields[0].clone();

        match account.as_str() {
            a if a.starts_with("Monthly") => continue,
            "Account" => continue,
            "Revenues" | "Revenue" => {
                in_revenues = true;
                in_expenses = false;
                continue;
            }
            "Expenses" | "Expense" => {
                in_revenues = false;
                in_expenses = true;
                continue;
            }
            "Total" => continue,
            _ => {}
        }

        if col_idx < fields.len() {
            let val_str = &fields[col_idx];
            if let Some((amount, commodity)) = parse_amount_str(val_str) {
                if commodity == "€" && amount.abs() > 0.005 {
                    if in_revenues {
                        data.income.push((account, amount.abs()));
                        data.total_income += amount.abs();
                    } else if in_expenses {
                        data.expenses.push((account, amount.abs()));
                        data.total_expenses += amount.abs();
                    }
                }
            }
        }
    }

    data.income.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    data.expenses
        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    Ok(data)
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                fields.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    fields.push(current);
    fields
}

fn month_name(m: usize) -> &'static str {
    match m {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

pub fn compute_portfolio(
    crypto_balances: &[AccountBalance],
    latest_prices: &HashMap<String, f64>,
) -> Vec<CryptoHolding> {
    let mut totals: HashMap<String, f64> = HashMap::new();
    for balance in crypto_balances {
        *totals.entry(balance.commodity.clone()).or_default() += balance.amount;
    }

    let order = ["SOL", "BTC", "ETH", "LINK", "TON", "AR"];
    let mut holdings: Vec<CryptoHolding> = Vec::new();

    for coin in order.iter() {
        if let Some(&amount) = totals.get(*coin) {
            if amount <= 1e-8 {
                continue;
            }
            let price = latest_prices.get(*coin).copied().unwrap_or(0.0);
            holdings.push(CryptoHolding {
                commodity: coin.to_string(),
                amount,
                price_eur: price,
                value_eur: amount * price,
            });
        }
    }

    // Any other coins not in the predefined order
    for (coin, &amount) in &totals {
        if !order.contains(&coin.as_str()) && amount > 1e-8 {
            let price = latest_prices.get(coin).copied().unwrap_or(0.0);
            holdings.push(CryptoHolding {
                commodity: coin.clone(),
                amount,
                price_eur: price,
                value_eur: amount * price,
            });
        }
    }

    holdings
}

/// Three EUR-denominated time series for the portfolio analysis chart.
#[derive(Debug, Clone, Default)]
pub struct CoinChartSeries {
    /// Cumulative EUR invested (cost basis) over time.
    pub investment: Vec<(f64, f64)>,
    /// EUR gain/loss from price movement on purchased coins.
    pub price_growth: Vec<(f64, f64)>,
    /// EUR value of coins received via staking rewards.
    pub staking_growth: Vec<(f64, f64)>,
}

/// Run `hledger register <args…> -O csv` and return `(date, amount, commodity)`
/// for every posting row. The commodity is parsed from the amount field.
fn run_register(journal_path: &Path, args: &[&str]) -> Vec<(NaiveDate, f64, String)> {
    let output = Command::new("hledger")
        .arg("-f")
        .arg(journal_path.to_str().unwrap_or("all.journal"))
        .arg("register")
        .args(args)
        .arg("-O")
        .arg("csv")
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let mut result = Vec::new();

    // CSV columns: txnidx, date, code, description, account, amount, total
    for line in text.lines().skip(1) {
        let fields = parse_csv_line(line);
        if fields.len() < 6 {
            continue;
        }

        let date = match NaiveDate::parse_from_str(fields[1].trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        let amount_str = fields[5].trim().trim_matches('"');
        if amount_str.is_empty() || amount_str == "0" {
            continue;
        }

        if let Some((amount, commodity)) = parse_amount_str(amount_str) {
            result.push((date, amount, commodity));
        }
    }

    result
}

/// Build the three portfolio analysis series for one coin, aligned to the
/// dates present in `price_history`.
///
/// How the three queries work together:
///
/// 1. `assets:crypto cur:SOL --cost`:  every SOL posting in the crypto asset
///    accounts, with `@ price` amounts converted to their EUR cost. Only the
///    EUR-denominated rows represent real purchases; SOL-denominated rows are
///    transfers/staking movements with no cost annotation and are ignored.
///
/// 2. `assets:crypto cur:SOL`:  all SOL balance changes in asset accounts.
///    Running sum = total SOL held at any date (purchases + staking + transfers,
///    transfers net to zero so they cancel out).
///
/// 3. `income cur:SOL`:  queries the income accounts directly (not the asset
///    accounts), so the AND semantics of hledger queries work in our favour.
///    These amounts are negative (income credits); negating gives the SOL
///    earned through staking.
pub fn load_coin_chart_series(
    journal_path: &Path,
    price_history: &[PriceEntry],
    coin: &str,
) -> CoinChartSeries {
    let coin_prices: Vec<&PriceEntry> =
        price_history.iter().filter(|e| e.commodity == coin).collect();

    if coin_prices.len() < 2 {
        return CoinChartSeries::default();
    }

    let first_date = coin_prices[0].date;
    let coin_filter = format!("cur:{coin}");

    // EUR cost of purchases: SOL postings converted via @ annotation.
    // Only rows whose commodity is EUR came from a real buy.
    let cost_entries = run_register(journal_path, &["assets:crypto", &coin_filter, "--cost"]);

    // All SOL movements in asset accounts.
    let sol_entries = run_register(journal_path, &["assets:crypto", &coin_filter]);

    // SOL credited to income accounts as staking rewards (negative amounts).
    // We query the income account directly so the hledger AND query works.
    let staking_entries = run_register(journal_path, &["income", &coin_filter]);

    let eur_commodities: &[&str] = &["€", "EUR", "eur"];

    let mut investment = Vec::with_capacity(coin_prices.len());
    let mut price_growth = Vec::with_capacity(coin_prices.len());
    let mut staking_growth = Vec::with_capacity(coin_prices.len());

    for entry in &coin_prices {
        let date = entry.date;
        let price = entry.price_eur;
        let days = (date - first_date).num_days() as f64;

        // Cumulative EUR spent on purchases up to this date.
        let cost: f64 = cost_entries
            .iter()
            .filter(|(d, _, com)| *d <= date && eur_commodities.contains(&com.as_str()))
            .map(|(_, amt, _)| amt)
            .sum();

        // Total SOL in all asset accounts at this date.
        let total_sol: f64 = sol_entries
            .iter()
            .filter(|(d, _, _)| *d <= date)
            .map(|(_, amt, _)| amt)
            .sum();

        // SOL earned via staking (income credits are negative, so we negate).
        let staked_sol: f64 = staking_entries
            .iter()
            .filter(|(d, _, _)| *d <= date)
            .map(|(_, amt, _)| -amt)
            .sum::<f64>()
            .max(0.0);

        // Purchased SOL = total minus staked; transfers between wallets net to
        // zero so they don't affect this figure (only their fees do).
        let bought_sol = (total_sol - staked_sol).max(0.0);

        investment.push((days, cost));
        price_growth.push((days, bought_sol * price - cost));
        staking_growth.push((days, staked_sol * price));
    }

    CoinChartSeries { investment, price_growth, staking_growth }
}
