use anyhow::Result;

use super::{MonthlyData, SingleMonth};

/// Parse a European-formatted number like "1.524,00" or "74,52" or "0,02352448"
pub fn parse_eu_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let negative = s.starts_with('-');
    let s = if negative { &s[1..] } else { s };

    let cleaned = if s.contains('.') && s.contains(',') {
        // e.g. "1.524,00" → dot is thousands sep, comma is decimal
        s.replace('.', "").replace(',', ".")
    } else if s.contains(',') {
        // e.g. "74,52"
        s.replace(',', ".")
    } else {
        s.to_string()
    };

    let val: f64 = cleaned.parse().ok()?;
    Some(if negative { -val } else { val })
}

/// Parse an amount string like "4,40140000 SOL" or "1430,15 €" into (amount, commodity)
pub(super) fn parse_amount_str(s: &str) -> Option<(f64, String)> {
    let s = s.trim().trim_matches('"');
    if s == "0" || s.is_empty() {
        return None;
    }

    let pos = s.rfind(' ')?;
    let num_str = s[..pos].trim();
    let commodity = s[pos + 1..].trim().to_string();

    let amount = parse_eu_number(num_str)?;
    Some((amount, commodity))
}

pub(super) fn parse_balance_csv(text: &str) -> Result<Vec<super::AccountBalance>> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut rdr = csv::ReaderBuilder::new().from_reader(text.as_bytes());

    // Validate header up front instead of relying on a "row count + first line
    // sniff" heuristic at the end. Expected hledger `balance -O csv` header:
    // first column is `account`, the rest are balance columns.
    let headers = rdr
        .headers()
        .map_err(|e| anyhow::anyhow!("hledger CSV: cannot read header: {e}"))?
        .clone();
    if headers.is_empty() || !headers[0].eq_ignore_ascii_case("account") {
        let first_line = text.lines().next().unwrap_or("");
        return Err(anyhow::anyhow!(
            "Unexpected hledger CSV format: {:?}",
            first_line
        ));
    }

    let mut result = Vec::new();
    for row in rdr.records() {
        let row = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if row.len() < 2 {
            continue;
        }
        let account = row[0].to_string();
        let balance = row[1].trim();
        if balance == "0" || balance.is_empty() {
            continue;
        }
        if let Some((amount, commodity)) = parse_amount_str(balance) {
            if amount.abs() > 1e-10 {
                result.push(super::AccountBalance {
                    account,
                    amount,
                    commodity,
                });
            }
        }
    }

    Ok(result)
}

pub(super) fn parse_monthly_csv(text: &str, currency_symbol: &str) -> Result<MonthlyData> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    let num_cols = rdr.headers().map(|h| h.len()).unwrap_or(2);

    let mut months_data: Vec<SingleMonth> = (1..num_cols)
        .map(|m| SingleMonth {
            month_name: month_name(m).to_string(),
            ..Default::default()
        })
        .collect();

    let mut in_revenues = false;
    let mut in_expenses = false;

    for row in rdr.records() {
        let fields = match row {
            Ok(r) => r,
            Err(_) => continue,
        };
        if fields.is_empty() {
            continue;
        }

        let account = fields[0].to_string();

        match account.as_str() {
            a if a.starts_with("Monthly") => continue,
            "Revenues" | "Revenue" => {
                in_revenues = true;
                in_expenses = false;
                continue;
            }
            "Expenses" | "Expense" => {
                in_revenues = false;
                in_expenses = true;
                continue;
            }
            "Total" => continue,
            _ => {}
        }

        for col_idx in 1..num_cols {
            if col_idx >= fields.len() {
                continue;
            }
            let val_str = &fields[col_idx];
            if let Some((amount, commodity)) = parse_amount_str(val_str) {
                if commodity == currency_symbol && amount.abs() > 0.005 {
                    let month = &mut months_data[col_idx - 1];
                    if in_revenues {
                        month.income.push((account.clone(), amount.abs()));
                        month.total_income += amount.abs();
                    } else if in_expenses {
                        month.expenses.push((account.clone(), amount.abs()));
                        month.total_expenses += amount.abs();
                    }
                }
            }
        }
    }

    let months: Vec<SingleMonth> = months_data
        .into_iter()
        .filter(|m| m.total_income > 0.0 || m.total_expenses > 0.0)
        .map(|mut m| {
            m.income
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            m.expenses
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            m
        })
        .collect();

    Ok(MonthlyData { months })
}

/// Calendar month names indexed 0..=11 (January = 0).
///
/// Single source of truth — used by `month_name`, `month_index`, the cash flow
/// forecast, the combined-months rebuild and the period formatter.
pub(crate) const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

pub(crate) fn month_name(m: usize) -> &'static str {
    if (1..=12).contains(&m) {
        MONTH_NAMES[m - 1]
    } else {
        "Unknown"
    }
}

/// Returns the 1-based month number (1..=12) for `name`, matching exact case
/// (e.g. "January" → Some(1)).
pub(crate) fn month_index(name: &str) -> Option<usize> {
    MONTH_NAMES.iter().position(|&n| n == name).map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_eu_number ───────────────────────────────────────────────────────

    #[test]
    fn eu_number_plain_comma() {
        assert!((parse_eu_number("74,52").unwrap() - 74.52).abs() < 1e-10);
    }

    #[test]
    fn eu_number_thousands_dot() {
        assert!((parse_eu_number("1.524,00").unwrap() - 1524.0).abs() < 1e-10);
    }

    #[test]
    fn eu_number_small_decimal() {
        assert!((parse_eu_number("0,02352448").unwrap() - 0.02352448).abs() < 1e-10);
    }

    #[test]
    fn eu_number_negative_comma() {
        assert!((parse_eu_number("-74,52").unwrap() - (-74.52)).abs() < 1e-10);
    }

    #[test]
    fn eu_number_negative_thousands() {
        assert!((parse_eu_number("-1.524,00").unwrap() - (-1524.0)).abs() < 1e-10);
    }

    #[test]
    fn eu_number_zero() {
        assert_eq!(parse_eu_number("0"), Some(0.0));
    }

    #[test]
    fn eu_number_plain_dot_decimal() {
        assert!((parse_eu_number("1234.56").unwrap() - 1234.56).abs() < 1e-10);
    }

    #[test]
    fn eu_number_whitespace() {
        assert!((parse_eu_number("  74,52  ").unwrap() - 74.52).abs() < 1e-10);
    }

    #[test]
    fn eu_number_empty() {
        assert_eq!(parse_eu_number(""), None);
    }

    #[test]
    fn eu_number_non_numeric() {
        assert_eq!(parse_eu_number("abc"), None);
    }

    // ── parse_amount_str ──────────────────────────────────────────────────────

    #[test]
    fn amount_str_crypto() {
        let (amt, com) = parse_amount_str("4,40140000 SOL").unwrap();
        assert!((amt - 4.4014).abs() < 1e-6);
        assert_eq!(com, "SOL");
    }

    #[test]
    fn amount_str_euros_thousands() {
        let (amt, com) = parse_amount_str("1.430,15 €").unwrap();
        assert!((amt - 1430.15).abs() < 1e-10);
        assert_eq!(com, "€");
    }

    #[test]
    fn amount_str_negative_eur() {
        let (amt, com) = parse_amount_str("-200,00 EUR").unwrap();
        assert!((amt - (-200.0)).abs() < 1e-10);
        assert_eq!(com, "EUR");
    }

    #[test]
    fn amount_str_zero_returns_none() {
        assert_eq!(parse_amount_str("0"), None);
    }

    #[test]
    fn amount_str_empty_returns_none() {
        assert_eq!(parse_amount_str(""), None);
    }

    #[test]
    fn amount_str_strips_outer_quotes() {
        let (amt, com) = parse_amount_str("\"1.000,00 €\"").unwrap();
        assert!((amt - 1000.0).abs() < 1e-10);
        assert_eq!(com, "€");
    }

    // ── parse_balance_csv ─────────────────────────────────────────────────────

    #[test]
    fn balance_csv_garbage_input_returns_err() {
        let garbage = "this is not csv at all and it is definitely longer than twenty chars";
        let result = parse_balance_csv(garbage);
        assert!(result.is_err(), "expected Err for garbage CSV input");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Unexpected hledger CSV format"), "got: {msg}");
    }

    #[test]
    fn balance_csv_parses_two_accounts() {
        let csv = "account,balance\n\
                   assets:bank:checking,\"1.430,15 €\"\n\
                   assets:crypto:btc,\"0,05 BTC\"\n";
        let result = parse_balance_csv(csv).unwrap();
        assert_eq!(result.len(), 2);

        let checking = result
            .iter()
            .find(|b| b.account == "assets:bank:checking")
            .unwrap();
        assert!((checking.amount - 1430.15).abs() < 1e-10);
        assert_eq!(checking.commodity, "€");

        let btc = result
            .iter()
            .find(|b| b.account == "assets:crypto:btc")
            .unwrap();
        assert!((btc.amount - 0.05).abs() < 1e-10);
        assert_eq!(btc.commodity, "BTC");
    }

    #[test]
    fn balance_csv_skips_zero_balance() {
        let csv = "account,balance\nassets:empty,0\n";
        let result = parse_balance_csv(csv).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn balance_csv_empty_input() {
        let result = parse_balance_csv("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn balance_csv_skips_malformed_rows() {
        let csv = "account,balance\nbad_row_only_one_field\nassets:good,\"100,00 €\"\n";
        let result = parse_balance_csv(csv).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].account, "assets:good");
    }

    // ── parse_monthly_csv ─────────────────────────────────────────────────────

    fn two_month_csv(sym: &str) -> String {
        format!(
            "Monthly Income Statement,Jan 2024,Feb 2024\n\
             Revenues,,\n\
             income:salary,\"2.000,00 {sym}\",\"2.000,00 {sym}\"\n\
             Expenses,,\n\
             expenses:food,\"150,00 {sym}\",0\n\
             expenses:housing,0,\"800,00 {sym}\"\n"
        )
    }

    #[test]
    fn monthly_csv_two_months_income_and_expenses() {
        let csv = two_month_csv("€");
        let data = parse_monthly_csv(&csv, "€").unwrap();

        assert_eq!(data.months.len(), 2);

        let jan = &data.months[0];
        assert_eq!(jan.month_name, "January");
        assert!((jan.total_income - 2000.0).abs() < 0.01);
        assert!((jan.total_expenses - 150.0).abs() < 0.01);
        assert_eq!(jan.income.len(), 1);
        assert_eq!(jan.expenses.len(), 1);

        let feb = &data.months[1];
        assert_eq!(feb.month_name, "February");
        assert!((feb.total_income - 2000.0).abs() < 0.01);
        assert!((feb.total_expenses - 800.0).abs() < 0.01);
    }

    #[test]
    fn monthly_csv_ignores_wrong_currency() {
        let csv = two_month_csv("$");
        let data = parse_monthly_csv(&csv, "€").unwrap();
        assert!(data.months.is_empty());
    }

    #[test]
    fn monthly_csv_expenses_sorted_descending() {
        let csv = "Monthly Income Statement,Jan 2024\n\
                   Revenues,\n\
                   income:salary,\"3.000,00 €\"\n\
                   Expenses,\n\
                   expenses:food,\"100,00 €\"\n\
                   expenses:housing,\"1.200,00 €\"\n";
        let data = parse_monthly_csv(csv, "€").unwrap();
        let jan = &data.months[0];
        assert!(
            jan.expenses[0].1 >= jan.expenses[1].1,
            "expenses not sorted descending"
        );
    }
}
