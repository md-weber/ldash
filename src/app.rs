use anyhow::Result;
use chrono::Local;
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::data::{
    compute_portfolio, latest_prices, load_account_balances_eur, load_coin_chart_series,
    load_crypto_balances, load_monthly_data, load_net_worth_history, load_price_history,
    load_recent_transactions, AccountBalance, CoinChartSeries, CryptoHolding, MonthlyData,
    NetWorthSeries, PriceEntry, SingleMonth, Transaction,
};

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
    pub monthly: MonthlyData,
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
}

impl App {
    pub fn new(journal_path: PathBuf) -> Result<Self> {
        let journal_dir = journal_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();

        let mut app = Self {
            journal_path,
            journal_dir,
            tab: Tab::Portfolio,
            price_history: Vec::new(),
            latest_prices: HashMap::new(),
            holdings: Vec::new(),
            coin_chart_cache: HashMap::new(),
            account_balances: Vec::new(),
            net_worth_history: NetWorthSeries::default(),
            monthly: MonthlyData::default(),
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
        };

        app.refresh()?;
        Ok(app)
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.loading = true;
        self.status_msg = "Refreshing…".to_string();

        self.price_history = load_price_history(&self.journal_dir);
        self.latest_prices = latest_prices(&self.price_history);

        match load_crypto_balances(&self.journal_path) {
            Ok(balances) => {
                self.holdings = compute_portfolio(&balances, &self.latest_prices);
            }
            Err(e) => {
                self.status_msg = format!("Error loading crypto balances: {e}");
            }
        }

        self.coin_chart_cache.clear();
        for holding in &self.holdings {
            let series =
                load_coin_chart_series(&self.journal_path, &self.price_history, &holding.commodity);
            self.coin_chart_cache
                .insert(holding.commodity.clone(), series);
        }

        match load_account_balances_eur(&self.journal_path) {
            Ok(mut balances) => {
                balances.sort_by(|a, b| {
                    b.amount
                        .partial_cmp(&a.amount)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.account_balances = balances;
            }
            Err(e) => {
                self.status_msg = format!("Error loading account balances: {e}");
            }
        }

        match load_net_worth_history(&self.journal_path) {
            Ok(series) => {
                self.net_worth_history = series;
            }
            Err(e) => {
                self.status_msg = format!("Error loading net worth history: {e}");
            }
        }

        match load_monthly_data(&self.journal_path) {
            Ok(monthly) => {
                self.monthly = monthly;
            }
            Err(e) => {
                self.status_msg = format!("Error loading monthly data: {e}");
            }
        }

        // Clamp selections
        if !self.holdings.is_empty() && self.selected_holding >= self.holdings.len() {
            self.selected_holding = self.holdings.len() - 1;
        }

        self.last_refresh = Instant::now();
        let now = Local::now().format("%H:%M:%S");
        self.status_msg = format!("Updated at {now}");
        self.loading = false;
        Ok(())
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
