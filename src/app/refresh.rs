use chrono::Local;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

use crate::config::Config;
use crate::data::{
    compute_portfolio, latest_prices, load_account_balances_eur, load_all_coin_chart_series,
    load_crypto_balances, load_last_year_monthly, load_liability_balances_eur, load_monthly_data,
    load_monthly_with_forecast, load_net_worth_breakdown, load_net_worth_history,
    load_price_history, AccountBalance, CoinChartSeries, CryptoHolding, MonthlyData,
    NetWorthBreakdownSeries, NetWorthSeries,
};

use super::{App, RefreshResult, Tab, TabData};

pub(super) fn load_all_data(
    journal_path: &Path,
    journal_dir: &Path,
    nw_period: &str,
    currency_symbol: &str,
    tabs: [bool; 3],
) -> RefreshResult {
    let price_history = load_price_history(journal_dir);
    let lp = latest_prices(&price_history);

    let want_portfolio = tabs[0];
    let want_accounts = tabs[1];
    let want_monthly = tabs[2];

    let (crypto_res, accounts_res, liab_res, nw_res, breakdown_res, monthly_res, ly_res, forecast_res) =
        std::thread::scope(|s| {
            let t_crypto = want_portfolio.then(|| s.spawn(|| load_crypto_balances(journal_path)));
            let t_accounts =
                want_accounts.then(|| s.spawn(|| load_account_balances_eur(journal_path)));
            let t_liabilities =
                want_accounts.then(|| s.spawn(|| load_liability_balances_eur(journal_path)));
            let t_nw = want_accounts.then(|| {
                s.spawn(|| load_net_worth_history(journal_path, nw_period, currency_symbol))
            });
            let t_breakdown = want_accounts.then(|| {
                s.spawn(|| load_net_worth_breakdown(journal_path, nw_period, currency_symbol))
            });
            let t_monthly =
                want_monthly.then(|| s.spawn(|| load_monthly_data(journal_path, currency_symbol)));
            let t_ly = want_monthly
                .then(|| s.spawn(|| load_last_year_monthly(journal_path, currency_symbol)));
            let t_forecast = want_monthly
                .then(|| s.spawn(|| load_monthly_with_forecast(journal_path, currency_symbol)));
            (
                t_crypto.map(|t| t.join().unwrap()),
                t_accounts.map(|t| t.join().unwrap()),
                t_liabilities.map(|t| t.join().unwrap()),
                t_nw.map(|t| t.join().unwrap()),
                t_breakdown.map(|t| t.join().unwrap()),
                t_monthly.map(|t| t.join().unwrap()),
                t_ly.map(|t| t.join().unwrap()),
                t_forecast.map(|t| t.join().unwrap()),
            )
        });

    let holdings: TabData<Vec<CryptoHolding>> = match crypto_res {
        None => TabData::NotRequested,
        Some(Ok(balances)) => TabData::Ok(compute_portfolio(&balances, &lp)),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let coin_chart_cache: TabData<HashMap<String, CoinChartSeries>> = if let TabData::Ok(ref h) =
        holdings
    {
        let coins: Vec<String> = h.iter().map(|holding| holding.commodity.clone()).collect();
        match load_all_coin_chart_series(journal_path, &price_history, &coins, currency_symbol) {
            Ok(cache) => TabData::Ok(cache),
            Err(e) => TabData::Err(e.to_string()),
        }
    } else {
        TabData::NotRequested
    };

    let account_balances: TabData<Vec<AccountBalance>> = match accounts_res {
        None => TabData::NotRequested,
        Some(Ok(mut b)) => {
            b.sort_by(|a, b| {
                b.amount
                    .partial_cmp(&a.amount)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            TabData::Ok(b)
        }
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let liabilities: TabData<Vec<AccountBalance>> = match liab_res {
        None => TabData::NotRequested,
        Some(Ok(l)) => TabData::Ok(l),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let net_worth_history: TabData<NetWorthSeries> = match nw_res {
        None => TabData::NotRequested,
        Some(Ok(s)) => TabData::Ok(s),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let net_worth_breakdown: TabData<NetWorthBreakdownSeries> = match breakdown_res {
        None => TabData::NotRequested,
        Some(Ok(s)) => TabData::Ok(s),
        Some(Err(_)) => TabData::NotRequested,
    };

    let monthly: TabData<MonthlyData> = match monthly_res {
        None => TabData::NotRequested,
        Some(Ok(m)) => TabData::Ok(m),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let last_year: TabData<MonthlyData> = match ly_res {
        None => TabData::NotRequested,
        Some(Ok(ly)) => TabData::Ok(ly),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let monthly_forecast: TabData<MonthlyData> = match forecast_res {
        None => TabData::NotRequested,
        Some(Ok(f)) => TabData::Ok(f),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    RefreshResult {
        tabs,
        price_history,
        latest_prices: lp,
        holdings,
        coin_chart_cache,
        account_balances,
        liabilities,
        net_worth_history,
        net_worth_breakdown,
        monthly,
        last_year,
        monthly_forecast,
    }
}

impl App {
    pub fn start_refresh(&mut self) {
        self.tabs_loaded = [false; 3];
        self.start_refresh_tabs([true; 3]);
    }

    pub fn ensure_tab_loaded(&mut self, tab: Tab) {
        if self.tabs_loaded[tab.index()] || self.refresh_rx.is_some() {
            return;
        }
        let mut tabs = [false; 3];
        tabs[tab.index()] = true;
        self.start_refresh_tabs(tabs);
    }

    fn start_refresh_tabs(&mut self, mut tabs: [bool; 3]) {
        if !self.crypto_enabled() {
            tabs[Tab::Portfolio.index()] = false;
        }
        if self.refresh_rx.is_some() {
            return;
        }
        self.loading = true;
        self.status_msg = "Refreshing…".to_string();

        let jp = self.journal_path.clone();
        let jd = self.journal_dir.clone();
        let nw_period = self.nw_range.period_arg().to_string();
        let currency = self.config.currency_symbol.clone();

        let (tx, rx) = mpsc::channel();
        self.refresh_rx = Some(rx);

        std::thread::spawn(move || {
            let result = load_all_data(&jp, &jd, &nw_period, &currency, tabs);
            let _ = tx.send(result);
        });
    }

    pub fn check_refresh(&mut self) -> bool {
        let result = match self.refresh_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            Some(r) => r,
            None => return false,
        };
        self.refresh_rx = None;
        self.apply_refresh(result);
        true
    }

    pub(super) fn apply_refresh(&mut self, r: RefreshResult) {
        self.price_history = r.price_history;
        self.latest_prices = r.latest_prices;

        let mut errors: Vec<String> = Vec::new();

        match r.holdings {
            TabData::Ok(h) => {
                let detected = !h.is_empty();
                self.holdings = h;
                if detected != self.has_crypto {
                    self.has_crypto = detected;
                    self.fix_tab_after_visibility_change();
                }
            }
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.coin_chart_cache {
            TabData::Ok(c) => self.coin_chart_cache = c,
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.account_balances {
            TabData::Ok(a) => self.account_balances = a,
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.liabilities {
            TabData::Ok(l) => self.liabilities = l,
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.net_worth_history {
            TabData::Ok(nw) => self.net_worth_history = nw,
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.net_worth_breakdown {
            TabData::Ok(bd) => self.net_worth_breakdown = bd,
            TabData::Err(_) | TabData::NotRequested => {}
        }
        match r.monthly {
            TabData::Ok(m) => self.monthly = m,
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.last_year {
            TabData::Ok(ly) => self.last_year = ly,
            TabData::Err(msg) => errors.push(msg),
            TabData::NotRequested => {}
        }
        match r.monthly_forecast {
            TabData::Ok(f) => self.monthly_forecast = f,
            // Forecast errors are non-fatal: the chart just won't show a
            // projected line (user may not have periodic transaction rules).
            TabData::Err(_) | TabData::NotRequested => {}
        }
        self.rebuild_combined_months();

        if !self.holdings.is_empty() && self.selected_holding >= self.holdings.len() {
            self.selected_holding = self.holdings.len() - 1;
        }

        for (i, loaded) in r.tabs.iter().enumerate() {
            if *loaded {
                self.tabs_loaded[i] = true;
            }
        }

        self.last_refresh = Instant::now();
        self.last_journal_mtime = std::fs::metadata(&self.journal_path)
            .and_then(|m| m.modified())
            .ok();
        if errors.is_empty() {
            let now = Local::now().format("%H:%M:%S");
            self.status_msg = format!("Updated at {now}");
        } else {
            self.status_msg = errors.last().unwrap().clone();
        }
        self.loading = false;

        if self.price_alerts.is_empty() && !self.alert_dismissed {
            self.compute_price_alerts();
        }
    }

    fn fix_tab_after_visibility_change(&mut self) {
        let visible = self.visible_tabs();
        if !visible.contains(&self.tab) {
            self.tab = visible[0];
            self.tabs_loaded[Tab::Portfolio.index()] = false;
        }
    }

    fn journal_exists(&self) -> bool {
        self.journal_path.exists()
    }

    pub fn auto_refresh(&mut self) {
        self.check_config_reload();

        if !self.journal_exists() {
            if !self.status_msg.starts_with("Journal not found") {
                self.status_msg = format!(
                    "Journal not found: {} — waiting for re-creation",
                    self.journal_path.display()
                );
            }
            return;
        }

        let watcher_changed = self
            .watcher
            .as_ref()
            .map(|w| w.has_changes())
            .unwrap_or(false);

        if watcher_changed {
            self.start_refresh();
            return;
        }

        let current_mtime = std::fs::metadata(&self.journal_path)
            .and_then(|m| m.modified())
            .ok();
        if current_mtime == self.last_journal_mtime {
            self.last_refresh = Instant::now();
            return;
        }
        self.start_refresh();
    }

    fn check_config_reload(&mut self) {
        let current = Config::config_mtime();
        if current == self.last_config_mtime {
            return;
        }
        self.last_config_mtime = current;
        if let Some(err) = self.config.hot_reload() {
            self.status_msg = err;
        } else {
            let now = Local::now().format("%H:%M:%S");
            self.status_msg = format!("Config reloaded at {now}");
        }
    }
}
