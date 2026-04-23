use anyhow::Result;
use chrono::NaiveDate;
use std::collections::{HashMap, VecDeque};
use std::path::Path;

use super::parse::parse_amount_str;
use super::{run_hledger, AccountBalance, CoinChartSeries, CryptoHolding, PriceEntry};

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
            CryptoHolding {
                commodity: coin,
                amount,
                price_eur: price,
                value_eur: amount * price,
            }
        })
        .collect();

    holdings.sort_by(|a, b| {
        b.value_eur
            .partial_cmp(&a.value_eur)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    holdings
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

fn run_register_full(journal_path: &Path, args: &[&str]) -> Result<Vec<RegisterEntry>> {
    let jp = journal_path.to_str().unwrap_or("all.journal");
    let mut all_args = vec!["-f", jp, "register"];
    all_args.extend_from_slice(args);
    all_args.extend_from_slice(&["-O", "csv"]);
    let text = run_hledger(&all_args)?;

    let mut result = Vec::new();
    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

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
            result.push(RegisterEntry {
                txnidx,
                date,
                amount,
                commodity,
                account,
            });
        }
    }

    Ok(result)
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
) -> Result<HashMap<String, CoinChartSeries>> {
    let (cost_res, asset_res, income_res) = std::thread::scope(|s| {
        let t_cost = s.spawn(|| run_register_full(journal_path, &["assets:crypto", "--cost"]));
        let t_asset = s.spawn(|| run_register_full(journal_path, &["assets:crypto"]));
        let t_income = s.spawn(|| run_register_full(journal_path, &["income"]));
        // Downgrade panics into `Err` so a crashing register call can't take
        // the whole refresh down with it.
        let join = |h: std::thread::ScopedJoinHandle<'_, Result<Vec<RegisterEntry>>>| -> Result<Vec<RegisterEntry>> {
            h.join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("register worker panicked")))
        };
        (join(t_cost), join(t_asset), join(t_income))
    });
    let cost_entries = cost_res?;
    let asset_entries = asset_res?;
    let income_entries = income_res?;

    // Fiat-leg detection: the configured display currency plus the common
    // EUR/USD aliases hledger journals use ("EUR", "USD", "$"). Anything else
    // is treated as a coin commodity for FIFO basis calculations.
    let fiat_commodities: Vec<&str> = ["€", "EUR", "eur", "$", "USD", "usd", currency_symbol]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let coin_set: std::collections::HashSet<&str> = coins.iter().map(|s| s.as_str()).collect();

    // Account → coin mapping from asset entries (commodity IS the coin)
    let mut account_to_coin: HashMap<&str, &str> = HashMap::new();
    for e in &asset_entries {
        if !fiat_commodities.contains(&e.commodity.as_str())
            && coin_set.contains(e.commodity.as_str())
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
                *txn_eur_net
                    .entry(coin)
                    .or_default()
                    .entry(e.txnidx)
                    .or_insert(0.0) += e.amount;
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
                let cost_per_unit = if net_eur > 0.0 {
                    net_eur / net_coin
                } else {
                    0.0
                };
                lots.push_back(Lot {
                    amount: net_coin,
                    cost_per_unit,
                });
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
        let coin_prices: Vec<&PriceEntry> = price_history
            .iter()
            .filter(|e| e.commodity == *coin)
            .collect();

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
        let basis_snaps = basis_timeline
            .get(coin.as_str())
            .cloned()
            .unwrap_or_default();
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
            sample_day(
                d,
                &mut investment,
                &mut price_growth,
                &mut staking_growth,
                &mut price_series,
            );
        }

        result.insert(
            coin.clone(),
            CoinChartSeries {
                investment,
                price_growth,
                staking_growth,
                price: price_series,
            },
        );
    }

    Ok(result)
}
