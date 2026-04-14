use anyhow::Result;
use chrono::Local;
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use crate::data::{
    compute_portfolio, latest_prices, load_account_balances_eur, load_coin_chart_series,
    load_crypto_balances, load_last_year_monthly, load_monthly_data, load_net_worth_history,
    load_price_history, load_recent_transactions, AccountBalance, CoinChartSeries, CryptoHolding,
    MonthlyData, NetWorthSeries, PriceEntry, SingleMonth, Transaction,
};

pub struct RefreshResult {
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: Vec<CryptoHolding>,
    pub coin_chart_cache: HashMap<String, CoinChartSeries>,
    pub account_balances: Vec<AccountBalance>,
    pub net_worth_history: NetWorthSeries,
    pub monthly: MonthlyData,
    pub last_year: MonthlyData,
    pub errors: Vec<String>,
}

fn load_all_data(journal_path: &Path, journal_dir: &Path, nw_period: &str) -> RefreshResult {
    let mut errors = Vec::new();

    let price_history = load_price_history(journal_dir);
    let lp = latest_prices(&price_history);

    let (crypto_res, accounts_res, nw_res, monthly_res, ly_res) = std::thread::scope(|s| {
        let t_crypto = s.spawn(|| load_crypto_balances(journal_path));
        let t_accounts = s.spawn(|| load_account_balances_eur(journal_path));
        let t_nw = s.spawn(|| load_net_worth_history(journal_path, nw_period));
        let t_monthly = s.spawn(|| load_monthly_data(journal_path));
        let t_ly = s.spawn(|| load_last_year_monthly(journal_path));
        (
            t_crypto.join().unwrap(),
            t_accounts.join().unwrap(),
            t_nw.join().unwrap(),
            t_monthly.join().unwrap(),
            t_ly.join().unwrap(),
        )
    });

    let holdings = match crypto_res {
        Ok(balances) => compute_portfolio(&balances, &lp),
        Err(e) => {
            errors.push(format!("Error loading crypto balances: {e}"));
            Vec::new()
        }
    };

    let coin_chart_cache: HashMap<String, CoinChartSeries> = std::thread::scope(|s| {
        let handles: Vec<_> = holdings
            .iter()
            .map(|h| {
                let coin = h.commodity.clone();
                let prices = &price_history;
                s.spawn(move || {
                    let series = load_coin_chart_series(journal_path, prices, &coin);
                    (coin, series)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut account_balances = match accounts_res {
        Ok(b) => b,
        Err(e) => {
            errors.push(format!("Error loading account balances: {e}"));
            Vec::new()
        }
    };
    account_balances.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let net_worth_history = match nw_res {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("Error loading net worth history: {e}"));
            NetWorthSeries::default()
        }
    };

    let monthly = match monthly_res {
        Ok(m) => m,
        Err(e) => {
            errors.push(format!("Error loading monthly data: {e}"));
            MonthlyData::default()
        }
    };

    let last_year = match ly_res {
        Ok(ly) => ly,
        Err(e) => {
            errors.push(format!("Error loading last year data: {e}"));
            MonthlyData::default()
        }
    };

    RefreshResult {
        price_history,
        latest_prices: lp,
        holdings,
        coin_chart_cache,
        account_balances,
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
    Year1,
    Year2,
    Year5,
    All,
}

impl NetWorthRange {
    pub fn period_arg(self) -> &'static str {
        match self {
            Self::Year1 => "monthly from 1 year ago",
            Self::Year2 => "monthly from 2 years ago",
            Self::Year5 => "monthly from 5 years ago",
            Self::All => "monthly",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Year1 => "1Y",
            Self::Year2 => "2Y",
            Self::Year5 => "5Y",
            Self::All => "All",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Year1 => Self::Year2,
            Self::Year2 => Self::Year5,
            Self::Year5 => Self::All,
            Self::All => Self::All,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Year1 => Self::Year1,
            Self::Year2 => Self::Year1,
            Self::Year5 => Self::Year2,
            Self::All => Self::Year5,
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

pub struct App {
    pub journal_path: PathBuf,
    pub journal_dir: PathBuf,
    pub tab: Tab,
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: Vec<CryptoHolding>,
    pub coin_chart_cache: HashMap<String, CoinChartSeries>,
    pub account_balances: Vec<AccountBalance>,
    pub net_worth_history: NetWorthSeries,
    pub nw_range: NetWorthRange,
    pub monthly: MonthlyData,
    pub last_year: MonthlyData,
    pub selected_holding: usize,
    pub account_state: TableState,
    pub expense_state: TableState,
    pub account_detail: Option<Vec<Transaction>>,
    pub detail_account_name: Option<String>,
    pub expense_colors: bool,
    pub status_msg: String,
    pub loading: bool,
    pub show_help: bool,
    pub last_refresh: Instant,
    refresh_rx: Option<mpsc::Receiver<RefreshResult>>,
}

impl App {
    pub fn new(journal_path: PathBuf) -> Result<Self> {
        let journal_dir = journal_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();

        Ok(Self {
            journal_path,
            journal_dir,
            tab: Tab::Portfolio,
            price_history: Vec::new(),
            latest_prices: HashMap::new(),
            holdings: Vec::new(),
            coin_chart_cache: HashMap::new(),
            account_balances: Vec::new(),
            net_worth_history: NetWorthSeries::default(),
            nw_range: NetWorthRange::Year2,
            monthly: MonthlyData::default(),
            last_year: MonthlyData::default(),
            selected_holding: 0,
            account_state: TableState::default().with_selected(0),
            expense_state: TableState::default().with_selected(0),
            account_detail: None,
            detail_account_name: None,
            expense_colors: true,
            status_msg: "Loading data…".to_string(),
            loading: true,
            show_help: false,
            last_refresh: Instant::now(),
            refresh_rx: None,
        })
    }

    pub fn start_refresh(&mut self) {
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
            let result = load_all_data(&jp, &jd, &nw_period);
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
        self.holdings = r.holdings;
        self.coin_chart_cache = r.coin_chart_cache;
        self.account_balances = r.account_balances;
        self.net_worth_history = r.net_worth_history;
        self.monthly = r.monthly;
        self.last_year = r.last_year;

        if !self.holdings.is_empty() && self.selected_holding >= self.holdings.len() {
            self.selected_holding = self.holdings.len() - 1;
        }

        self.last_refresh = Instant::now();
        if r.errors.is_empty() {
            let now = Local::now().format("%H:%M:%S");
            self.status_msg = format!("Updated at {now}");
        } else {
            self.status_msg = r.errors.last().unwrap().clone();
        }
        self.loading = false;
    }

    pub fn next_tab(&mut self) {
        let tabs = Tab::all();
        let idx = (self.tab.index() + 1) % tabs.len();
        self.tab = tabs[idx];
    }

    pub fn prev_tab(&mut self) {
        let tabs = Tab::all();
        let idx = (self.tab.index() + tabs.len() - 1) % tabs.len();
        self.tab = tabs[idx];
    }

    pub fn select_tab(&mut self, idx: usize) {
        if let Some(&t) = Tab::all().get(idx) {
            self.tab = t;
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

    pub fn total_portfolio_pl(&self) -> (f64, f64) {
        let mut total_invested = 0.0_f64;
        let mut total_value = 0.0_f64;
        for h in &self.holdings {
            if let Some(s) = self.coin_chart_cache.get(&h.commodity) {
                if !s.investment.is_empty() {
                    total_invested += s.total_invested();
                    total_value += h.value_eur;
                }
            }
        }
        let pl_abs = total_value - total_invested;
        let pl_pct = if total_invested > 0.0 {
            pl_abs / total_invested * 100.0
        } else {
            0.0
        };
        (pl_abs, pl_pct)
    }

    pub fn total_net_worth(&self) -> f64 {
        self.account_balances.iter().map(|b| b.amount).sum()
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

    fn reload_net_worth(&mut self) {
        match load_net_worth_history(&self.journal_path, self.nw_range.period_arg()) {
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
            match load_recent_transactions(&self.journal_path, &account, 30) {
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
}
