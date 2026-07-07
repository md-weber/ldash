use chrono::Local;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

use crate::config::Config;
use crate::data::{
    compute_portfolio, fetch_and_append_prices, latest_prices, load_account_balances_eur,
    load_all_coin_chart_series, load_crypto_balances, load_last_year_monthly,
    load_liability_balances_eur, load_liability_progress, load_liquid_cash_monthly,
    load_monthly_data, load_monthly_with_forecast, load_net_worth_breakdown,
    load_net_worth_history, load_payee_analytics, load_price_history, today_prices_present,
    AccountBalance, CoinChartSeries, CryptoHolding, LiabilityProgress, MonthlyData,
    NetWorthBreakdownSeries, NetWorthSeries, PayeeSummary,
};

use super::{App, RefreshResult, Tab, TabData, TabFlags};

/// Config-derived strings forwarded to every loader thread.
/// Bundled to keep `load_all_data` and `spawn_all` below the 7-argument limit
/// and to make extending the parameter set (e.g. date-range filter) cheap.
pub(super) struct LoadConfig {
    pub(super) nw_period: String,
    pub(super) currency_symbol: String,
    pub(super) assets_account: String,
    pub(super) liabilities_account: String,
    pub(super) expenses_account: String,
    pub(super) liquid_accounts: Vec<String>,
}

/// Joined results of every background loader thread. Each field is `None` when
/// the corresponding tab was not requested, otherwise it carries the loader's
/// `Result`. Splitting this out keeps `load_all_data` readable and makes it
/// trivial to add a new data source (one field + one spawn line + one mapping).
struct LoadHandles {
    crypto: Option<Result<Vec<AccountBalance>, anyhow::Error>>,
    accounts: Option<Result<Vec<AccountBalance>, anyhow::Error>>,
    liabilities: Option<Result<Vec<AccountBalance>, anyhow::Error>>,
    liability_progress: Option<Result<Vec<LiabilityProgress>, anyhow::Error>>,
    net_worth: Option<Result<NetWorthSeries, anyhow::Error>>,
    breakdown: Option<Result<NetWorthBreakdownSeries, anyhow::Error>>,
    monthly: Option<Result<MonthlyData, anyhow::Error>>,
    last_year: Option<Result<MonthlyData, anyhow::Error>>,
    forecast: Option<Result<MonthlyData, anyhow::Error>>,
    payee: Option<Result<Vec<PayeeSummary>, anyhow::Error>>,
    liquid_cash_monthly: Option<Result<Vec<(String, f64)>, anyhow::Error>>,
}

fn spawn_all(journal_path: &Path, cfg: &LoadConfig, tabs: TabFlags) -> LoadHandles {
    let want_portfolio = tabs.portfolio;
    let want_accounts = tabs.accounts;
    let want_monthly = tabs.monthly;

    let nw_period = cfg.nw_period.as_str();
    let currency = cfg.currency_symbol.as_str();
    let assets = cfg.assets_account.as_str();
    let liabilities = cfg.liabilities_account.as_str();
    let expenses = cfg.expenses_account.as_str();
    let liquid_accounts = cfg.liquid_accounts.as_slice();

    std::thread::scope(|s| {
        let t_crypto = want_portfolio.then(|| s.spawn(|| load_crypto_balances(journal_path)));
        let t_accounts =
            want_accounts.then(|| s.spawn(|| load_account_balances_eur(journal_path, assets)));
        let t_liab = want_accounts
            .then(|| s.spawn(|| load_liability_balances_eur(journal_path, liabilities)));
        let t_liab_progress = want_accounts
            .then(|| s.spawn(|| load_liability_progress(journal_path, liabilities, currency)));
        let t_nw = want_accounts.then(|| {
            s.spawn(|| {
                load_net_worth_history(journal_path, nw_period, currency, assets, liabilities)
            })
        });
        let t_bd = want_accounts.then(|| {
            s.spawn(|| load_net_worth_breakdown(journal_path, nw_period, currency, assets))
        });
        let t_monthly = want_monthly.then(|| s.spawn(|| load_monthly_data(journal_path, currency)));
        let t_ly = want_monthly.then(|| s.spawn(|| load_last_year_monthly(journal_path, currency)));
        let t_fc =
            want_monthly.then(|| s.spawn(|| load_monthly_with_forecast(journal_path, currency)));
        let t_payee = want_monthly
            .then(|| s.spawn(|| load_payee_analytics(journal_path, expenses, currency)));
        let t_liquid = want_monthly
            .then(|| s.spawn(|| load_liquid_cash_monthly(journal_path, liquid_accounts, currency)));

        LoadHandles {
            crypto: t_crypto.map(join_or_panic_err),
            accounts: t_accounts.map(join_or_panic_err),
            liabilities: t_liab.map(join_or_panic_err),
            liability_progress: t_liab_progress.map(join_or_panic_err),
            net_worth: t_nw.map(join_or_panic_err),
            breakdown: t_bd.map(join_or_panic_err),
            monthly: t_monthly.map(join_or_panic_err),
            last_year: t_ly.map(join_or_panic_err),
            forecast: t_fc.map(join_or_panic_err),
            payee: t_payee.map(join_or_panic_err),
            liquid_cash_monthly: t_liquid.map(join_or_panic_err),
        }
    })
}

/// Join a worker thread, downgrading a panic into a regular `Err` so that one
/// crashing loader can't take the whole TUI down. The error message is
/// intentionally generic — the panic message has already been printed by the
/// default panic hook.
fn join_or_panic_err<T>(
    handle: std::thread::ScopedJoinHandle<'_, Result<T, anyhow::Error>>,
) -> Result<T, anyhow::Error> {
    handle
        .join()
        .unwrap_or_else(|_| Err(anyhow::anyhow!("worker thread panicked")))
}

/// Map an `Option<Result<T, E>>` (joined loader output) into `TabData<T>`.
fn into_tab_data<T, E: std::fmt::Display>(opt: Option<Result<T, E>>) -> TabData<T> {
    match opt {
        None => TabData::NotRequested,
        Some(Ok(v)) => TabData::Ok(v),
        Some(Err(e)) => TabData::Err(e.to_string()),
    }
}

pub(super) fn load_all_data(
    journal_path: &Path,
    journal_dir: &Path,
    cfg: LoadConfig,
    tabs: TabFlags,
) -> RefreshResult {
    let price_history = load_price_history(journal_dir);
    let lp = latest_prices(&price_history);

    let h = spawn_all(journal_path, &cfg, tabs);

    let holdings: TabData<Vec<CryptoHolding>> = match h.crypto {
        None => TabData::NotRequested,
        Some(Ok(balances)) => TabData::Ok(compute_portfolio(&balances, &lp)),
        Some(Err(e)) => TabData::Err(e.to_string()),
    };

    let coin_chart_cache: TabData<HashMap<String, CoinChartSeries>> = if let TabData::Ok(ref hs) =
        holdings
    {
        let coins: Vec<String> = hs.iter().map(|holding| holding.commodity.clone()).collect();
        match load_all_coin_chart_series(journal_path, &price_history, &coins, &cfg.currency_symbol)
        {
            Ok(cache) => TabData::Ok(cache),
            Err(e) => TabData::Err(e.to_string()),
        }
    } else {
        TabData::NotRequested
    };

    let account_balances: TabData<Vec<AccountBalance>> = match h.accounts {
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

    let liabilities = into_tab_data(h.liabilities);
    let liability_progress = into_tab_data(h.liability_progress);
    let net_worth_history = into_tab_data(h.net_worth);
    // Breakdown errors are non-fatal — surface as NotRequested so the chart
    // simply omits the layer instead of poisoning the status bar.
    let net_worth_breakdown: TabData<NetWorthBreakdownSeries> = match h.breakdown {
        None | Some(Err(_)) => TabData::NotRequested,
        Some(Ok(s)) => TabData::Ok(s),
    };
    let monthly = into_tab_data(h.monthly);
    let last_year = into_tab_data(h.last_year);
    let monthly_forecast = into_tab_data(h.forecast);
    let payee_data: TabData<Vec<PayeeSummary>> = into_tab_data(h.payee);
    let liquid_cash_monthly: TabData<Vec<(String, f64)>> = into_tab_data(h.liquid_cash_monthly);

    RefreshResult {
        tabs,
        price_history,
        latest_prices: lp,
        holdings,
        coin_chart_cache,
        account_balances,
        liabilities,
        liability_progress,
        net_worth_history,
        net_worth_breakdown,
        monthly,
        last_year,
        monthly_forecast,
        payee_data,
        liquid_cash_monthly,
    }
}

impl App {
    pub fn start_refresh(&mut self) {
        self.tabs_loaded = TabFlags::none();
        self.start_refresh_tabs(TabFlags::all(true));
    }

    pub fn ensure_tab_loaded(&mut self, tab: Tab) {
        if self.tabs_loaded.get(tab) || self.refresh_rx.is_some() {
            return;
        }
        let mut tabs = TabFlags::none();
        tabs.set(tab, true);
        self.start_refresh_tabs(tabs);
    }

    fn start_refresh_tabs(&mut self, mut tabs: TabFlags) {
        if !self.crypto_enabled() {
            tabs.portfolio = false;
        }
        if self.refresh_rx.is_some() {
            // Already refreshing — queue (or merge into) the pending request
            // so it fires once the current one lands. Without this, pressing
            // `r` during a refresh is silently dropped.
            self.pending_refresh = Some(match self.pending_refresh {
                Some(existing) => TabFlags {
                    portfolio: existing.portfolio || tabs.portfolio,
                    accounts: existing.accounts || tabs.accounts,
                    monthly: existing.monthly || tabs.monthly,
                },
                None => tabs,
            });
            return;
        }
        self.loading = true;
        self.status_msg = "Refreshing…".to_string();

        let jp = self.journal_path.clone();
        let jd = self.journal_dir.clone();
        let cfg = LoadConfig {
            nw_period: self.nw_range.period_arg(),
            currency_symbol: self.config.currency_symbol.clone(),
            assets_account: self.config.assets_account.clone(),
            liabilities_account: self.config.liabilities_account.clone(),
            expenses_account: self.config.expenses_account.clone(),
            liquid_accounts: self.config.liquid_accounts.clone(),
        };

        let (tx, rx) = mpsc::channel();
        self.refresh_rx = Some(rx);

        std::thread::spawn(move || {
            let result = load_all_data(&jp, &jd, cfg, tabs);
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
        if let Some(pending) = self.pending_refresh.take() {
            self.start_refresh_tabs(pending);
        }
        true
    }

    /// Drain any completed background tasks (detail loads, year reloads,
    /// net-worth reloads, price fetches). Called from the main loop on every
    /// tick so the UI picks up async results promptly without blocking input.
    pub fn check_background(&mut self) {
        self.check_price_fetch();
        use ratatui::widgets::TableState;

        if let Some(rx) = &self.account_detail_rx {
            if let Ok(load) = rx.try_recv() {
                self.account_detail_rx = None;
                match load.result {
                    Ok(txns) => {
                        self.detail_account_name = Some(load.name);
                        self.account_detail = Some(txns);
                        self.detail_state = TableState::default().with_selected(0);
                        self.status_msg.clear();
                    }
                    Err(e) => self.status_msg = format!("Error loading transactions: {e}"),
                }
            }
        }
        if let Some(rx) = &self.expense_detail_rx {
            if let Ok(load) = rx.try_recv() {
                self.expense_detail_rx = None;
                match load.result {
                    Ok(txns) => {
                        self.detail_expense_name = Some(load.name);
                        self.expense_detail = Some(txns);
                        self.detail_state = TableState::default().with_selected(0);
                        self.status_msg.clear();
                    }
                    Err(e) => self.status_msg = format!("Error loading transactions: {e}"),
                }
            }
        }
        if let Some(rx) = &self.income_detail_rx {
            if let Ok(load) = rx.try_recv() {
                self.income_detail_rx = None;
                match load.result {
                    Ok(txns) => {
                        self.detail_income_name = Some(load.name);
                        self.income_detail = Some(txns);
                        self.detail_state = TableState::default().with_selected(0);
                        self.status_msg.clear();
                    }
                    Err(e) => self.status_msg = format!("Error loading transactions: {e}"),
                }
            }
        }
        if let Some(rx) = &self.monthly_year_rx {
            if let Ok(load) = rx.try_recv() {
                self.monthly_year_rx = None;
                // Apply only if the year offset still matches what the user
                // is viewing — otherwise this is a stale result for a year
                // they've since cycled away from.
                if load.year_offset == self.monthly_year_offset {
                    match load.result {
                        Ok(m) => {
                            self.monthly = m;
                            self.rebuild_combined_months();
                            self.status_msg.clear();
                        }
                        Err(e) => self.status_msg = format!("Error loading year data: {e}"),
                    }
                } else {
                    // The user navigated to a different year while this load
                    // was in flight. Kick off a fresh load for the current
                    // offset so the screen doesn't stay empty.
                    self.reload_monthly_year();
                }
                self.loading = self.any_bg_in_flight();
            }
        }
        if let Some(rx) = &self.net_worth_rx {
            if let Ok(load) = rx.try_recv() {
                self.net_worth_rx = None;
                match load.history {
                    Ok(series) => {
                        self.net_worth_history = series;
                        if let Some(bd) = load.breakdown {
                            self.net_worth_breakdown = bd;
                        }
                        self.status_msg.clear();
                    }
                    Err(e) => self.status_msg = format!("Error loading net worth: {e}"),
                }
                self.loading = self.any_bg_in_flight();
            }
        }
    }

    /// Any background task still in flight? Used to keep the loading flag
    /// (and spinner) live until every spawned worker has returned.
    fn any_bg_in_flight(&self) -> bool {
        self.refresh_rx.is_some()
            || self.account_detail_rx.is_some()
            || self.expense_detail_rx.is_some()
            || self.income_detail_rx.is_some()
            || self.monthly_year_rx.is_some()
            || self.net_worth_rx.is_some()
    }

    /// Start an asynchronous price fetch if none is already running.
    ///
    /// Fetches spot prices from CoinGecko for every token configured under
    /// `[price_fetch]` and appends today's `P` directives to `prices.journal`
    /// in the journal directory.  The result is surfaced via `check_price_fetch`.
    pub fn start_price_fetch(&mut self) {
        if self.price_fetch_rx.is_some() {
            self.status_msg = "Price fetch already in progress…".to_string();
            return;
        }
        if self.config.price_fetch.tokens.is_empty() {
            self.status_msg =
                "No tokens configured — add [[price_fetch.tokens]] to config".to_string();
            return;
        }

        let prices_path = self.journal_dir.join("prices.journal");
        let tokens = self.config.price_fetch.tokens.clone();
        let currency = self.config.price_fetch.currency.clone();
        let currency_symbol = self.config.currency_symbol.clone();

        self.status_msg = "Fetching prices…".to_string();

        let (tx, rx) = mpsc::channel();
        self.price_fetch_rx = Some(rx);

        std::thread::spawn(move || {
            let result =
                fetch_and_append_prices(&prices_path, &tokens, &currency, &currency_symbol);
            let _ = tx.send(result);
        });
    }

    /// Check whether the auto-fetch-on-startup condition is met and, if so,
    /// kick off a price fetch in the background.
    ///
    /// This is a no-op when:
    /// - No tokens are configured (`[price_fetch]` section absent or empty).
    /// - Today's prices are already present in `prices.journal`.
    /// - A fetch is already running.
    pub fn maybe_auto_fetch_prices(&mut self) {
        if self.config.price_fetch.tokens.is_empty() {
            return;
        }
        let prices_path = self.journal_dir.join("prices.journal");
        if today_prices_present(&prices_path) {
            return;
        }
        self.start_price_fetch();
    }

    /// Poll the price-fetch background thread and apply its result.
    /// Called every tick from the main loop.
    pub fn check_price_fetch(&mut self) {
        let result = match self
            .price_fetch_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            Some(r) => r,
            None => return,
        };
        self.price_fetch_rx = None;
        match result {
            Ok(msg) => {
                let now = Local::now().format("%H:%M:%S");
                self.status_msg = format!("{msg} (at {now})");
                // Re-load prices from disk so the portfolio chart reflects the
                // freshly appended entries without waiting for the next full refresh.
                self.start_refresh();
            }
            Err(e) => {
                self.status_msg = format!("Price fetch failed: {e}");
            }
        }
    }

    pub(super) fn apply_refresh(&mut self, r: RefreshResult) {
        // Capture per-tab Ok status BEFORE field-by-field consumption below
        // moves the `TabData` values out of `r`. Tabs that errored stay
        // unloaded so the next interaction (or auto-refresh) retries.
        fn is_ok<T>(t: &TabData<T>) -> bool {
            matches!(t, TabData::Ok(_))
        }
        let portfolio_ok = r.tabs.portfolio && is_ok(&r.holdings);
        let accounts_ok = r.tabs.accounts && is_ok(&r.account_balances);
        let monthly_ok = r.tabs.monthly && is_ok(&r.monthly);

        self.price_history = r.price_history;
        self.latest_prices = r.latest_prices;

        let mut errors: Vec<String> = Vec::new();

        macro_rules! apply_field {
            ($field:expr, $src:expr, $errs:expr) => {
                match $src {
                    TabData::Ok(v) => $field = v,
                    TabData::Err(e) => $errs.push(e),
                    TabData::NotRequested => {}
                }
            };
        }
        // Like apply_field, but silently swallow errors (used for non-fatal data
        // such as the breakdown chart and forecast — partial UI is acceptable).
        macro_rules! apply_field_silent {
            ($field:expr, $src:expr) => {
                match $src {
                    TabData::Ok(v) => $field = v,
                    TabData::Err(_) | TabData::NotRequested => {}
                }
            };
        }

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
        apply_field!(self.coin_chart_cache, r.coin_chart_cache, errors);
        apply_field!(self.account_balances, r.account_balances, errors);
        apply_field!(self.liabilities, r.liabilities, errors);
        // Liability-progress errors are non-fatal: the table just omits the
        // progress bar and shows balances-only rather than failing entirely.
        apply_field_silent!(self.liability_progress, r.liability_progress);
        apply_field!(self.net_worth_history, r.net_worth_history, errors);
        apply_field_silent!(self.net_worth_breakdown, r.net_worth_breakdown);
        apply_field!(self.monthly, r.monthly, errors);
        apply_field!(self.last_year, r.last_year, errors);
        // Forecast errors are non-fatal: chart just omits the projected line
        // (user may not have periodic transaction rules).
        apply_field_silent!(self.monthly_forecast, r.monthly_forecast);
        apply_field!(self.payee_data, r.payee_data, errors);
        // Liquid-cash errors are non-fatal: the summary line just omits
        // itself (e.g. user hasn't configured `liquid_accounts` yet).
        apply_field_silent!(self.liquid_cash_monthly, r.liquid_cash_monthly);
        self.rebuild_combined_months();

        // Current year has no data → jump to the last year with actual entries
        // so the Monthly tab doesn't open on a blank screen.
        if self.combined_months.is_empty() && self.monthly_year_offset == 0 && r.tabs.monthly {
            self.jump_to_last_entry();
        }

        if !self.holdings.is_empty() && self.selected_holding >= self.holdings.len() {
            self.selected_holding = self.holdings.len() - 1;
        }

        if portfolio_ok {
            self.tabs_loaded.portfolio = true;
        }
        if accounts_ok {
            self.tabs_loaded.accounts = true;
        }
        if monthly_ok {
            self.tabs_loaded.monthly = true;
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

        if !self.alert_dismissed {
            self.compute_price_alerts();
        }
    }

    fn fix_tab_after_visibility_change(&mut self) {
        let visible = self.visible_tabs();
        if !visible.contains(&self.tab) {
            self.tab = visible[0];
            self.tabs_loaded.portfolio = false;
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
