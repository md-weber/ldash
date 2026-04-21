use anyhow::{Context, Result};
use chrono::{Datelike, NaiveDate};
use std::collections::{HashMap, VecDeque};
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
pub struct SingleMonth {
    pub month_name: String,
    pub income: Vec<(String, f64)>,
    pub expenses: Vec<(String, f64)>,
    pub total_income: f64,
    pub total_expenses: f64,
}

#[derive(Debug, Default, Clone)]
pub struct MonthlyData {
    pub months: Vec<SingleMonth>,
    pub selected: usize,
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

pub fn load_liability_balances_eur(journal_path: &Path) -> Result<Vec<AccountBalance>> {
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
            "liabilities",
        ])
        .output()
        .context("Failed to run hledger for liabilities")?;

    let text = String::from_utf8_lossy(&output.stdout);
    parse_balance_csv(&text)
}

fn parse_balance_csv(text: &str) -> Result<Vec<AccountBalance>> {
    let mut result = Vec::new();
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    for row in rdr.records() {
        let row = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if row.len() < 2 {
            continue;
        }
        let account = row[0].to_string();
        let balance = row[1].trim();
        if balance == "0" || balance.is_empty() {
            continue;
        }
        if let Some((amount, commodity)) = parse_amount_str(balance) {
            if amount.abs() > 1e-10 {
                result.push(AccountBalance { account, amount, commodity });
            }
        }
    }

    Ok(result)
}

pub fn load_monthly_data(journal_path: &Path, currency_symbol: &str) -> Result<MonthlyData> {
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
    parse_monthly_csv(&text, current_month, currency_symbol)
}

pub fn load_monthly_for_period(
    journal_path: &Path,
    period: &str,
    currency_symbol: &str,
) -> Result<MonthlyData> {
    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "incomestatement",
            "-O",
            "csv",
            "--no-total",
            "-p",
            period,
        ])
        .output()
        .context("Failed to run hledger incomestatement")?;

    let text = String::from_utf8_lossy(&output.stdout);
    parse_monthly_csv(&text, 0, currency_symbol)
}

pub fn load_last_year_monthly(
    journal_path: &Path,
    currency_symbol: &str,
) -> Result<MonthlyData> {
    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "incomestatement",
            "-O",
            "csv",
            "--no-total",
            "-p",
            "monthly last year",
        ])
        .output()
        .context("Failed to run hledger incomestatement for last year")?;

    let text = String::from_utf8_lossy(&output.stdout);
    parse_monthly_csv(&text, 0, currency_symbol)
}

fn parse_monthly_csv(
    text: &str,
    current_month: usize,
    currency_symbol: &str,
) -> Result<MonthlyData> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let num_cols = rdr.headers().map(|h| h.len()).unwrap_or(2);

    let mut months_data: Vec<SingleMonth> = (1..num_cols)
        .map(|m| SingleMonth {
            month_name: month_name(m).to_string(),
            ..Default::default()
        })
        .collect();

    let mut in_revenues = false;
    let mut in_expenses = false;

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.is_empty() {
            continue;
        }

        let account = fields[0].to_string();

        match account.as_str() {
            a if a.starts_with("Monthly") => continue,
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

        for col_idx in 1..num_cols {
            if col_idx >= fields.len() {
                continue;
            }
            let val_str = &fields[col_idx];
            if let Some((amount, commodity)) = parse_amount_str(val_str) {
                if commodity == currency_symbol && amount.abs() > 0.005 {
                    let month = &mut months_data[col_idx - 1];
                    if in_revenues {
                        month.income.push((account.clone(), amount.abs()));
                        month.total_income += amount.abs();
                    } else if in_expenses {
                        month.expenses.push((account.clone(), amount.abs()));
                        month.total_expenses += amount.abs();
                    }
                }
            }
        }
    }

    let months: Vec<SingleMonth> = months_data
        .into_iter()
        .filter(|m| m.total_income > 0.0 || m.total_expenses > 0.0)
        .map(|mut m| {
            m.income.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            m.expenses.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            m
        })
        .collect();

    let selected = months
        .iter()
        .position(|m| m.month_name == month_name(current_month))
        .unwrap_or(months.len().saturating_sub(1));

    Ok(MonthlyData { months, selected })
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

#[derive(Debug, Clone, Default)]
pub struct NetWorthSeries {
    pub points: Vec<(f64, f64)>,
    pub labels: Vec<(NaiveDate, f64)>,
}

pub fn load_net_worth_history(
    journal_path: &Path,
    period: &str,
    currency_symbol: &str,
) -> Result<NetWorthSeries> {
    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "balance",
            "assets",
            "liabilities",
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
        ])
        .output()
        .context("Failed to run hledger balance for net worth history")?;

    let text = String::from_utf8_lossy(&output.stdout);
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let headers = rdr.headers().context("No CSV headers")?.clone();
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
        if commodity != currency_symbol {
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

#[derive(Debug, Clone)]
pub struct Transaction {
    pub date: NaiveDate,
    pub description: String,
    pub amount: f64,
    pub running_total: f64,
}

pub fn load_recent_transactions(
    journal_path: &Path,
    account: &str,
    n: usize,
    period: Option<&str>,
) -> Result<Vec<Transaction>> {
    let mut cmd = Command::new("hledger");
    cmd.args([
        "-f",
        journal_path.to_str().unwrap_or("all.journal"),
        "register",
        account,
        "-O",
        "csv",
    ]);
    if let Some(p) = period {
        cmd.args(["-p", p]);
    }
    let output = cmd.output().context("Failed to run hledger register")?;

    let text = String::from_utf8_lossy(&output.stdout);
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

        txns.push(Transaction { date, description, amount, running_total });
    }

    if txns.len() > n {
        txns = txns.split_off(txns.len() - n);
    }

    Ok(txns)
}

pub fn search_transactions(journal_path: &Path, query: &str) -> Result<Vec<Transaction>> {
    let output = Command::new("hledger")
        .args([
            "-f",
            journal_path.to_str().unwrap_or("all.journal"),
            "register",
            "-O",
            "csv",
            &format!("desc:{}", query),
        ])
        .output()
        .context("Failed to run hledger register for search")?;

    let text = String::from_utf8_lossy(&output.stdout);
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

pub fn compute_portfolio(
    crypto_balances: &[AccountBalance],
    latest_prices: &HashMap<String, f64>,
) -> Vec<CryptoHolding> {
    let mut totals: HashMap<String, f64> = HashMap::new();
    for balance in crypto_balances {
        *totals.entry(balance.commodity.clone()).or_default() += balance.amount;
    }

    let mut holdings: Vec<CryptoHolding> = totals
        .into_iter()
        .filter(|(_, amount)| *amount > 1e-8)
        .map(|(coin, amount)| {
            let price = latest_prices.get(&coin).copied().unwrap_or(0.0);
            CryptoHolding { commodity: coin, amount, price_eur: price, value_eur: amount * price }
        })
        .collect();

    holdings.sort_by(|a, b| b.value_eur.partial_cmp(&a.value_eur).unwrap_or(std::cmp::Ordering::Equal));
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
    /// EUR price per coin at each sample point.
    pub price: Vec<(f64, f64)>,
}

impl CoinChartSeries {
    pub fn total_invested(&self) -> f64 {
        self.investment.last().map(|p| p.1).unwrap_or(0.0)
    }
}

#[derive(Clone, Copy)]
struct Lot {
    amount: f64,
    cost_per_unit: f64,
}

struct RegisterEntry {
    txnidx: u64,
    date: NaiveDate,
    amount: f64,
    commodity: String,
    account: String,
}

fn run_register_full(journal_path: &Path, args: &[&str]) -> Vec<RegisterEntry> {
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

    let mut result = Vec::new();
    let mut rdr = csv::ReaderBuilder::new().from_reader(output.stdout.as_slice());

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.len() < 6 {
            continue;
        }

        let txnidx: u64 = fields[0].trim().parse().unwrap_or(0);
        let date = match NaiveDate::parse_from_str(fields[1].trim(), "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        let account = fields[4].trim().to_string();
        let amount_str = fields[5].trim();
        if amount_str.is_empty() || amount_str == "0" {
            continue;
        }

        if let Some((amount, commodity)) = parse_amount_str(amount_str) {
            result.push(RegisterEntry { txnidx, date, amount, commodity, account });
        }
    }

    result
}

/// Build chart series for all coins in 3 bulk hledger calls instead of 3×N.
///
/// Three queries (run in parallel):
/// 1. `register assets:crypto --cost` — all crypto postings with cost
///    conversion. EUR rows = purchase cost basis. Account field maps to coin.
/// 2. `register assets:crypto` — all crypto movements. Commodity = coin.
/// 3. `register income` — all income. Non-EUR commodity rows = staking rewards.
pub fn load_all_coin_chart_series(
    journal_path: &Path,
    price_history: &[PriceEntry],
    coins: &[String],
    currency_symbol: &str,
) -> HashMap<String, CoinChartSeries> {
    let (cost_entries, asset_entries, income_entries) = std::thread::scope(|s| {
        let t_cost = s.spawn(|| run_register_full(journal_path, &["assets:crypto", "--cost"]));
        let t_asset = s.spawn(|| run_register_full(journal_path, &["assets:crypto"]));
        let t_income = s.spawn(|| run_register_full(journal_path, &["income"]));
        (
            t_cost.join().unwrap(),
            t_asset.join().unwrap(),
            t_income.join().unwrap(),
        )
    });

    // Fiat-leg detection: the configured display currency plus the common
    // EUR/USD aliases hledger journals use ("EUR", "USD", "$"). Anything else
    // is treated as a coin commodity for FIFO basis calculations.
    let fiat_commodities: Vec<&str> =
        ["€", "EUR", "eur", "$", "USD", "usd", currency_symbol]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect();
    let coin_set: std::collections::HashSet<&str> = coins.iter().map(|s| s.as_str()).collect();

    // Account → coin mapping from asset entries (commodity IS the coin)
    let mut account_to_coin: HashMap<&str, &str> = HashMap::new();
    for e in &asset_entries {
        if !fiat_commodities.contains(&e.commodity.as_str()) && coin_set.contains(e.commodity.as_str())
        {
            account_to_coin.entry(&e.account).or_insert(&e.commodity);
        }
    }

    // FIFO cost basis: build per-txn (coin, net coin amount, net EUR cost).
    // Net across all crypto-account postings cancels inter-account transfers
    // and pure staking moves (no EUR leg). Sells produce negative net coin
    // and (typically) positive EUR proceeds — we discard proceeds and only
    // pop lots from the FIFO queue on sells.
    //
    // Per-txn coin net amount, per coin
    let mut txn_coin_net: HashMap<&str, HashMap<u64, (NaiveDate, f64)>> = HashMap::new();
    for e in &asset_entries {
        if coin_set.contains(e.commodity.as_str()) {
            let entry = txn_coin_net
                .entry(e.commodity.as_str())
                .or_default()
                .entry(e.txnidx)
                .or_insert((e.date, 0.0));
            entry.1 += e.amount;
        }
    }
    // Per-txn EUR net (only crypto-account EUR rows in --cost output)
    let mut txn_eur_net: HashMap<&str, HashMap<u64, f64>> = HashMap::new();
    for e in &cost_entries {
        if fiat_commodities.contains(&e.commodity.as_str()) {
            if let Some(&coin) = account_to_coin.get(e.account.as_str()) {
                *txn_eur_net.entry(coin).or_default().entry(e.txnidx).or_insert(0.0) += e.amount;
            }
        }
    }

    // Group asset movements by coin (native commodity only) — used for total
    // coin held over time
    let mut asset_by_coin: HashMap<&str, Vec<(NaiveDate, f64)>> = HashMap::new();
    for e in &asset_entries {
        if coin_set.contains(e.commodity.as_str()) {
            asset_by_coin
                .entry(&e.commodity)
                .or_default()
                .push((e.date, e.amount));
        }
    }

    // Group staking income by coin (non-EUR commodity)
    let mut income_by_coin: HashMap<&str, Vec<(NaiveDate, f64)>> = HashMap::new();
    for e in &income_entries {
        if coin_set.contains(e.commodity.as_str()) {
            income_by_coin
                .entry(&e.commodity)
                .or_default()
                .push((e.date, e.amount));
        }
    }

    // Build chronological FIFO lot timeline per coin: walk events in order,
    // maintain VecDeque of remaining lots, snapshot remaining cost basis at
    // each event for time-series lookup.
    let mut basis_timeline: HashMap<String, Vec<(NaiveDate, f64)>> = HashMap::new();
    for coin in coins {
        let coin_txns = txn_coin_net.get(coin.as_str());
        let mut events: Vec<(NaiveDate, u64, f64, f64)> = Vec::new();
        if let Some(map) = coin_txns {
            for (&tid, &(date, net_coin)) in map {
                if net_coin.abs() < 1e-12 {
                    continue;
                }
                let net_eur = txn_eur_net
                    .get(coin.as_str())
                    .and_then(|m| m.get(&tid))
                    .copied()
                    .unwrap_or(0.0);
                events.push((date, tid, net_coin, net_eur));
            }
        }
        events.sort_by_key(|e| (e.0, e.1));

        let mut lots: VecDeque<Lot> = VecDeque::new();
        let mut snapshots: Vec<(NaiveDate, f64)> = Vec::new();
        for (date, _, net_coin, net_eur) in events {
            if net_coin > 0.0 {
                // Buy / inflow. cost_per_unit = max(net_eur, 0) / net_coin.
                // Pure inflow with no EUR (staking, gifts) → 0 cost basis.
                let cost_per_unit = if net_eur > 0.0 { net_eur / net_coin } else { 0.0 };
                lots.push_back(Lot { amount: net_coin, cost_per_unit });
            } else {
                let mut to_consume = -net_coin;
                while to_consume > 1e-12 {
                    match lots.front_mut() {
                        Some(lot) => {
                            if lot.amount <= to_consume + 1e-12 {
                                to_consume -= lot.amount;
                                lots.pop_front();
                            } else {
                                lot.amount -= to_consume;
                                to_consume = 0.0;
                            }
                        }
                        None => break,
                    }
                }
            }
            let basis: f64 = lots.iter().map(|l| l.amount * l.cost_per_unit).sum();
            snapshots.push((date, basis));
        }
        basis_timeline.insert(coin.clone(), snapshots);
    }

    let today = chrono::Local::now().date_naive();

    let mut result = HashMap::new();
    for coin in coins {
        let coin_prices: Vec<&PriceEntry> =
            price_history.iter().filter(|e| e.commodity == *coin).collect();

        if coin_prices.len() < 2 {
            result.insert(coin.clone(), CoinChartSeries::default());
            continue;
        }

        let first_date = coin_prices[0].date;
        let last_price_date = coin_prices.last().unwrap().date;
        let end_date = today.max(last_price_date);
        let total_days = (end_date - first_date).num_days();

        let assets = asset_by_coin.get(coin.as_str());
        let staking = income_by_coin.get(coin.as_str());
        let basis_snaps = basis_timeline.get(coin.as_str()).cloned().unwrap_or_default();
        // Cost basis on a given date = last snapshot at or before that date.
        let basis_at = |date: NaiveDate| -> f64 {
            match basis_snaps.binary_search_by_key(&date, |s| s.0) {
                Ok(i) => basis_snaps[i].1,
                Err(0) => 0.0,
                Err(i) => basis_snaps[i - 1].1,
            }
        };

        // Build a price lookup: linearly interpolate between known price
        // entries so that sparse prices.journal data doesn't create flat
        // stretches that break sub-range P/L calculations.
        let mut price_by_day: Vec<f64> = Vec::with_capacity(total_days as usize + 1);
        let mut price_idx = 0;
        for d in 0..=total_days {
            let date = first_date + chrono::Duration::days(d);
            while price_idx < coin_prices.len() && coin_prices[price_idx].date <= date {
                price_idx += 1;
            }
            let price = if price_idx == 0 {
                coin_prices[0].price_eur
            } else if price_idx >= coin_prices.len() {
                coin_prices.last().unwrap().price_eur
            } else {
                let prev = &coin_prices[price_idx - 1];
                let next = &coin_prices[price_idx];
                let span = (next.date - prev.date).num_days() as f64;
                if span <= 0.0 {
                    next.price_eur
                } else {
                    let t = (date - prev.date).num_days() as f64 / span;
                    prev.price_eur + t * (next.price_eur - prev.price_eur)
                }
            };
            price_by_day.push(price);
        }

        // Sample every few days to keep chart data manageable
        let step = (total_days as usize / 500).max(1);
        let mut investment = Vec::new();
        let mut price_growth = Vec::new();
        let mut staking_growth = Vec::new();
        let mut price_series = Vec::new();

        let sample_day = |d: i64,
                          investment: &mut Vec<(f64, f64)>,
                          price_growth: &mut Vec<(f64, f64)>,
                          staking_growth: &mut Vec<(f64, f64)>,
                          price_series: &mut Vec<(f64, f64)>| {
            let date = first_date + chrono::Duration::days(d);
            let price = price_by_day[d as usize];
            let days = d as f64;

            let total_coin: f64 = assets
                .map(|v| v.iter().filter(|(dd, _)| *dd <= date).map(|(_, a)| a).sum())
                .unwrap_or(0.0);

            let staked_coin: f64 = staking
                .map(|v| {
                    v.iter()
                        .filter(|(dd, _)| *dd <= date)
                        .map(|(_, a)| -a)
                        .sum::<f64>()
                })
                .unwrap_or(0.0)
                .max(0.0);

            let bought_coin = (total_coin - staked_coin).max(0.0);
            let cost = basis_at(date);

            investment.push((days, cost));
            price_growth.push((days, bought_coin * price - cost));
            staking_growth.push((days, staked_coin * price));
            price_series.push((days, price));
        };

        let mandatory_days: Vec<i64> = basis_snaps
            .iter()
            .map(|(d, _)| (*d - first_date).num_days())
            .filter(|&d| d >= 0 && d <= total_days)
            .collect();

        let mut sample_days: Vec<i64> = Vec::new();
        let mut d = 0i64;
        while d <= total_days {
            sample_days.push(d);
            d += step as i64;
        }
        for md in &mandatory_days {
            sample_days.push(*md);
        }
        sample_days.sort();
        sample_days.dedup();
        if sample_days.last().copied() != Some(total_days) {
            sample_days.push(total_days);
        }

        for d in sample_days {
            sample_day(d, &mut investment, &mut price_growth, &mut staking_growth, &mut price_series);
        }

        result.insert(
            coin.clone(),
            CoinChartSeries { investment, price_growth, staking_growth, price: price_series },
        );
    }

    result
}
