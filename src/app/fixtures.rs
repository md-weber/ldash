use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::widgets::TableState;

use crate::config::Config;

use super::{App, MonthlyFocus, NetWorthRange, PortfolioRange, Tab};

impl App {
    pub fn fixture_empty() -> Self {
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
            net_worth_history: crate::data::NetWorthSeries::default(),
            net_worth_breakdown: crate::data::NetWorthBreakdownSeries::default(),
            nw_range: NetWorthRange::All,
            monthly: crate::data::MonthlyData::default(),
            last_year: crate::data::MonthlyData::default(),
            monthly_forecast: crate::data::MonthlyData::default(),
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
            chart_stacked: true,
            expense_colors: false,
            status_msg: "Test mode".to_string(),
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
            tabs_loaded: [true; 3],
            tab_bar_area: Rect::default(),
            tab_rects: Vec::new(),
            table_area: Rect::default(),
            income_table_area: Rect::default(),
            expense_table_area: Rect::default(),
            monthly_chart_area: Rect::default(),
            range_selector_rects: Vec::new(),
            portfolio_scroll_offset: 0,
            refresh_rx: None,
            watcher: None,
            last_journal_mtime: None,
            last_config_mtime: None,
        }
    }

    pub fn fixture_with_accounts() -> Self {
        let mut app = Self::fixture_empty();
        app.account_balances = vec![
            crate::data::AccountBalance {
                account: "assets:bank:checking".to_string(),
                amount: 5000.0,
                commodity: "€".to_string(),
            },
            crate::data::AccountBalance {
                account: "assets:savings".to_string(),
                amount: 10000.0,
                commodity: "€".to_string(),
            },
        ];
        app
    }

    pub fn fixture_with_monthly() -> Self {
        let mut app = Self::fixture_empty();
        app.tab = Tab::Monthly;
        app.monthly = crate::data::MonthlyData {
            months: vec![
                crate::data::SingleMonth {
                    month_name: "January".to_string(),
                    income: vec![("income:salary".to_string(), 3000.0)],
                    expenses: vec![
                        ("expenses:housing".to_string(), 1200.0),
                        ("expenses:food".to_string(), 500.0),
                    ],
                    total_income: 3000.0,
                    total_expenses: 1700.0,
                },
                crate::data::SingleMonth {
                    month_name: "February".to_string(),
                    income: vec![("income:salary".to_string(), 3000.0)],
                    expenses: vec![
                        ("expenses:housing".to_string(), 1200.0),
                        ("expenses:food".to_string(), 450.0),
                    ],
                    total_income: 3000.0,
                    total_expenses: 1650.0,
                },
            ],
            selected: 0,
        };
        app.rebuild_combined_months();
        app
    }
}
