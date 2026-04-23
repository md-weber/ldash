use super::{App, Tab, TabFlags};

impl App {
    /// Empty fixture — every field at its default. Builds via `Default`, so
    /// adding a new field on `App` no longer requires touching this file.
    /// Test-mode overrides: deterministic colours off, all tabs marked
    /// loaded so `auto_refresh` behaves as if the data is already present.
    pub fn fixture_empty() -> Self {
        Self {
            status_msg: "Test mode".to_string(),
            expense_colors: false,
            tabs_loaded: TabFlags::all(true),
            ..Self::default()
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
        };
        app.rebuild_combined_months();
        app
    }
}
