use chrono::{Datelike, Local, NaiveDate};
use std::collections::HashMap;

use crate::data::{
    AccountBalance, CoinChartSeries, CryptoHolding, MonthlyData, NetWorthBreakdownSeries,
    NetWorthSeries, PriceEntry,
};

/// Per-tab load result. Distinguishes "not requested", "ok data", and "errored
/// (keep stale data, surface message in status bar)".
pub enum TabData<T> {
    NotRequested,
    Ok(T),
    Err(String),
}

pub struct RefreshResult {
    pub tabs: [bool; 3],
    pub price_history: Vec<PriceEntry>,
    pub latest_prices: HashMap<String, f64>,
    pub holdings: TabData<Vec<CryptoHolding>>,
    pub coin_chart_cache: TabData<HashMap<String, CoinChartSeries>>,
    pub account_balances: TabData<Vec<AccountBalance>>,
    pub liabilities: TabData<Vec<AccountBalance>>,
    pub net_worth_history: TabData<NetWorthSeries>,
    pub net_worth_breakdown: TabData<NetWorthBreakdownSeries>,
    pub monthly: TabData<MonthlyData>,
    pub last_year: TabData<MonthlyData>,
    /// Full-year income statement with periodic-rule projections for future
    /// months, loaded via `hledger --forecast`.
    pub monthly_forecast: TabData<MonthlyData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MonthlyFocus {
    Income,
    #[default]
    Expenses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Portfolio,
    Accounts,
    Monthly,
}

impl Tab {
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

pub struct RecurringExpense {
    pub name: String,
    pub monthly_avg: f64,
    #[allow(dead_code)]
    pub occurrences: usize,
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

/// Returns true if expense `name` (e.g. "expenses:abos:youtube premium")
/// falls under budget `category` (e.g. "expenses:abos" or "abos").
pub fn budget_matches(category: &str, name: &str) -> bool {
    let cat_full = if category.starts_with("expenses:") {
        category.to_lowercase()
    } else {
        format!("expenses:{}", category.to_lowercase())
    };
    let name_lower = name.to_lowercase();
    name_lower == cat_full || name_lower.starts_with(&format!("{}:", cat_full))
}

/// Sum of all leaf expenses matching `category`. Leaf = no child entry present
/// in `expenses`. Prevents double-counting when hledger emits both a parent
/// account and its sub-accounts (both carry the same aggregated amount).
pub fn budget_spent(category: &str, expenses: &[(String, f64)]) -> f64 {
    expenses
        .iter()
        .filter(|(name, _)| budget_matches(category, name))
        .filter(|(name, _)| {
            let name_lower = name.to_lowercase();
            !expenses.iter().any(|(other, _)| {
                other
                    .to_lowercase()
                    .starts_with(&format!("{}:", name_lower))
            })
        })
        .map(|(_, amount)| *amount)
        .sum()
}

pub fn month_name_to_period(month_name: &str, year: i32) -> String {
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
