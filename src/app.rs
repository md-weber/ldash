use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use crate::config::Config;
use crate::data::{
    self, compute_portfolio, latest_prices, load_account_balances_eur, load_all_coin_chart_series,
    load_crypto_balances, load_last_year_monthly, load_liability_balances_eur, load_monthly_data,
    load_net_worth_history, load_price_history, load_recent_transactions, AccountBalance,
    CoinChartSeries, CryptoHolding, MonthlyData, NetWorthSeries, PriceEntry, SingleMonth,
    Transaction,
};
use crate::watcher::JournalWatcher;

pub struct RefreshResult {
    pub tabs: [bool; 3],
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: Option<Vec<CryptoHolding>>,
    pub coin_chart_cache: Option<HashMap<String, CoinChartSeries>>,
    pub account_balances: Option<Vec<AccountBalance>>,
    pub liabilities: Option<Vec<AccountBalance>>,
    pub net_worth_history: Option<NetWorthSeries>,
    pub monthly: Option<MonthlyData>,
    pub last_year: Option<MonthlyData>,
    pub errors: Vec<String>,
}

fn load_all_data(
    journal_path: &Path,
    journal_dir: &Path,
    nw_period: &str,
    tabs: [bool; 3],
) -> RefreshResult {
    let mut errors = Vec::new();

    let price_history = load_price_history(journal_dir);
    let lp = latest_prices(&price_history);

    let want_portfolio = tabs[0];
    let want_accounts = tabs[1];
    let want_monthly = tabs[2];

    let (crypto_res, accounts_res, liab_res, nw_res, monthly_res, ly_res) =
        std::thread::scope(|s| {
            let t_crypto =
                want_portfolio.then(|| s.spawn(|| load_crypto_balances(journal_path)));
            let t_accounts =
                want_accounts.then(|| s.spawn(|| load_account_balances_eur(journal_path)));
            let t_liabilities =
                want_accounts.then(|| s.spawn(|| load_liability_balances_eur(journal_path)));
            let t_nw =
                want_accounts.then(|| s.spawn(|| load_net_worth_history(journal_path, nw_period)));
            let t_monthly = want_monthly.then(|| s.spawn(|| load_monthly_data(journal_path)));
            let t_ly = want_monthly.then(|| s.spawn(|| load_last_year_monthly(journal_path)));
            (
                t_crypto.map(|t| t.join().unwrap()),
                t_accounts.map(|t| t.join().unwrap()),
                t_liabilities.map(|t| t.join().unwrap()),
                t_nw.map(|t| t.join().unwrap()),
                t_monthly.map(|t| t.join().unwrap()),
                t_ly.map(|t| t.join().unwrap()),
            )
        });

    let holdings = crypto_res.map(|res| match res {
        Ok(balances) => compute_portfolio(&balances, &lp),
        Err(e) => {
            errors.push(format!("Error loading crypto balances: {e}"));
            Vec::new()
        }
    });

    let coin_chart_cache = holdings.as_ref().map(|h| {
        let coins: Vec<String> = h.iter().map(|holding| holding.commodity.clone()).collect();
        load_all_coin_chart_series(journal_path, &price_history, &coins)
    });

    let account_balances = accounts_res.map(|res| {
        let mut balances = match res {
            Ok(b) => b,
            Err(e) => {
                errors.push(format!("Error loading account balances: {e}"));
                Vec::new()
            }
        };
        balances.sort_by(|a, b| {
            b.amount
                .partial_cmp(&a.amount)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        balances
    });

    let liabilities = liab_res.map(|res| match res {
        Ok(l) => l,
        Err(e) => {
            errors.push(format!("Error loading liabilities: {e}"));
            Vec::new()
        }
    });

    let net_worth_history = nw_res.map(|res| match res {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("Error loading net worth history: {e}"));
            NetWorthSeries::default()
        }
    });

    let monthly = monthly_res.map(|res| match res {
        Ok(m) => m,
        Err(e) => {
            errors.push(format!("Error loading monthly data: {e}"));
            MonthlyData::default()
        }
    });

    let last_year = ly_res.map(|res| match res {
        Ok(ly) => ly,
        Err(e) => {
            errors.push(format!("Error loading last year data: {e}"));
            MonthlyData::default()
        }
    });

    RefreshResult {
        tabs,
        price_history,
        latest_prices: lp,
        holdings,
        coin_chart_cache,
        account_balances,
        liabilities,
        net_worth_history,
        monthly,
        last_year,
        errors,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Portfolio,
    Accounts,
    Monthly,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::Portfolio, Tab::Accounts, Tab::Monthly]
    }
    pub fn index(self) -> usize {
        match self {
            Tab::Portfolio => 0,
            Tab::Accounts => 1,
            Tab::Monthly => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetWorthRange {
    All,
    Ytd,
    Year1,
    Year2,
    Year5,
}

impl NetWorthRange {
    pub fn period_arg(self) -> String {
        let today = Local::now().date_naive();
        match self {
            Self::All => "monthly".to_string(),
            Self::Ytd => {
                let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
                format!("monthly from {}", jan1.format("%Y-%m-%d"))
            }
            Self::Year1 => {
                let start = today - chrono::Duration::days(365);
                format!("monthly from {}", start.format("%Y-%m-%d"))
            }
            Self::Year2 => {
                let start = today - chrono::Duration::days(730);
                format!("monthly from {}", start.format("%Y-%m-%d"))
            }
            Self::Year5 => {
                let start = today - chrono::Duration::days(1825);
                format!("monthly from {}", start.format("%Y-%m-%d"))
            }
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Ytd => "YTD",
            Self::Year1 => "1Y",
            Self::Year2 => "2Y",
            Self::Year5 => "5Y",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Ytd,
            Self::Ytd => Self::Year1,
            Self::Year1 => Self::Year2,
            Self::Year2 => Self::Year5,
            Self::Year5 => Self::Year5,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::All => Self::All,
            Self::Ytd => Self::All,
            Self::Year1 => Self::Ytd,
            Self::Year2 => Self::Year1,
            Self::Year5 => Self::Year2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortfolioRange {
    Month3,
    Month6,
    Ytd,
    All,
}

impl PortfolioRange {
    pub fn label(self) -> &'static str {
        match self {
            Self::Month3 => "3M",
            Self::Month6 => "6M",
            Self::Ytd => "YTD",
            Self::All => "All",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Month3 => Self::Month6,
            Self::Month6 => Self::Ytd,
            Self::Ytd => Self::All,
            Self::All => Self::All,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Month3 => Self::Month3,
            Self::Month6 => Self::Month3,
            Self::Ytd => Self::Month6,
            Self::All => Self::Ytd,
        }
    }
}

#[derive(Debug, Default)]
pub struct YtdStats {
    pub total_income: f64,
    pub total_expenses: f64,
    pub avg_savings_rate: f64,
    pub best_month: String,
    pub best_net: f64,
    pub worst_month: String,
    pub worst_net: f64,
}

pub struct BudgetItem {
    pub category: String,
    pub limit: f64,
    pub spent: f64,
    pub pct: f64,
}

pub struct GoalProgress {
    pub name: String,
    pub target: f64,
    pub current: f64,
    pub pct: f64,
}

pub struct PriceAlert {
    pub coin: String,
    pub change_pct: f64,
}

pub struct App {
    pub journal_path: PathBuf,
    pub journal_dir: PathBuf,
    pub config: Config,
    pub tab: Tab,
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: Vec<CryptoHolding>,
    pub coin_chart_cache: HashMap<String, CoinChartSeries>,
    pub account_balances: Vec<AccountBalance>,
    pub liabilities: Vec<AccountBalance>,
    pub net_worth_history: NetWorthSeries,
    pub nw_range: NetWorthRange,
    pub monthly: MonthlyData,
    pub last_year: MonthlyData,
    pub monthly_year_offset: i32,
    pub selected_holding: usize,
    pub account_state: TableState,
    pub expense_state: TableState,
    pub account_detail: Option<Vec<Transaction>>,
    pub detail_account_name: Option<String>,
    pub expense_detail: Option<Vec<Transaction>>,
    pub detail_expense_name: Option<String>,
    pub portfolio_range: PortfolioRange,
    pub chart_stacked: bool,
    pub expense_colors: bool,
    pub status_msg: String,
    pub loading: bool,
    pub show_help: bool,
    pub search_active: bool,
    pub search_query: String,
    pub search_results: Vec<Transaction>,
    pub search_state: TableState,
    pub price_alerts: Vec<PriceAlert>,
    pub show_alerts: bool,
    pub alert_dismissed: bool,
    pub alert_shown_at: Option<Instant>,
    pub last_refresh: Instant,
    pub tabs_loaded: [bool; 3],
    refresh_rx: Option<mpsc::Receiver<RefreshResult>>,
    watcher: Option<JournalWatcher>,
    last_journal_mtime: Option<SystemTime>,
    last_config_mtime: Option<SystemTime>,
}

impl App {
    pub fn new(journal_path: PathBuf, config: Config) -> Result<Self> {
        let journal_dir = journal_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();

        let watcher = JournalWatcher::new(&journal_path);
        if watcher.is_none() {
            eprintln!("Warning: file watcher unavailable, falling back to polling");
        }

        let default_tab = match config.default_tab_index() {
            1 => Tab::Accounts,
            2 => Tab::Monthly,
            _ => Tab::Portfolio,
        };
        let chart_stacked = config.chart_mode != "unstacked";

        Ok(Self {
            journal_path,
            journal_dir,
            config,
            tab: default_tab,
            price_history: Vec::new(),
            latest_prices: HashMap::new(),
            holdings: Vec::new(),
            coin_chart_cache: HashMap::new(),
            account_balances: Vec::new(),
            liabilities: Vec::new(),
            net_worth_history: NetWorthSeries::default(),
            nw_range: NetWorthRange::All,
            monthly: MonthlyData::default(),
            last_year: MonthlyData::default(),
            monthly_year_offset: 0,
            selected_holding: 0,
            account_state: TableState::default().with_selected(0),
            expense_state: TableState::default().with_selected(0),
            account_detail: None,
            detail_account_name: None,
            expense_detail: None,
            detail_expense_name: None,
            portfolio_range: PortfolioRange::All,
            chart_stacked,
            expense_colors: true,
            status_msg: "Loading data…".to_string(),
            loading: true,
            show_help: false,
            search_active: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_state: TableState::default(),
            price_alerts: Vec::new(),
            show_alerts: false,
            alert_dismissed: false,
            alert_shown_at: None,
            last_refresh: Instant::now(),
            tabs_loaded: [false; 3],
            refresh_rx: None,
            watcher,
            last_journal_mtime: None,
            last_config_mtime: Config::config_mtime(),
        })
    }

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

    fn start_refresh_tabs(&mut self, tabs: [bool; 3]) {
        if self.refresh_rx.is_some() {
            return;
        }
        self.loading = true;
        self.status_msg = "Refreshing…".to_string();

        let jp = self.journal_path.clone();
        let jd = self.journal_dir.clone();
        let nw_period = self.nw_range.period_arg().to_string();

        let (tx, rx) = mpsc::channel();
        self.refresh_rx = Some(rx);

        std::thread::spawn(move || {
            let result = load_all_data(&jp, &jd, &nw_period, tabs);
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

    fn apply_refresh(&mut self, r: RefreshResult) {
        self.price_history = r.price_history;
        self.latest_prices = r.latest_prices;

        if let Some(h) = r.holdings {
            self.holdings = h;
        }
        if let Some(c) = r.coin_chart_cache {
            self.coin_chart_cache = c;
        }
        if let Some(a) = r.account_balances {
            self.account_balances = a;
        }
        if let Some(l) = r.liabilities {
            self.liabilities = l;
        }
        if let Some(nw) = r.net_worth_history {
            self.net_worth_history = nw;
        }
        if let Some(m) = r.monthly {
            self.monthly = m;
        }
        if let Some(ly) = r.last_year {
            self.last_year = ly;
        }

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
        if r.errors.is_empty() {
            let now = Local::now().format("%H:%M:%S");
            self.status_msg = format!("Updated at {now}");
        } else {
            self.status_msg = r.errors.last().unwrap().clone();
        }
        self.loading = false;

        if self.price_alerts.is_empty() && !self.alert_dismissed {
            self.compute_price_alerts();
        }
    }

    pub fn auto_refresh(&mut self) {
        self.check_config_reload();

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

    pub fn has_watcher(&self) -> bool {
        self.watcher.is_some()
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

    pub fn next_tab(&mut self) {
        let tabs = Tab::all();
        let idx = (self.tab.index() + 1) % tabs.len();
        self.tab = tabs[idx];
        self.ensure_tab_loaded(self.tab);
    }

    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let idx = (self.tab.index() + tabs.len() - 1) % tabs.len();
        self.tab = tabs[idx];
        self.ensure_tab_loaded(self.tab);
    }

    pub fn select_tab(&mut self, idx: usize) {
        if let Some(&t) = Tab::all().get(idx) {
            self.tab = t;
            self.ensure_tab_loaded(t);
        }
    }

    pub fn scroll_up(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                self.selected_holding = self.selected_holding.saturating_sub(1);
            }
            Tab::Accounts => {
                let i = self.account_state.selected().unwrap_or(0);
                self.account_state.select(Some(i.saturating_sub(1)));
            }
            Tab::Monthly => {
                let i = self.expense_state.selected().unwrap_or(0);
                self.expense_state.select(Some(i.saturating_sub(1)));
            }
        }
    }

    pub fn scroll_down(&mut self) {
        match self.tab {
            Tab::Portfolio => {
                if self.selected_holding + 1 < self.holdings.len() {
                    self.selected_holding += 1;
                }
            }
            Tab::Accounts => {
                let i = self.account_state.selected().unwrap_or(0);
                if i + 1 < self.account_balances.len() {
                    self.account_state.select(Some(i + 1));
                }
            }
            Tab::Monthly => {
                let i = self.expense_state.selected().unwrap_or(0);
                let len = self.current_month().map(|m| m.expenses.len()).unwrap_or(0);
                if i + 1 < len {
                    self.expense_state.select(Some(i + 1));
                }
            }
        }
    }

    pub fn selected_coin(&self) -> Option<&str> {
        self.holdings
            .get(self.selected_holding)
            .map(|h| h.commodity.as_str())
    }

    pub fn total_portfolio_value(&self) -> f64 {
        self.holdings.iter().map(|h| h.value_eur).sum()
    }

    pub fn total_net_worth(&self) -> f64 {
        let assets: f64 = self.account_balances.iter().map(|b| b.amount).sum();
        let liabs: f64 = self.liabilities.iter().map(|b| b.amount).sum();
        assets + liabs
    }

    pub fn current_month(&self) -> Option<&SingleMonth> {
        self.monthly.months.get(self.monthly.selected)
    }

    pub fn last_year_match(&self) -> Option<&SingleMonth> {
        let cur = self.current_month()?;
        self.last_year
            .months
            .iter()
            .find(|m| m.month_name == cur.month_name)
    }

    pub fn ytd_stats(&self) -> YtdStats {
        let months = &self.monthly.months;
        if months.is_empty() {
            return YtdStats::default();
        }
        let total_income: f64 = months.iter().map(|m| m.total_income).sum();
        let total_expenses: f64 = months.iter().map(|m| m.total_expenses).sum();
        let avg_savings_rate = if total_income > 0.0 {
            ((total_income - total_expenses) / total_income * 100.0).max(0.0)
        } else {
            0.0
        };
        let best = months
            .iter()
            .max_by(|a, b| {
                let na = a.total_income - a.total_expenses;
                let nb = b.total_income - b.total_expenses;
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        let worst = months
            .iter()
            .min_by(|a, b| {
                let na = a.total_income - a.total_expenses;
                let nb = b.total_income - b.total_expenses;
                na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();
        YtdStats {
            total_income,
            total_expenses,
            avg_savings_rate,
            best_month: best.month_name.clone(),
            best_net: best.total_income - best.total_expenses,
            worst_month: worst.month_name.clone(),
            worst_net: worst.total_income - worst.total_expenses,
        }
    }

    pub fn budget_status(&self) -> Vec<BudgetItem> {
        let m = match self.current_month() {
            Some(m) => m,
            None => return Vec::new(),
        };
        self.config.budgets.iter().filter_map(|(category, &limit)| {
            let spent = m.expenses.iter()
                .find(|(name, _)| {
                    let short = name.strip_prefix("expenses:").unwrap_or(name);
                    let full = format!("expenses:{}", short);
                    category == name
                        || category == short
                        || category == &full
                        || category.eq_ignore_ascii_case(name)
                        || category.eq_ignore_ascii_case(short)
                        || category.eq_ignore_ascii_case(&full)
                })
                .map(|(_, amount)| *amount)
                .unwrap_or(0.0);
            Some(BudgetItem {
                category: category.clone(),
                limit,
                spent,
                pct: if limit > 0.0 { spent / limit * 100.0 } else { 0.0 },
            })
        }).collect()
    }

    pub fn goal_progress(&self) -> Vec<GoalProgress> {
        self.config.goals.iter().map(|g| {
            let current = self.account_balances.iter()
                .filter(|b| b.account.starts_with(&g.account))
                .map(|b| b.amount)
                .sum::<f64>();
            let pct = if g.target > 0.0 { (current / g.target * 100.0).min(100.0) } else { 0.0 };
            GoalProgress {
                name: g.name.clone(),
                target: g.target,
                current,
                pct,
            }
        }).collect()
    }

    pub fn cycle_year_back(&mut self) {
        if self.monthly_year_offset > -3 {
            self.monthly_year_offset -= 1;
            self.reload_monthly_year();
        }
    }

    pub fn cycle_year_forward(&mut self) {
        if self.monthly_year_offset < 0 {
            self.monthly_year_offset += 1;
            self.reload_monthly_year();
        }
    }

    fn reload_monthly_year(&mut self) {
        let year = Local::now().date_naive().year() + self.monthly_year_offset;
        let period = format!("monthly in {year}");
        match data::load_monthly_for_period(&self.journal_path, &period) {
            Ok(m) => self.monthly = m,
            Err(e) => self.status_msg = format!("Error loading {year} data: {e}"),
        }
    }

    pub fn displayed_year(&self) -> i32 {
        Local::now().date_naive().year() + self.monthly_year_offset
    }

    fn compute_price_alerts(&mut self) {
        if self.alert_dismissed || self.price_history.len() < 2 {
            return;
        }

        let today = Local::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);
        let mut alerts = Vec::new();

        let coins: Vec<String> = self.holdings.iter().map(|h| h.commodity.clone()).collect();
        for coin in &coins {
            let prices: Vec<_> = self.price_history.iter()
                .filter(|e| &e.commodity == coin)
                .collect();

            let current = prices.last().map(|e| e.price_eur);
            let prev = prices.iter().rev()
                .find(|e| e.date <= yesterday)
                .map(|e| e.price_eur);

            if let (Some(cur), Some(old)) = (current, prev) {
                if old > 0.0 {
                    let change = (cur - old) / old * 100.0;
                    if change.abs() >= 2.0 {
                        alerts.push(PriceAlert { coin: coin.clone(), change_pct: change });
                    }
                }
            }
        }

        if !alerts.is_empty() {
            alerts.sort_by(|a, b| b.change_pct.abs().partial_cmp(&a.change_pct.abs())
                .unwrap_or(std::cmp::Ordering::Equal));
            self.price_alerts = alerts;
            self.show_alerts = true;
            self.alert_shown_at = Some(Instant::now());
        }
    }

    pub fn check_alert_timeout(&mut self) {
        if self.show_alerts {
            if let Some(shown_at) = self.alert_shown_at {
                if shown_at.elapsed() >= Duration::from_secs(5) {
                    self.show_alerts = false;
                    self.alert_dismissed = true;
                }
            }
        }
    }

    pub fn export_current_view(&self) -> String {
        match self.tab {
            Tab::Portfolio => {
                let mut csv = String::from("Coin,Amount,Price EUR,Value EUR,Allocation %\n");
                let total = self.total_portfolio_value();
                for h in &self.holdings {
                    let alloc = if total > 0.0 { h.value_eur / total * 100.0 } else { 0.0 };
                    csv.push_str(&format!(
                        "{},{:.6},{:.2},{:.2},{:.1}\n",
                        h.commodity, h.amount, h.price_eur, h.value_eur, alloc
                    ));
                }
                csv
            }
            Tab::Accounts => {
                if let Some(sel) = self.account_state.selected() {
                    if let Some(b) = self.account_balances.get(sel) {
                        return format!("{}\t{:.2} €", b.account, b.amount);
                    }
                }
                let mut csv = String::from("Account,Balance EUR\n");
                for b in &self.account_balances {
                    csv.push_str(&format!("{},{:.2}\n", b.account, b.amount));
                }
                csv
            }
            Tab::Monthly => {
                let empty = data::SingleMonth::default();
                let m = self.current_month().unwrap_or(&empty);
                let mut csv = format!("# {} Income/Expenses\n", m.month_name);
                csv.push_str("Category,Amount\n");
                for (name, amount) in &m.income {
                    csv.push_str(&format!("{},{:.2}\n", name, amount));
                }
                for (name, amount) in &m.expenses {
                    csv.push_str(&format!("{},-{:.2}\n", name, amount));
                }
                csv
            }
        }
    }

    pub fn month_left(&mut self) {
        self.monthly.selected = self.monthly.selected.saturating_sub(1);
    }

    pub fn month_right(&mut self) {
        if !self.monthly.months.is_empty() && self.monthly.selected + 1 < self.monthly.months.len()
        {
            self.monthly.selected += 1;
        }
    }

    pub fn nw_range_left(&mut self) {
        let prev = self.nw_range.prev();
        if prev != self.nw_range {
            self.nw_range = prev;
            self.reload_net_worth();
        }
    }

    pub fn nw_range_right(&mut self) {
        let next = self.nw_range.next();
        if next != self.nw_range {
            self.nw_range = next;
            self.reload_net_worth();
        }
    }

    /// Min x (days offset) for current portfolio range, given a coin's first_date.
    pub fn portfolio_range_min_x(&self, first_date: NaiveDate) -> f64 {
        let today = Local::now().date_naive();
        let today_x = (today - first_date).num_days() as f64;
        match self.portfolio_range {
            PortfolioRange::Month3 => (today_x - 90.0).max(0.0),
            PortfolioRange::Month6 => (today_x - 180.0).max(0.0),
            PortfolioRange::Ytd => {
                let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap();
                (jan1 - first_date).num_days().max(0) as f64
            }
            PortfolioRange::All => 0.0,
        }
    }

    pub fn portfolio_range_left(&mut self) {
        self.portfolio_range = self.portfolio_range.prev();
    }

    pub fn portfolio_range_right(&mut self) {
        self.portfolio_range = self.portfolio_range.next();
    }

    fn reload_net_worth(&mut self) {
        let period = self.nw_range.period_arg();
        match load_net_worth_history(&self.journal_path, &period) {
            Ok(series) => self.net_worth_history = series,
            Err(e) => self.status_msg = format!("Error loading net worth: {e}"),
        }
    }

    pub fn open_account_detail(&mut self) {
        if self.tab != Tab::Accounts || self.account_detail.is_some() {
            return;
        }
        let sel = self.account_state.selected().unwrap_or(0);
        if let Some(b) = self.account_balances.get(sel) {
            let account = b.account.clone();
            match load_recent_transactions(&self.journal_path, &account, 30, None) {
                Ok(txns) => {
                    self.detail_account_name = Some(account);
                    self.account_detail = Some(txns);
                }
                Err(e) => {
                    self.status_msg = format!("Error loading transactions: {e}");
                }
            }
        }
    }

    pub fn close_account_detail(&mut self) {
        self.account_detail = None;
        self.detail_account_name = None;
    }

    pub fn open_expense_detail(&mut self) {
        if self.tab != Tab::Monthly || self.expense_detail.is_some() {
            return;
        }
        let sel = self.expense_state.selected().unwrap_or(0);
        let (category, period) = match self.current_month() {
            Some(m) => match m.expenses.get(sel) {
                Some((cat, _)) => (cat.clone(), month_name_to_period(&m.month_name, self.displayed_year())),
                None => return,
            },
            None => return,
        };
        match load_recent_transactions(&self.journal_path, &category, 50, Some(&period)) {
            Ok(txns) => {
                self.detail_expense_name = Some(category);
                self.expense_detail = Some(txns);
            }
            Err(e) => {
                self.status_msg = format!("Error loading transactions: {e}");
            }
        }
    }

    pub fn close_expense_detail(&mut self) {
        self.expense_detail = None;
        self.detail_expense_name = None;
    }

    pub fn open_search(&mut self) {
        self.search_active = true;
        self.search_query.clear();
        self.search_results.clear();
        self.search_state = TableState::default();
    }

    pub fn close_search(&mut self) {
        self.search_active = false;
        self.search_query.clear();
        self.search_results.clear();
    }

    pub fn execute_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_results.clear();
            return;
        }
        match data::search_transactions(&self.journal_path, &self.search_query) {
            Ok(txns) => {
                self.search_results = txns;
                if !self.search_results.is_empty() {
                    self.search_state = TableState::default().with_selected(0);
                }
            }
            Err(e) => {
                self.status_msg = format!("Search error: {e}");
            }
        }
    }

    pub fn search_scroll_up(&mut self) {
        let i = self.search_state.selected().unwrap_or(0);
        self.search_state.select(Some(i.saturating_sub(1)));
    }

    pub fn search_scroll_down(&mut self) {
        let i = self.search_state.selected().unwrap_or(0);
        if i + 1 < self.search_results.len() {
            self.search_state.select(Some(i + 1));
        }
    }
}

fn month_name_to_period(month_name: &str, year: i32) -> String {
    let month_num = match month_name {
        "January" => 1,
        "February" => 2,
        "March" => 3,
        "April" => 4,
        "May" => 5,
        "June" => 6,
        "July" => 7,
        "August" => 8,
        "September" => 9,
        "October" => 10,
        "November" => 11,
        "December" => 12,
        _ => 1,
    };
    format!("{year}-{month_num:02}")
}
