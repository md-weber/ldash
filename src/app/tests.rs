use std::collections::HashMap;

use chrono::Local;

use super::*;

fn empty_refresh_result(tabs: TabFlags) -> RefreshResult {
    RefreshResult {
        tabs,
        price_history: Vec::new(),
        latest_prices: HashMap::new(),
        holdings: TabData::NotRequested,
        coin_chart_cache: TabData::NotRequested,
        account_balances: TabData::NotRequested,
        liabilities: TabData::NotRequested,
        net_worth_history: TabData::NotRequested,
        net_worth_breakdown: TabData::NotRequested,
        monthly: TabData::NotRequested,
        last_year: TabData::NotRequested,
        monthly_forecast: TabData::NotRequested,
    }
}

#[test]
fn apply_refresh_err_keeps_stale_data_and_sets_status() {
    let mut app = App::fixture_with_accounts();
    let stale = app.account_balances.clone();

    let mut r = empty_refresh_result(TabFlags {
        portfolio: false,
        accounts: true,
        monthly: false,
    });
    r.account_balances = TabData::Err("hledger: parse error line 42".to_string());

    app.apply_refresh(r);

    assert_eq!(
        app.account_balances.len(),
        stale.len(),
        "stale data should be preserved"
    );
    assert_eq!(app.account_balances[0].account, stale[0].account);
    assert!(
        app.status_msg.contains("parse error"),
        "status should contain error, got: {}",
        app.status_msg
    );
}

#[test]
fn apply_refresh_ok_overwrites_stale_data() {
    let mut app = App::fixture_with_accounts();
    let mut r = empty_refresh_result(TabFlags {
        portfolio: false,
        accounts: true,
        monthly: false,
    });
    r.account_balances = TabData::Ok(vec![crate::data::AccountBalance {
        account: "assets:new".to_string(),
        amount: 1.0,
        commodity: "€".to_string(),
    }]);

    app.apply_refresh(r);

    assert_eq!(app.account_balances.len(), 1);
    assert_eq!(app.account_balances[0].account, "assets:new");
}

#[test]
fn auto_refresh_missing_journal_sets_status_no_refresh() {
    let mut app = App::fixture_empty();
    app.journal_path = std::path::PathBuf::from("/tmp/does_not_exist_xyz.journal");
    app.tabs_loaded = TabFlags::none();

    app.auto_refresh();

    assert!(
        app.refresh_rx.is_none(),
        "should not start refresh when journal missing"
    );
    assert!(
        app.status_msg.starts_with("Journal not found"),
        "got: {}",
        app.status_msg
    );
}

#[test]
fn auto_refresh_missing_journal_does_not_overwrite_message() {
    let mut app = App::fixture_empty();
    app.journal_path = std::path::PathBuf::from("/tmp/does_not_exist_xyz.journal");
    app.status_msg =
        "Journal not found: /tmp/does_not_exist_xyz.journal — waiting for re-creation".to_string();

    app.auto_refresh();

    assert_eq!(
        app.status_msg,
        "Journal not found: /tmp/does_not_exist_xyz.journal — waiting for re-creation"
    );
}

#[test]
fn budget_matches_exact() {
    let cfg = crate::config::Config::default();
    assert!(cfg.budget_matches("expenses:food", "expenses:food"));
}

#[test]
fn budget_matches_child() {
    let cfg = crate::config::Config::default();
    assert!(cfg.budget_matches("expenses:food", "expenses:food:restaurants"));
}

#[test]
fn budget_matches_without_prefix() {
    let cfg = crate::config::Config::default();
    assert!(cfg.budget_matches("food", "expenses:food"));
}

#[test]
fn budget_no_match_sibling() {
    let cfg = crate::config::Config::default();
    assert!(!cfg.budget_matches("expenses:food", "expenses:transport"));
}

#[test]
fn budget_no_match_partial_name() {
    let cfg = crate::config::Config::default();
    assert!(!cfg.budget_matches("expenses:foo", "expenses:food"));
}

#[test]
fn budget_matches_custom_root() {
    let mut cfg = crate::config::Config::default();
    cfg.expenses_account = "Ausgaben".to_string();
    assert!(cfg.budget_matches("Ausgaben:Essen", "Ausgaben:Essen:Restaurant"));
    assert!(cfg.budget_matches("Essen", "Ausgaben:Essen"));
    assert!(!cfg.budget_matches("Essen", "expenses:food"));
}

#[test]
fn budget_matches_case_insensitive_root() {
    // "Expenses:Food" should match with default root "expenses"
    let cfg = crate::config::Config::default();
    assert!(cfg.budget_matches("expenses:food", "Expenses:Food"));
    assert!(cfg.budget_matches("food", "Expenses:Food:Restaurant"));
}

#[test]
fn budget_spent_sums_matching_leaves() {
    let cfg = crate::config::Config::default();
    let expenses = vec![
        ("expenses:food:restaurants".to_string(), 120.0),
        ("expenses:food:groceries".to_string(), 80.0),
        ("expenses:transport".to_string(), 50.0),
    ];
    let spent = cfg.budget_spent("expenses:food", &expenses);
    assert!((spent - 200.0).abs() < 0.01);
}

#[test]
fn budget_spent_skips_parent_when_child_present() {
    let cfg = crate::config::Config::default();
    let expenses = vec![
        ("expenses:food".to_string(), 200.0),
        ("expenses:food:groceries".to_string(), 80.0),
    ];
    let spent = cfg.budget_spent("expenses:food", &expenses);
    assert!(
        (spent - 80.0).abs() < 0.01,
        "should only count leaf, got {spent}"
    );
}

#[test]
fn budget_spent_zero_when_no_match() {
    let cfg = crate::config::Config::default();
    let expenses = vec![("expenses:transport".to_string(), 50.0)];
    let spent = cfg.budget_spent("expenses:food", &expenses);
    assert_eq!(spent, 0.0);
}

fn make_month(name: &str, expenses: Vec<(&str, f64)>) -> crate::data::SingleMonth {
    let total_expenses = expenses.iter().map(|(_, a)| a).sum();
    crate::data::SingleMonth {
        month_name: name.to_string(),
        income: vec![],
        expenses: expenses
            .into_iter()
            .map(|(n, a)| (n.to_string(), a))
            .collect(),
        total_income: 0.0,
        total_expenses,
    }
}

#[test]
fn recurring_expenses_detects_stable_subscription() {
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month("January", vec![("expenses:abos:netflix", 9.99)]),
            make_month("February", vec![("expenses:abos:netflix", 9.99)]),
            make_month("March", vec![("expenses:abos:netflix", 9.99)]),
            make_month("April", vec![("expenses:abos:netflix", 9.99)]),
        ],
    };
    let r = app.recurring_expenses();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].name, "expenses:abos:netflix");
    assert!((r[0].monthly_avg - 9.99).abs() < 0.01);
}

#[test]
fn recurring_expenses_ignores_too_variable() {
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month("January", vec![("expenses:misc", 10.0)]),
            make_month("February", vec![("expenses:misc", 100.0)]),
            make_month("March", vec![("expenses:misc", 500.0)]),
        ],
    };
    let r = app.recurring_expenses();
    assert!(r.is_empty(), "variable amounts should not be recurring");
}

#[test]
fn recurring_expenses_ignores_slightly_variable() {
    // 100 vs 150 = max/min 1.5, above 1.15 threshold → not recurring
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month("January", vec![("expenses:food", 100.0)]),
            make_month("February", vec![("expenses:food", 150.0)]),
            make_month("March", vec![("expenses:food", 120.0)]),
        ],
    };
    let r = app.recurring_expenses();
    assert!(
        r.is_empty(),
        "grocery-style variation should not be recurring"
    );
}

#[test]
fn recurring_expenses_skips_parent_when_child_present() {
    // parent "expenses:wohnen" and child "expenses:wohnen:miete" both in
    // expenses — only the leaf should be counted, not the parent
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month(
                "January",
                vec![("expenses:wohnen", 800.0), ("expenses:wohnen:miete", 800.0)],
            ),
            make_month(
                "February",
                vec![("expenses:wohnen", 800.0), ("expenses:wohnen:miete", 800.0)],
            ),
            make_month(
                "March",
                vec![("expenses:wohnen", 800.0), ("expenses:wohnen:miete", 800.0)],
            ),
        ],
    };
    let r = app.recurring_expenses();
    assert_eq!(r.len(), 1, "parent+child should deduplicate to one entry");
    assert_eq!(r[0].name, "expenses:wohnen:miete");
}

#[test]
fn recurring_expenses_requires_at_least_3_months() {
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month("January", vec![("expenses:abos:spotify", 9.99)]),
            make_month("February", vec![("expenses:abos:spotify", 9.99)]),
        ],
    };
    let r = app.recurring_expenses();
    assert!(r.is_empty(), "need ≥3 occurrences");
}

#[test]
fn recurring_expenses_combines_last_year_months() {
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![make_month("January", vec![("expenses:abos:gym", 30.0)])],
    };
    app.last_year = crate::data::MonthlyData {
        months: vec![
            make_month("November", vec![("expenses:abos:gym", 30.0)]),
            make_month("December", vec![("expenses:abos:gym", 30.0)]),
        ],
    };
    let r = app.recurring_expenses();
    assert_eq!(r.len(), 1);
}

#[test]
fn cash_flow_forecast_actuals_only_when_full_year() {
    let mut app = App::fixture_empty();
    // 12 months all loaded → no projected points
    let months: Vec<crate::data::SingleMonth> = (1..=12)
        .map(|i| {
            let name = crate::data::month_name(i).to_string();
            crate::data::SingleMonth {
                month_name: name,
                income: vec![("income:salary".to_string(), 2000.0)],
                expenses: vec![("expenses:rent".to_string(), 1000.0)],
                total_income: 2000.0,
                total_expenses: 1000.0,
            }
        })
        .collect();
    app.monthly = crate::data::MonthlyData { months };
    let f = app.cash_flow_forecast();
    assert_eq!(f.actuals.len(), 12);
    assert!(f.projected.is_empty());
}

#[test]
fn cash_flow_forecast_projects_remaining_months() {
    let mut app = App::fixture_empty();
    // 3 months loaded (Jan-Mar) → 9 projected + bridge point
    let months: Vec<crate::data::SingleMonth> = ["January", "February", "March"]
        .iter()
        .map(|name| crate::data::SingleMonth {
            month_name: name.to_string(),
            income: vec![("income:salary".to_string(), 3000.0)],
            expenses: vec![("expenses:rent".to_string(), 1500.0)],
            total_income: 3000.0,
            total_expenses: 1500.0,
        })
        .collect();
    app.monthly = crate::data::MonthlyData { months };
    let f = app.cash_flow_forecast();
    assert_eq!(f.actuals.len(), 3);
    // projected = bridge(Mar) + Apr..Dec = 1 + 9 = 10
    assert_eq!(f.projected.len(), 10);
    assert!((f.projected_monthly_net - 1500.0).abs() < 0.01);
    // First projected point is the last actual (bridge)
    assert!(
        (f.projected[0].0 - 2.0).abs() < 1e-9,
        "bridge at month idx 2"
    );
}

#[test]
fn cash_flow_forecast_empty_months() {
    let app = App::fixture_empty();
    let f = app.cash_flow_forecast();
    assert!(f.actuals.is_empty());
    assert!(f.projected.is_empty());
}

#[test]
fn cash_flow_forecast_uses_hledger_forecast_over_computed_avg() {
    let mut app = App::fixture_empty();
    // 2 actual months (Jan income 3000, expenses 1500 → net 1500)
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month_with_income("January", 3000.0, vec![("expenses:rent", 1500.0)]),
            make_month_with_income("February", 3000.0, vec![("expenses:rent", 1500.0)]),
        ],
    };
    // hledger forecast provides Mar–May with income 2500, rent 1000 (net 1500 differs)
    app.monthly_forecast = crate::data::MonthlyData {
        months: vec![
            // Jan & Feb included (actual months – should be ignored in hledger_forecast_map)
            make_month_with_income("January", 3000.0, vec![("expenses:rent", 1500.0)]),
            make_month_with_income("February", 3000.0, vec![("expenses:rent", 1500.0)]),
            // Forecast months
            make_month_with_income("March", 2500.0, vec![("expenses:rent", 1000.0)]),
            make_month_with_income("April", 2500.0, vec![("expenses:rent", 1000.0)]),
            make_month_with_income("May", 2500.0, vec![("expenses:rent", 1000.0)]),
        ],
    };
    let f = app.cash_flow_forecast();
    assert_eq!(f.actuals.len(), 2, "Jan+Feb are actuals");
    // projected: bridge(Feb) + Mar + Apr + May = 4
    assert_eq!(f.projected.len(), 4);
    // Projected values come from hledger forecast (2500-1000=1500), not computed avg
    for &(_, y) in &f.projected[1..] {
        assert!(
            (y - 1500.0).abs() < 0.01,
            "projected net should be 1500 from hledger, got {y}"
        );
    }
}

fn make_month_with_income(
    name: &str,
    income: f64,
    expenses: Vec<(&str, f64)>,
) -> crate::data::SingleMonth {
    let total_expenses = expenses.iter().map(|(_, a)| a).sum();
    crate::data::SingleMonth {
        month_name: name.to_string(),
        income: vec![("income:salary".to_string(), income)],
        expenses: expenses
            .into_iter()
            .map(|(n, a)| (n.to_string(), a))
            .collect(),
        total_income: income,
        total_expenses,
    }
}

#[test]
fn cash_flow_forecast_y_bounds_contain_all_points() {
    let app = App::fixture_with_monthly();
    let f = app.cash_flow_forecast();
    for &(_, y) in f.actuals.iter().chain(f.projected.iter()) {
        assert!(y >= f.min_y - 1e-9, "y={y} below min_y={}", f.min_y);
        assert!(y <= f.max_y + 1e-9, "y={y} above max_y={}", f.max_y);
    }
}

#[test]
fn recurring_expenses_sorted_by_amount_desc() {
    let mut app = App::fixture_empty();
    app.monthly = crate::data::MonthlyData {
        months: vec![
            make_month(
                "January",
                vec![("expenses:abos:cheap", 5.0), ("expenses:abos:pricey", 50.0)],
            ),
            make_month(
                "February",
                vec![("expenses:abos:cheap", 5.0), ("expenses:abos:pricey", 50.0)],
            ),
            make_month(
                "March",
                vec![("expenses:abos:cheap", 5.0), ("expenses:abos:pricey", 50.0)],
            ),
        ],
    };
    let r = app.recurring_expenses();
    assert_eq!(r.len(), 2);
    assert!(r[0].monthly_avg > r[1].monthly_avg, "should be sorted desc");
}

#[test]
fn price_alerts_match_with_normalized_commodities() {
    let mut app = App::fixture_empty();
    app.has_crypto = true;
    app.alert_dismissed = false;
    app.holdings = vec![
        crate::data::CryptoHolding {
            commodity: "ETH".to_string(),
            amount: 1.0,
            price_eur: 0.0,
            value_eur: 0.0,
        },
        crate::data::CryptoHolding {
            commodity: "SOL".to_string(),
            amount: 1.0,
            price_eur: 0.0,
            value_eur: 0.0,
        },
        crate::data::CryptoHolding {
            commodity: "LINK".to_string(),
            amount: 1.0,
            price_eur: 0.0,
            value_eur: 0.0,
        },
    ];

    let today = Local::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);
    app.price_history = vec![
        crate::data::PriceEntry {
            date: yesterday,
            commodity: "\"eth\"".to_string(),
            price_eur: 100.0,
        },
        crate::data::PriceEntry {
            date: today,
            commodity: "eth".to_string(),
            price_eur: 103.0,
        },
        crate::data::PriceEntry {
            date: yesterday,
            commodity: "sol".to_string(),
            price_eur: 50.0,
        },
        crate::data::PriceEntry {
            date: today,
            commodity: "\"SOL\"".to_string(),
            price_eur: 48.0,
        },
        crate::data::PriceEntry {
            date: yesterday,
            commodity: "link".to_string(),
            price_eur: 20.0,
        },
        crate::data::PriceEntry {
            date: today,
            commodity: "LINK".to_string(),
            price_eur: 20.6,
        },
    ];

    app.compute_price_alerts();
    assert_eq!(app.price_alerts.len(), 3);
    let coins: std::collections::HashSet<&str> =
        app.price_alerts.iter().map(|a| a.coin.as_str()).collect();
    assert!(coins.contains("ETH"));
    assert!(coins.contains("SOL"));
    assert!(coins.contains("LINK"));
}

#[test]
fn price_alerts_clear_when_no_longer_over_threshold() {
    let mut app = App::fixture_empty();
    app.has_crypto = true;
    app.alert_dismissed = false;
    app.holdings = vec![crate::data::CryptoHolding {
        commodity: "BTC".to_string(),
        amount: 1.0,
        price_eur: 0.0,
        value_eur: 0.0,
    }];

    let today = Local::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);
    app.price_history = vec![
        crate::data::PriceEntry {
            date: yesterday,
            commodity: "BTC".to_string(),
            price_eur: 100.0,
        },
        crate::data::PriceEntry {
            date: today,
            commodity: "BTC".to_string(),
            price_eur: 105.0,
        },
    ];
    app.compute_price_alerts();
    assert_eq!(app.price_alerts.len(), 1);

    app.price_history = vec![
        crate::data::PriceEntry {
            date: yesterday,
            commodity: "BTC".to_string(),
            price_eur: 100.0,
        },
        crate::data::PriceEntry {
            date: today,
            commodity: "BTC".to_string(),
            price_eur: 101.0,
        },
    ];
    app.compute_price_alerts();
    assert!(app.price_alerts.is_empty());
    assert!(!app.show_alerts);
}

#[test]
fn price_alert_threshold_comes_from_config() {
    let mut app = App::fixture_empty();
    app.has_crypto = true;
    app.alert_dismissed = false;
    app.holdings = vec![crate::data::CryptoHolding {
        commodity: "ETH".to_string(),
        amount: 1.0,
        price_eur: 0.0,
        value_eur: 0.0,
    }];

    let today = Local::now().date_naive();
    let yesterday = today - chrono::Duration::days(1);
    app.price_history = vec![
        crate::data::PriceEntry {
            date: yesterday,
            commodity: "ETH".to_string(),
            price_eur: 100.0,
        },
        crate::data::PriceEntry {
            date: today,
            commodity: "ETH".to_string(),
            price_eur: 101.0, // +1.0%
        },
    ];

    app.config.price_alert_threshold_pct = 2.0;
    app.compute_price_alerts();
    assert!(app.price_alerts.is_empty(), "1% should be filtered at 2%");

    app.config.price_alert_threshold_pct = 0.5;
    app.compute_price_alerts();
    assert_eq!(app.price_alerts.len(), 1, "1% should pass at 0.5%");
}
