use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn hledger_available() -> bool {
    std::process::Command::new("hledger")
        .arg("--version")
        .output()
        .is_ok()
}

// ── parse_eu_number (public API, no hledger needed) ───────────────────────────

#[test]
fn integration_parse_eu_number_roundtrip() {
    use ldash::data::parse_eu_number;
    assert!((parse_eu_number("1.234,56").unwrap() - 1234.56).abs() < 1e-10);
    assert!((parse_eu_number("-99,99").unwrap() - (-99.99)).abs() < 1e-10);
    assert_eq!(parse_eu_number("garbage"), None);
}

// ── hledger-dependent tests ───────────────────────────────────────────────────
// These are marked #[ignore] so `cargo test` skips them by default.
// Run with: cargo test -- --ignored
// Or gate with the hledger_available() guard inside the test body.

#[test]
#[ignore]
fn integration_load_monthly_data_two_months() {
    if !hledger_available() {
        return;
    }
    let path = fixture("simple.journal");
    let data = ldash::data::load_monthly_data(&path, "EUR").unwrap();

    assert_eq!(data.months.len(), 2, "expected Jan + Feb");

    let jan = data
        .months
        .iter()
        .find(|m| m.month_name == "January")
        .unwrap();
    assert!(
        (jan.total_income - 2000.0).abs() < 0.01,
        "jan income {}",
        jan.total_income
    );
    assert!(
        (jan.total_expenses - 150.0).abs() < 0.01,
        "jan expenses {}",
        jan.total_expenses
    );

    let feb = data
        .months
        .iter()
        .find(|m| m.month_name == "February")
        .unwrap();
    assert!(
        (feb.total_income - 2000.0).abs() < 0.01,
        "feb income {}",
        feb.total_income
    );
    assert!(
        (feb.total_expenses - 800.0).abs() < 0.01,
        "feb expenses {}",
        feb.total_expenses
    );
}

#[test]
#[ignore]
fn integration_load_monthly_with_forecast_periodic_rules() {
    if !hledger_available() {
        return;
    }
    let path = fixture("forecast.journal");
    let data = ldash::data::load_monthly_with_forecast(&path, "EUR").unwrap();

    // The fixture has actual Jan–Mar and periodic rules for salary (3000) and
    // rent (1000). With --forecast starting from next month after today, future
    // months (May–Dec when run in April 2026) should have the periodic income.
    // We just assert that at least one future month is present and carries the
    // expected periodic salary amount.
    let future_months: Vec<_> = data
        .months
        .iter()
        .filter(|m| !["January", "February", "March"].contains(&m.month_name.as_str()))
        .collect();

    assert!(
        !future_months.is_empty(),
        "expected at least one forecasted month, got: {:?}",
        data.months
            .iter()
            .map(|m| &m.month_name)
            .collect::<Vec<_>>()
    );

    for m in future_months {
        assert!(
            (m.total_income - 3000.0).abs() < 0.01,
            "forecast month {} income should be 3000 (periodic), got {}",
            m.month_name,
            m.total_income
        );
        assert!(
            (m.total_expenses - 1000.0).abs() < 0.01,
            "forecast month {} expenses should be 1000 (periodic), got {}",
            m.month_name,
            m.total_expenses
        );
    }
}

#[test]
#[ignore]
fn integration_load_account_balances_checking() {
    if !hledger_available() {
        return;
    }
    let path = fixture("simple.journal");
    let balances = ldash::data::load_account_balances_eur(&path, "assets").unwrap();

    let checking = balances
        .iter()
        .find(|b| b.account.contains("checking"))
        .unwrap();

    let expected = 2000.0 - 150.0 + 2000.0 - 800.0;
    assert!(
        (checking.amount - expected).abs() < 0.01,
        "checking balance {}, expected {expected}",
        checking.amount,
    );
}

#[test]
#[ignore]
fn integration_load_liquid_cash_monthly_reports_per_month_change() {
    if !hledger_available() {
        return;
    }
    let path = fixture("simple.journal");
    let prefixes = vec!["assets:bank:checking".to_string()];
    let monthly = ldash::data::load_liquid_cash_monthly(&path, &prefixes, "EUR").unwrap();

    // Fixture dates are Jan/Feb 2026; this test (like the other date-coupled
    // integration tests in this file) assumes it runs within calendar 2026.
    let jan = monthly
        .iter()
        .find(|(m, _)| m == "January")
        .expect("January column present");
    assert!(
        (jan.1 - 1850.0).abs() < 0.01,
        "jan liquid change {}, expected 1850 (2000 salary - 150 groceries)",
        jan.1
    );

    let feb = monthly
        .iter()
        .find(|(m, _)| m == "February")
        .expect("February column present");
    assert!(
        (feb.1 - 1200.0).abs() < 0.01,
        "feb liquid change {}, expected 1200 (2000 salary - 800 rent)",
        feb.1
    );
}

#[test]
#[ignore]
fn integration_load_monthly_data_us_format() {
    if !hledger_available() {
        return;
    }
    let path = fixture("us_simple.journal");
    let data = ldash::data::load_monthly_data(&path, "$").unwrap();

    assert_eq!(
        data.months.len(),
        2,
        "expected Jan + Feb, got: {:?}",
        data.months
            .iter()
            .map(|m| &m.month_name)
            .collect::<Vec<_>>()
    );

    let jan = data
        .months
        .iter()
        .find(|m| m.month_name == "January")
        .unwrap();
    assert!(
        (jan.total_income - 2000.0).abs() < 0.01,
        "jan income {}, expected 2000",
        jan.total_income
    );
    assert!(
        (jan.total_expenses - 150.0).abs() < 0.01,
        "jan expenses {}, expected 150",
        jan.total_expenses
    );

    let feb = data
        .months
        .iter()
        .find(|m| m.month_name == "February")
        .unwrap();
    assert!(
        (feb.total_income - 2000.0).abs() < 0.01,
        "feb income {}, expected 2000",
        feb.total_income
    );
    assert!(
        (feb.total_expenses - 800.0).abs() < 0.01,
        "feb expenses {}, expected 800",
        feb.total_expenses
    );
}

#[test]
fn integration_load_liquid_cash_monthly_empty_whitelist_returns_empty() {
    // No hledger call is made when the whitelist is empty, so this doesn't
    // need the availability guard.
    let path = fixture("simple.journal");
    let monthly = ldash::data::load_liquid_cash_monthly(&path, &[], "EUR").unwrap();
    assert!(monthly.is_empty());
}

#[test]
#[ignore]
fn integration_currency_alias_eur_symbol_loaders() {
    if !hledger_available() {
        return;
    }
    let path = fixture("simple.journal");

    let monthly = ldash::data::load_monthly_data(&path, "€").unwrap();
    assert!(
        !monthly.months.is_empty(),
        "expected monthly rows with EUR alias"
    );

    let nw = ldash::data::load_net_worth_history(&path, "monthly", "€", "assets", "liabilities")
        .unwrap();
    assert!(
        !nw.points.is_empty(),
        "expected net worth points with EUR alias"
    );

    let breakdown = ldash::data::load_net_worth_breakdown(&path, "monthly", "€", "assets").unwrap();
    assert!(
        !breakdown.layer_total.is_empty(),
        "expected breakdown rows with EUR alias"
    );
}
