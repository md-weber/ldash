use chrono::{Datelike, Local, NaiveDate};
use std::collections::HashMap;

use crate::data::{
    AccountBalance, CoinChartSeries, CryptoHolding, LiabilityProgress, LiquidAccountsMonthly,
    LiquidCashMonthly, MonthlyData, NetWorthBreakdownSeries, NetWorthSeries, PayeeSummary,
    PriceEntry,
};

/// Per-tab load result. Distinguishes "not requested", "ok data", and "errored
/// (keep stale data, surface message in status bar)".
pub enum TabData<T> {
    NotRequested,
    Ok(T),
    Err(String),
}

pub struct RefreshResult {
    pub tabs: TabFlags,
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: TabData<Vec<CryptoHolding>>,
    pub coin_chart_cache: TabData<HashMap<String, CoinChartSeries>>,
    pub account_balances: TabData<Vec<AccountBalance>>,
    pub liabilities: TabData<Vec<AccountBalance>>,
    /// Payoff progress analytics for each liability account.
    pub liability_progress: TabData<Vec<LiabilityProgress>>,
    pub net_worth_history: TabData<NetWorthSeries>,
    pub net_worth_breakdown: TabData<NetWorthBreakdownSeries>,
    pub monthly: TabData<MonthlyData>,
    pub last_year: TabData<MonthlyData>,
    /// Full-year income statement with periodic-rule projections for future
    /// months, loaded via `hledger --forecast`.
    pub monthly_forecast: TabData<MonthlyData>,
    /// Payee analytics for the Monthly tab's `p` sub-view.
    pub payee_data: TabData<Vec<PayeeSummary>>,
    /// Per-month net change in liquid cash (whitelisted accounts only) for
    /// the current calendar year. Empty when `liquid_accounts` is unset.
    pub liquid_cash_monthly: TabData<LiquidCashMonthly>,
    /// Per-account monthly net change for the same period as `liquid_cash_monthly`.
    pub liquid_accounts_monthly: TabData<LiquidAccountsMonthly>,
    /// Same series with `--forecast`. Non-fatal if the journal has no
    /// periodic rules.
    pub liquid_cash_forecast: TabData<LiquidCashMonthly>,
    /// Per-account forecast monthly series.
    pub liquid_accounts_forecast: TabData<LiquidAccountsMonthly>,
}

/// How the portfolio analysis chart layers price-growth and staking on top of
/// the investment cost basis. Mirrors the `chart_mode` config string and the
/// `s` toggle key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartMode {
    Stacked,
    Unstacked,
}

impl ChartMode {
    pub fn is_stacked(self) -> bool {
        matches!(self, Self::Stacked)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Stacked => "stacked",
            Self::Unstacked => "unstacked",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Stacked => Self::Unstacked,
            Self::Unstacked => Self::Stacked,
        }
    }

    pub fn from_config(s: &str) -> Self {
        if s == "unstacked" {
            Self::Unstacked
        } else {
            Self::Stacked
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonthlyFocus {
    Income,
    #[default]
    Expenses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Portfolio,
    Accounts,
    Monthly,
    Register,
}

/// Per-tab boolean flags with named accessors. Replaces the older `[bool; 3]`
/// indexed by `Tab::index()` — fewer off-by-index bugs and self-documenting
/// field names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TabFlags {
    pub portfolio: bool,
    pub accounts: bool,
    pub monthly: bool,
}

impl TabFlags {
    pub const fn all(value: bool) -> Self {
        Self {
            portfolio: value,
            accounts: value,
            monthly: value,
        }
    }

    pub const fn none() -> Self {
        Self::all(false)
    }

    pub fn get(self, tab: Tab) -> bool {
        match tab {
            Tab::Dashboard => self.accounts && self.monthly,
            Tab::Portfolio => self.portfolio,
            Tab::Accounts => self.accounts,
            Tab::Monthly => self.monthly,
            Tab::Register => false,
        }
    }

    pub fn set(&mut self, tab: Tab, value: bool) {
        match tab {
            Tab::Dashboard => {
                self.accounts = value;
                self.monthly = value;
            }
            Tab::Portfolio => self.portfolio = value,
            Tab::Accounts => self.accounts = value,
            Tab::Monthly => self.monthly = value,
            Tab::Register => {}
        }
    }

    pub fn any(self) -> bool {
        self.portfolio || self.accounts || self.monthly
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

#[derive(Debug)]
pub struct RecurringExpense {
    pub name: String,
    pub monthly_avg: f64,
}

/// Income/expense split for the Dashboard tab month summary.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MonthBreakdown {
    pub total_income: f64,
    pub recurring_income: f64,
    pub recurring_expenses: f64,
    pub other_expenses: f64,
    pub total_expenses: f64,
}

#[derive(Debug, Clone)]
pub struct CategoryShare {
    pub name: String,
    pub amount: f64,
    pub fraction: f64,
}

#[derive(Debug, Clone, Default)]
pub struct CashOverview {
    pub accounts: Vec<(String, f64)>,
    pub total_balance: f64,
    pub ytd_net_change: f64,
}

/// Render-pass geometry written by `ui::*` and read back by mouse hit-testing.
/// Grouped together so `App` doesn't carry seven loose `Rect`/`Vec<Rect>`
/// fields; everything UI-only lives behind `app.geometry.*`.
#[derive(Debug, Default, Clone)]
pub struct Geometry {
    pub tab_bar_area: ratatui::layout::Rect,
    pub tab_rects: Vec<ratatui::layout::Rect>,
    pub table_area: ratatui::layout::Rect,
    pub liability_table_area: ratatui::layout::Rect,
    pub income_table_area: ratatui::layout::Rect,
    pub expense_table_area: ratatui::layout::Rect,
    pub monthly_chart_area: ratatui::layout::Rect,
    pub range_selector_rects: Vec<ratatui::layout::Rect>,
    pub payee_table_area: ratatui::layout::Rect,
}

/// 12-month cash flow forecast for the current year.
/// `actuals` holds (month_idx 0-11, net) for months with real data.
/// `projected` holds (month_idx, projected_net) for future months, with the
/// last actual point prepended so the forecast line starts where actuals end.
pub struct CashFlowForecast {
    pub actuals: Vec<(f64, f64)>,
    pub projected: Vec<(f64, f64)>,
    /// Projected monthly net used for all future months.
    pub projected_monthly_net: f64,
    pub min_y: f64,
    pub max_y: f64,
}

pub fn month_name_to_period(month_name: &str, year: i32) -> String {
    let month_num = crate::data::month_index(month_name).unwrap_or(1);
    format!("{year}-{month_num:02}")
}
