mod combined;
mod dashboard;
mod detail;
mod export;
mod insights;
mod mouse;
mod refresh;
mod register;
mod scroll;
mod types;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

pub use types::*;
pub use dashboard::LiquidAccountChart;

use anyhow::Result;
use ratatui::widgets::TableState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Instant, SystemTime};

use crate::config::Config;
use crate::data::{
    AccountBalance, CoinChartSeries, CryptoHolding, LiabilityProgress, MonthlyData,
    NetWorthBreakdownSeries, NetWorthSeries, PayeeSummary, PriceEntry, RegisterQuery, Transaction,
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
    /// Payoff-progress analytics, keyed by account name. Loaded in parallel
    /// with `liabilities`; may lag one render cycle on first load.
    pub liability_progress: Vec<LiabilityProgress>,
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
    pub liability_state: TableState,
    /// Whether keyboard focus is on the liabilities table vs the assets table.
    pub liability_focus: bool,
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
    pub export_prompt_active: bool,
    pub export_prompt_path: String,
    pub file_prompt_active: bool,
    pub file_prompt_path: String,
    /// Selected index within the combined journal list when the picker is open.
    pub file_prompt_journal_idx: Option<usize>,
    /// Journals opened this session (most-recent first). Seeded with the
    /// startup journal; extended by every successful `confirm_file_prompt`.
    /// Session-only — never written to disk.
    pub recent_journals: Vec<String>,
    /// Whether the payee analytics sub-view is active on the Monthly tab (`p`).
    pub payee_view: bool,
    /// Whether the YoY comparison bar chart is shown below the monthly chart (`C`).
    pub yoy_view: bool,
    /// When true, the current calendar month is shown with actuals + periodic-rule
    /// projections from today to end of month (toggled with `F`).
    pub show_current_month_forecast: bool,
    pub payee_data: Vec<PayeeSummary>,
    pub payee_state: TableState,
    /// Per-month net change in liquid cash (whitelisted accounts only) for
    /// the current calendar year, posted transactions only. Empty when
    /// `config.liquid_accounts` is unset or the load failed.
    pub liquid_cash_monthly: Vec<(String, f64)>,
    /// Per-account monthly net change for liquid accounts.
    pub liquid_accounts_monthly: Vec<(String, Vec<(String, f64)>)>,
    /// Same series with `--forecast` from today through year-end. Used when
    /// the selected month is a forecast month (`F` on the current month, or
    /// a future month).
    pub liquid_cash_forecast: Vec<(String, f64)>,
    pub liquid_accounts_forecast: Vec<(String, Vec<(String, f64)>)>,
    pub register_query: RegisterQuery,
    pub register_draft: String,
    pub register_txns: Vec<crate::data::RegisterTxn>,
    pub register_rows: Vec<crate::data::RegisterViewRow>,
    pub register_table: TableState,
    pub register_focus_query: bool,
    pub register_error: Option<String>,
    pub register_detail: Option<crate::data::RegisterTxn>,
    /// First `g` of a pending `gg` jump-to-top on the Register tab.
    pub(super) register_g_pending: bool,
    /// When Register was opened via Enter from Accounts/Monthly, ESC returns
    /// here instead of quitting. Cleared by explicit tab navigation (`3`, Tab).
    pub register_return_tab: Option<Tab>,
    /// Scope of the last successful Register load. Account and description
    /// filters are applied in memory against this page.
    pub(super) register_loaded_key: Option<crate::data::RegisterLoadKey>,
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
    pub(super) register_rx: Option<mpsc::Receiver<RegisterLoad>>,
    pub(super) monthly_year_rx: Option<mpsc::Receiver<MonthlyYearLoad>>,
    pub(super) net_worth_rx: Option<mpsc::Receiver<NetWorthLoad>>,
    pub(crate) watcher: Option<JournalWatcher>,
    last_journal_mtime: Option<SystemTime>,
    last_config_mtime: Option<SystemTime>,
    /// Receiver for the background price-fetch thread result.
    pub(super) price_fetch_rx: Option<mpsc::Receiver<Result<String, String>>>,
}

/// Background-thread payload for an account/expense/income detail load.
pub struct DetailLoad {
    pub name: String,
    pub result: Result<Vec<Transaction>, String>,
}

/// Background-thread payload for a Register tab load.
pub struct RegisterLoad {
    pub query: RegisterQuery,
    pub result: Result<Vec<crate::data::RegisterTxn>, String>,
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
            liability_progress: Vec::new(),
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
            liability_state: TableState::default().with_selected(0),
            liability_focus: false,
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
            export_prompt_active: false,
            export_prompt_path: String::new(),
            file_prompt_active: false,
            file_prompt_path: String::new(),
            file_prompt_journal_idx: None,
            recent_journals: Vec::new(),
            payee_view: false,
            yoy_view: false,
            show_current_month_forecast: false,
            payee_data: Vec::new(),
            payee_state: TableState::default(),
            liquid_cash_monthly: Vec::new(),
            liquid_accounts_monthly: Vec::new(),
            liquid_cash_forecast: Vec::new(),
            liquid_accounts_forecast: Vec::new(),
            register_query: RegisterQuery::current_month(),
            register_draft: String::new(),
            register_txns: Vec::new(),
            register_rows: Vec::new(),
            register_table: TableState::default(),
            register_focus_query: false,
            register_error: None,
            register_detail: None,
            register_g_pending: false,
            register_return_tab: None,
            register_loaded_key: None,
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
            register_rx: None,
            monthly_year_rx: None,
            net_worth_rx: None,
            watcher: None,
            last_journal_mtime: None,
            last_config_mtime: None,
            price_fetch_rx: None,
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
            "start" | "dashboard" => Tab::Dashboard,
            "portfolio" => Tab::Portfolio,
            "monthly" => Tab::Monthly,
            "register" => Tab::Register,
            _ => Tab::Accounts,
        };
        let chart_mode = ChartMode::from_config(&config.chart_mode);
        let forecast_on_dashboard = default_tab == Tab::Dashboard;
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
            liability_progress: Vec::new(),
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
            liability_state: TableState::default().with_selected(0),
            liability_focus: false,
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
            export_prompt_active: false,
            export_prompt_path: String::new(),
            file_prompt_active: false,
            file_prompt_path: String::new(),
            file_prompt_journal_idx: None,
            recent_journals: Vec::new(),
            payee_view: false,
            yoy_view: false,
            show_current_month_forecast: forecast_on_dashboard,
            payee_data: Vec::new(),
            payee_state: TableState::default(),
            liquid_cash_monthly: Vec::new(),
            liquid_accounts_monthly: Vec::new(),
            liquid_cash_forecast: Vec::new(),
            liquid_accounts_forecast: Vec::new(),
            register_query: RegisterQuery::current_month(),
            register_draft: String::new(),
            register_txns: Vec::new(),
            register_rows: Vec::new(),
            register_table: TableState::default(),
            register_focus_query: false,
            register_error: None,
            register_detail: None,
            register_g_pending: false,
            register_return_tab: None,
            register_loaded_key: None,
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
            register_rx: None,
            monthly_year_rx: None,
            net_worth_rx: None,
            watcher,
            last_journal_mtime: None,
            last_config_mtime: Config::config_mtime(),
            price_fetch_rx: None,
        })
    }

    pub fn visible_tabs(&self) -> Vec<Tab> {
        let mut v = vec![Tab::Dashboard, Tab::Accounts, Tab::Monthly, Tab::Register];
        if self.portfolio_tab_visible() {
            v.push(Tab::Portfolio);
        }
        v
    }

    pub fn switch_tab(&mut self, tab: Tab) {
        if self.tab == Tab::Accounts {
            self.close_account_filter();
            self.liability_focus = false;
        }
        if tab == Tab::Register {
            self.register_return_tab = None;
        } else {
            self.register_g_pending = false;
        }
        self.tab = tab;
        if tab == Tab::Dashboard {
            self.ensure_dashboard_view();
        }
        self.ensure_tab_loaded(tab);
    }

    /// Whether the Portfolio tab appears in the tab bar.
    /// Defaults to `true` (always visible) — use `show_portfolio = false` in
    /// config to explicitly hide it.
    pub fn portfolio_tab_visible(&self) -> bool {
        self.config.show_portfolio.unwrap_or(true)
    }

    /// Whether to actually load and display portfolio data (i.e. the tab is
    /// visible AND we either have detected holdings or the user forced it on).
    /// Used by the refresh plumbing to decide whether to spawn the crypto
    /// data loaders.
    pub fn crypto_enabled(&self) -> bool {
        self.config.show_portfolio.unwrap_or(true)
    }

    pub fn next_tab(&mut self) {
        if self.tab == Tab::Accounts {
            self.close_account_filter();
            self.liability_focus = false;
        }
        let tabs = self.visible_tabs();
        let pos = tabs.iter().position(|&t| t == self.tab).unwrap_or(0);
        self.tab = tabs[(pos + 1) % tabs.len()];
        self.ensure_tab_loaded(self.tab);
    }

    pub fn prev_tab(&mut self) {
        if self.tab == Tab::Accounts {
            self.close_account_filter();
            self.liability_focus = false;
        }
        let tabs = self.visible_tabs();
        let pos = tabs.iter().position(|&t| t == self.tab).unwrap_or(0);
        self.tab = tabs[(pos + tabs.len() - 1) % tabs.len()];
        self.ensure_tab_loaded(self.tab);
    }

    pub fn select_tab(&mut self, idx: usize) {
        if let Some(&t) = self.visible_tabs().get(idx) {
            self.switch_tab(t);
        }
    }

    /// Combined ordered list shown in the journal picker.
    /// Recent (session) journals come first, followed by any entries in
    /// `config.journals` that aren't already in the recent list.
    pub fn picker_journals(&self) -> Vec<String> {
        let mut out = self.recent_journals.clone();
        for j in &self.config.journals {
            if !out.contains(j) {
                out.push(j.clone());
            }
        }
        out
    }

    pub fn open_file_prompt(&mut self) {
        self.file_prompt_path.clear();
        self.file_prompt_journal_idx = None;
        self.file_prompt_active = true;
    }

    pub fn cancel_file_prompt(&mut self) {
        self.file_prompt_active = false;
        self.file_prompt_path.clear();
        self.file_prompt_journal_idx = None;
    }

    /// Persist the path currently typed in the journal-switch prompt to
    /// `~/.config/ldash/config.toml` as the new `journal = "..."` value.
    ///
    /// If the prompt is empty, the *currently active* journal is saved so
    /// `Ctrl-O` → `Ctrl-S` is a one-keystroke "remember this journal" action
    /// after the file has already been opened.
    ///
    /// The session's in-memory `config.journal` is updated to match so
    /// subsequent hot-reloads don't overwrite the value with stale state.
    /// The actual journal switch is **not** performed here — call
    /// `confirm_file_prompt` separately when the user wants to both save
    /// and switch.
    pub fn save_journal_to_config(&mut self) {
        let raw = self.file_prompt_path.trim().to_string();
        let target = if raw.is_empty() {
            self.journal_path.to_string_lossy().to_string()
        } else {
            expand_tilde(&raw)
        };

        if target.is_empty() {
            self.status_msg = "No journal to save".to_string();
            return;
        }

        match crate::config::Config::persist_journal(&target) {
            Ok(path) => {
                self.config.journal = Some(target.clone());
                self.file_prompt_active = false;
                self.file_prompt_path.clear();
                self.file_prompt_journal_idx = None;
                self.status_msg = format!("Saved journal to {}", path.display());
            }
            Err(e) => {
                self.status_msg = format!("Save error: {e}");
            }
        }
    }

    /// Tab-complete the current `file_prompt_path` against the filesystem.
    ///
    /// Behaviour:
    /// - If the path is empty and `config.journals` is non-empty, falls back
    ///   to journal cycling (same as pressing ↓).
    /// - Otherwise expands `~/` and lists the parent directory for entries
    ///   whose name starts with the typed prefix:
    ///   - 0 matches → no change.
    ///   - 1 match   → completes the path; appends `/` for directories.
    ///   - N matches → completes to the longest common prefix; appends `/`
    ///     when that prefix is itself a directory.
    pub fn file_prompt_tab_complete(&mut self) {
        if self.file_prompt_path.is_empty() {
            if !self.picker_journals().is_empty() {
                self.file_prompt_next_journal();
            }
            return;
        }

        let expanded = expand_tilde(&self.file_prompt_path);

        // Split into (dir, stem): for "/home/user/Fi" → ("/home/user", "Fi")
        let path = std::path::Path::new(&expanded);
        let (dir, stem): (std::path::PathBuf, String) = if expanded.ends_with('/') {
            (path.to_path_buf(), String::new())
        } else {
            let parent = path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let file = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            (parent, file)
        };

        let entries: Vec<(String, bool)> = match std::fs::read_dir(&dir) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with(stem.as_str()))
                        .unwrap_or(false)
                })
                .map(|e| {
                    let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let name = e.file_name().to_string_lossy().to_string();
                    (name, is_dir)
                })
                .collect(),
            Err(_) => return,
        };

        if entries.is_empty() {
            return;
        }

        let completed_name = if entries.len() == 1 {
            let (name, is_dir) = &entries[0];
            if *is_dir {
                format!("{name}/")
            } else {
                name.clone()
            }
        } else {
            // Longest common prefix of all matching names
            let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
            let prefix = longest_common_prefix(&names);
            if prefix.len() <= stem.len() {
                // No additional characters to add
                return;
            }
            let completed_path = dir.join(prefix);
            if completed_path.is_dir() && prefix == names[0] {
                format!("{prefix}/")
            } else {
                prefix.to_string()
            }
        };

        // Re-assemble: keep the original prefix style (tilde vs absolute)
        let new_path = {
            let dir_str = dir.to_string_lossy();
            if dir_str == "." {
                completed_name
            } else {
                format!("{}/{}", dir_str.trim_end_matches('/'), completed_name)
            }
        };

        // Preserve tilde if the original input used it
        let new_path = if self.file_prompt_path.starts_with("~/") || self.file_prompt_path == "~" {
            if let Ok(home) = std::env::var("HOME") {
                if new_path.starts_with(&home) {
                    format!("~{}", &new_path[home.len()..])
                } else {
                    new_path
                }
            } else {
                new_path
            }
        } else {
            new_path
        };

        self.file_prompt_path = new_path;
        self.file_prompt_journal_idx = None;
    }

    /// Cycle to the next journal in the picker list.
    pub fn file_prompt_next_journal(&mut self) {
        let list = self.picker_journals();
        if list.is_empty() {
            return;
        }
        let next = match self.file_prompt_journal_idx {
            None => 0,
            Some(i) => (i + 1) % list.len(),
        };
        self.file_prompt_journal_idx = Some(next);
        self.file_prompt_path = list[next].clone();
    }

    /// Cycle to the previous journal in the picker list.
    pub fn file_prompt_prev_journal(&mut self) {
        let list = self.picker_journals();
        if list.is_empty() {
            return;
        }
        let prev = match self.file_prompt_journal_idx {
            None => list.len() - 1,
            Some(0) => list.len() - 1,
            Some(i) => i - 1,
        };
        self.file_prompt_journal_idx = Some(prev);
        self.file_prompt_path = list[prev].clone();
    }

    /// Attempt to switch to the path currently in `file_prompt_path`.
    /// Expands `~/` to the home directory. Returns `Ok(())` on success or
    /// an error message string on failure.  On success the journal path is
    /// updated in-memory (session-only; config is not written) and a full
    /// refresh is queued.
    pub fn confirm_file_prompt(&mut self) {
        let raw = self.file_prompt_path.trim().to_string();
        self.file_prompt_active = false;
        self.file_prompt_path.clear();

        let expanded = expand_tilde(&raw);
        let path = std::path::PathBuf::from(&expanded);

        if !path.exists() {
            self.status_msg = format!("File not found: {expanded}");
            return;
        }

        let journal_dir = path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();

        // Capture before overwriting so the old path lands in session history.
        let previous = self.journal_path.to_string_lossy().to_string();

        self.journal_path = path;
        self.journal_dir = journal_dir;
        self.tabs_loaded = TabFlags::none();
        self.watcher = crate::watcher::JournalWatcher::new(&self.journal_path);
        self.recent_journals
            .retain(|j| j != &expanded && j != &previous);
        self.recent_journals.insert(0, expanded.clone());
        if previous != expanded {
            self.recent_journals.insert(1, previous);
        }
        self.recent_journals.truncate(10);

        self.start_refresh();
        self.status_msg = format!("Switched to {expanded}");
    }

    pub fn selected_coin(&self) -> Option<&str> {
        self.holdings
            .get(self.selected_holding)
            .map(|h| h.commodity.as_str())
    }

    pub fn total_portfolio_value(&self) -> f64 {
        self.holdings.iter().map(|h| h.value_eur).sum()
    }

    /// Net change in liquid cash (whitelisted accounts) for the currently
    /// selected month on the Monthly tab. `None` when the feature is off
    /// (`liquid_accounts` unset) or the selected month has no matching data
    /// (e.g. a past year with no periodic history for this metric).
    ///
    /// Forecast remainder is included only when the selected month is a
    /// forecast month (`F` on the current calendar month, or a future month).
    pub fn liquid_cash_change_for_selected_month(&self) -> Option<f64> {
        let month_name = &self.current_month()?.month_name;
        if self.current_month_is_forecast() {
            if let Some((_, v)) = self
                .liquid_cash_forecast
                .iter()
                .find(|(name, _)| name == month_name)
            {
                return Some(*v);
            }
        }
        self.liquid_cash_monthly
            .iter()
            .find(|(name, _)| name == month_name)
            .map(|(_, v)| *v)
    }

    pub fn total_net_worth(&self) -> f64 {
        let assets: f64 = self.account_balances.iter().map(|b| b.amount).sum();
        let liabs: f64 = self.liabilities.iter().map(|b| b.amount).sum();
        assets + liabs
    }
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") || path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

fn longest_common_prefix<'a>(strs: &[&'a str]) -> &'a str {
    if strs.is_empty() {
        return "";
    }
    let first = strs[0];
    let mut len = first.len();
    for s in &strs[1..] {
        len = len.min(
            first
                .chars()
                .zip(s.chars())
                .take_while(|(a, b)| a == b)
                .count(),
        );
    }
    &first[..len]
}
