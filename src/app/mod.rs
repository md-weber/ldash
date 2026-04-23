mod combined;
mod detail;
mod export;
mod insights;
mod mouse;
mod refresh;
mod scroll;
mod types;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

pub use types::*;

use anyhow::Result;
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Instant, SystemTime};

use crate::config::Config;
use crate::data::{
    AccountBalance, CoinChartSeries, CryptoHolding, MonthlyData, NetWorthBreakdownSeries,
    NetWorthSeries, PriceEntry, Transaction,
};
use crate::watcher::JournalWatcher;

pub struct App {
    pub journal_path: PathBuf,
    pub journal_dir: PathBuf,
    pub config: Config,
    pub tab: Tab,
    pub has_crypto: bool,
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: Vec<CryptoHolding>,
    pub coin_chart_cache: HashMap<String, CoinChartSeries>,
    pub account_balances: Vec<AccountBalance>,
    pub liabilities: Vec<AccountBalance>,
    pub net_worth_history: NetWorthSeries,
    pub net_worth_breakdown: NetWorthBreakdownSeries,
    pub nw_range: NetWorthRange,
    pub monthly: MonthlyData,
    pub last_year: MonthlyData,
    /// Full-year data from `hledger --forecast`; future months hold periodic
    /// projections, past/current months hold actual data (same as `monthly`).
    pub monthly_forecast: MonthlyData,
    /// Merged calendar-order list: actual months first, then forecast months
    /// for the remaining months of the current year. Each entry carries a flag
    /// indicating whether the month is projected (`true`) or actual (`false`).
    pub combined_months: Vec<(crate::data::SingleMonth, bool)>,
    /// Index into `combined_months` for the currently viewed month.
    pub combined_selected: usize,
    pub monthly_year_offset: i32,
    pub selected_holding: usize,
    pub monthly_focus: MonthlyFocus,
    pub account_state: TableState,
    pub expense_state: TableState,
    pub income_state: TableState,
    pub account_detail: Option<Vec<Transaction>>,
    pub detail_account_name: Option<String>,
    pub expense_detail: Option<Vec<Transaction>>,
    pub detail_expense_name: Option<String>,
    pub income_detail: Option<Vec<Transaction>>,
    pub detail_income_name: Option<String>,
    pub detail_state: TableState,
    pub portfolio_range: PortfolioRange,
    pub chart_mode: ChartMode,
    pub expense_colors: bool,
    pub status_msg: String,
    pub loading: bool,
    pub show_help: bool,
    pub account_filter: String,
    pub account_filter_active: bool,
    pub search_active: bool,
    pub search_query: String,
    pub export_prompt_active: bool,
    pub export_prompt_path: String,
    pub search_results: Vec<Transaction>,
    pub search_state: TableState,
    pub price_alerts: Vec<PriceAlert>,
    pub show_alerts: bool,
    pub alert_dismissed: bool,
    pub alert_shown_at: Option<Instant>,
    pub last_refresh: Instant,
    pub tabs_loaded: TabFlags,
    /// Geometry written by the render pass, used for mouse hit-testing.
    pub geometry: Geometry,
    pub portfolio_scroll_offset: usize,
    pub(super) refresh_rx: Option<mpsc::Receiver<RefreshResult>>,
    /// Refresh request received while another refresh is in flight. Re-fired
    /// by `apply_refresh` once the current one completes, so pressing `r`
    /// during a refresh isn't silently dropped.
    pub(super) pending_refresh: Option<TabFlags>,
    pub(super) account_detail_rx: Option<mpsc::Receiver<DetailLoad>>,
    pub(super) expense_detail_rx: Option<mpsc::Receiver<DetailLoad>>,
    pub(super) income_detail_rx: Option<mpsc::Receiver<DetailLoad>>,
    pub(super) monthly_year_rx: Option<mpsc::Receiver<MonthlyYearLoad>>,
    pub(super) net_worth_rx: Option<mpsc::Receiver<NetWorthLoad>>,
    pub(crate) watcher: Option<JournalWatcher>,
    last_journal_mtime: Option<SystemTime>,
    last_config_mtime: Option<SystemTime>,
}

/// Background-thread payload for an account/expense/income detail load.
pub struct DetailLoad {
    pub name: String,
    pub result: Result<Vec<Transaction>, String>,
}

/// Background-thread payload for a yearly monthly-data reload.
pub struct MonthlyYearLoad {
    pub year_offset: i32,
    pub result: Result<MonthlyData, String>,
}

/// Background-thread payload for a net-worth chart reload (history + the
/// optional stacked breakdown).
pub struct NetWorthLoad {
    pub history: Result<NetWorthSeries, String>,
    pub breakdown: Option<NetWorthBreakdownSeries>,
}

impl Default for App {
    /// Cheap, in-process default — does **not** spawn watchers, read the
    /// filesystem, or invoke `Config::config_mtime()`. Intended for tests
    /// (`#[cfg(test)] fixtures.rs` builds on top via `..Default::default()`).
    /// Production code must go through `App::new` instead.
    fn default() -> Self {
        Self {
            journal_path: PathBuf::from("/tmp/test.journal"),
            journal_dir: PathBuf::from("/tmp"),
            config: Config::default(),
            tab: Tab::Accounts,
            has_crypto: false,
            price_history: Vec::new(),
            latest_prices: HashMap::new(),
            holdings: Vec::new(),
            coin_chart_cache: HashMap::new(),
            account_balances: Vec::new(),
            liabilities: Vec::new(),
            net_worth_history: NetWorthSeries::default(),
            net_worth_breakdown: NetWorthBreakdownSeries::default(),
            nw_range: NetWorthRange::All,
            monthly: MonthlyData::default(),
            last_year: MonthlyData::default(),
            monthly_forecast: MonthlyData::default(),
            combined_months: Vec::new(),
            combined_selected: 0,
            monthly_year_offset: 0,
            selected_holding: 0,
            monthly_focus: MonthlyFocus::default(),
            account_state: TableState::default().with_selected(0),
            expense_state: TableState::default().with_selected(0),
            income_state: TableState::default().with_selected(0),
            account_detail: None,
            detail_account_name: None,
            expense_detail: None,
            detail_expense_name: None,
            income_detail: None,
            detail_income_name: None,
            detail_state: TableState::default(),
            portfolio_range: PortfolioRange::All,
            chart_mode: ChartMode::Stacked,
            expense_colors: true,
            status_msg: String::new(),
            loading: false,
            show_help: false,
            account_filter: String::new(),
            account_filter_active: false,
            search_active: false,
            search_query: String::new(),
            export_prompt_active: false,
            export_prompt_path: String::new(),
            search_results: Vec::new(),
            search_state: TableState::default(),
            price_alerts: Vec::new(),
            show_alerts: false,
            alert_dismissed: false,
            alert_shown_at: None,
            last_refresh: Instant::now(),
            tabs_loaded: TabFlags::none(),
            geometry: Geometry::default(),
            portfolio_scroll_offset: 0,
            refresh_rx: None,
            pending_refresh: None,
            account_detail_rx: None,
            expense_detail_rx: None,
            income_detail_rx: None,
            monthly_year_rx: None,
            net_worth_rx: None,
            watcher: None,
            last_journal_mtime: None,
            last_config_mtime: None,
        }
    }
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

        let default_tab = match config.default_tab.as_str() {
            "portfolio" => Tab::Portfolio,
            "monthly" => Tab::Monthly,
            _ => Tab::Accounts,
        };
        let chart_mode = ChartMode::from_config(&config.chart_mode);

        Ok(Self {
            journal_path,
            journal_dir,
            config,
            tab: default_tab,
            has_crypto: true,
            price_history: Vec::new(),
            latest_prices: HashMap::new(),
            holdings: Vec::new(),
            coin_chart_cache: HashMap::new(),
            account_balances: Vec::new(),
            liabilities: Vec::new(),
            net_worth_history: NetWorthSeries::default(),
            net_worth_breakdown: NetWorthBreakdownSeries::default(),
            nw_range: NetWorthRange::All,
            monthly: MonthlyData::default(),
            last_year: MonthlyData::default(),
            monthly_forecast: MonthlyData::default(),
            combined_months: Vec::new(),
            combined_selected: 0,
            monthly_year_offset: 0,
            selected_holding: 0,
            monthly_focus: MonthlyFocus::default(),
            account_state: TableState::default().with_selected(0),
            expense_state: TableState::default().with_selected(0),
            income_state: TableState::default().with_selected(0),
            account_detail: None,
            detail_account_name: None,
            expense_detail: None,
            detail_expense_name: None,
            income_detail: None,
            detail_income_name: None,
            detail_state: TableState::default(),
            portfolio_range: PortfolioRange::All,
            chart_mode,
            expense_colors: true,
            status_msg: "Loading data…".to_string(),
            loading: true,
            show_help: false,
            account_filter: String::new(),
            account_filter_active: false,
            search_active: false,
            search_query: String::new(),
            export_prompt_active: false,
            export_prompt_path: String::new(),
            search_results: Vec::new(),
            search_state: TableState::default(),
            price_alerts: Vec::new(),
            show_alerts: false,
            alert_dismissed: false,
            alert_shown_at: None,
            last_refresh: Instant::now(),
            tabs_loaded: TabFlags::none(),
            geometry: Geometry::default(),
            portfolio_scroll_offset: 0,
            refresh_rx: None,
            pending_refresh: None,
            account_detail_rx: None,
            expense_detail_rx: None,
            income_detail_rx: None,
            monthly_year_rx: None,
            net_worth_rx: None,
            watcher,
            last_journal_mtime: None,
            last_config_mtime: Config::config_mtime(),
        })
    }

    pub fn visible_tabs(&self) -> Vec<Tab> {
        let mut v = vec![Tab::Accounts, Tab::Monthly];
        if self.crypto_enabled() {
            v.push(Tab::Portfolio);
        }
        v
    }

    pub fn crypto_enabled(&self) -> bool {
        self.config.show_portfolio.unwrap_or(self.has_crypto)
    }

    pub fn next_tab(&mut self) {
        if self.tab == Tab::Accounts {
            self.close_account_filter();
        }
        let tabs = self.visible_tabs();
        let pos = tabs.iter().position(|&t| t == self.tab).unwrap_or(0);
        self.tab = tabs[(pos + 1) % tabs.len()];
        self.ensure_tab_loaded(self.tab);
    }

    pub fn prev_tab(&mut self) {
        if self.tab == Tab::Accounts {
            self.close_account_filter();
        }
        let tabs = self.visible_tabs();
        let pos = tabs.iter().position(|&t| t == self.tab).unwrap_or(0);
        self.tab = tabs[(pos + tabs.len() - 1) % tabs.len()];
        self.ensure_tab_loaded(self.tab);
    }

    pub fn select_tab(&mut self, idx: usize) {
        if self.tab == Tab::Accounts {
            self.close_account_filter();
        }
        if let Some(&t) = self.visible_tabs().get(idx) {
            self.tab = t;
            self.ensure_tab_loaded(t);
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
}
