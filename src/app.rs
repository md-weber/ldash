use anyhow::Result;
use chrono::Local;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::data::{
    compute_portfolio, latest_prices, load_account_balances_eur, load_coin_chart_series,
    load_crypto_balances, load_monthly_data, load_net_worth_history, load_price_history,
    AccountBalance, CoinChartSeries, CryptoHolding, MonthlyData, NetWorthSeries, PriceEntry,
    SingleMonth,
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
    pub account_scroll: usize,
    pub expense_scroll: usize,
    pub status_msg: String,
    pub loading: bool,
    pub show_help: bool,
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
            account_scroll: 0,
            expense_scroll: 0,
            status_msg: "Loading data…".to_string(),
            loading: true,
            show_help: false,
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
            let series = load_coin_chart_series(
                &self.journal_path,
                &self.price_history,
                &holding.commodity,
            );
            self.coin_chart_cache.insert(holding.commodity.clone(), series);
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
                self.account_scroll = self.account_scroll.saturating_sub(1);
            }
            Tab::Monthly => {
                self.expense_scroll = self.expense_scroll.saturating_sub(1);
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
                if self.account_scroll + 1 < self.account_balances.len() {
                    self.account_scroll += 1;
                }
            }
            Tab::Monthly => {
                let len = self
                    .current_month()
                    .map(|m| m.expenses.len())
                    .unwrap_or(0);
                if self.expense_scroll + 1 < len {
                    self.expense_scroll += 1;
                }
            }
        }
    }

    pub fn selected_coin(&self) -> Option<&str> {
        self.holdings.get(self.selected_holding).map(|h| h.commodity.as_str())
    }

    pub fn total_portfolio_value(&self) -> f64 {
        self.holdings.iter().map(|h| h.value_eur).sum()
    }

    pub fn total_net_worth(&self) -> f64 {
        self.account_balances.iter().map(|b| b.amount).sum()
    }

    pub fn current_month(&self) -> Option<&SingleMonth> {
        self.monthly.months.get(self.monthly.selected)
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
}
