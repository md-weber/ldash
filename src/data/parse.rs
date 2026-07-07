use anyhow::Result;

use super::{currency_matches, MonthlyData, SingleMonth};

/// Parse a number that may be formatted in either EU or US convention.
///
/// When both a dot and a comma are present, the one that appears **last**
/// is treated as the decimal separator:
/// - `"1.234,56"` → last sep is `,` → EU format → 1234.56
/// - `"1,234.56"` → last sep is `.` → US format → 1234.56
///
/// When only a comma is present it is treated as the decimal separator
/// (EU convention): `"74,52"` → 74.52.
/// When only a dot is present (or neither), standard float parsing applies.
pub fn parse_eu_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let negative = s.starts_with('-');
    let s = if negative { &s[1..] } else { s };

    let cleaned = if s.contains('.') && s.contains(',') {
        let last_dot = s.rfind('.').unwrap();
        let last_comma = s.rfind(',').unwrap();
        if last_dot > last_comma {
            // US format: "1,234.56" — comma is thousands sep, dot is decimal
            s.replace(',', "")
        } else {
            // EU format: "1.234,56" — dot is thousands sep, comma is decimal
            s.replace('.', "").replace(',', ".")
        }
    } else if s.contains(',') {
        // e.g. "74,52"
        s.replace(',', ".")
    } else {
        s.to_string()
    };

    let val: f64 = cleaned.parse().ok()?;
    Some(if negative { -val } else { val })
}

/// Parse an amount string like "4,40140000 SOL" or "1430,15 €" or "$1,234.56"
/// into (amount, commodity).
///
/// Three formats are tried in order:
/// 1. Suffix with space: `"100 EUR"`, `"3 800,00 €"` — rfind keeps
///    space-as-thousands-separator amounts intact.
/// 2. Prefix with space: `"Eur 100"`, `"-Eur 100"`.
/// 3. Attached prefix (no space): `"$100"`, `"$1,234.56"`, `"-$100.00"` —
///    common US convention where the currency symbol is glued to the number.
///
/// Only handles a **single** commodity amount. For multi-commodity strings use
/// `parse_first_amount_str`.
pub(super) fn parse_amount_str(s: &str) -> Option<(f64, String)> {
    let s = s.trim().trim_matches('"');
    if s == "0" || s.is_empty() {
        return None;
    }

    // Suffix format: "<number> <commodity>"  e.g. "100 EUR", "3 800,00 €"
    // rfind keeps amounts with a space-thousands separator intact.
    if let Some(pos) = s.rfind(' ') {
        let num_str = s[..pos].trim();
        let commodity = s[pos + 1..].trim().to_string();
        if let Some(amount) = parse_eu_number(num_str) {
            return Some((amount, commodity));
        }
    }

    // Prefix format: "<commodity> <amount>"  e.g. "Eur 100", "Eur -100"
    // Also handles a sign attached to the symbol: "-Eur 100".
    if let Some(pos) = s.find(' ') {
        let raw_sym = s[..pos].trim();
        let num_str = s[pos + 1..].trim();

        let (commodity, num_str): (String, String) =
            if let Some(stripped) = raw_sym.strip_prefix('-') {
                // "-Eur 100" → commodity "Eur", amount -100
                let negated = format!("-{}", num_str.trim_start_matches('-'));
                (stripped.to_string(), negated)
            } else {
                (raw_sym.to_string(), num_str.to_string())
            };

        if let Some(amount) = parse_eu_number(&num_str) {
            return Some((amount, commodity));
        }
    }

    // Attached-prefix format: symbol directly against number, no space.
    // Handles "$100", "$1,234.56", "-$100.00", "$-100".
    // The outer sign is stripped first, then leading non-digit chars form the
    // commodity symbol, and the remainder is parsed as a number.
    {
        let (outer_neg, after_sign) = if s.starts_with('-') {
            (true, &s[1..])
        } else {
            (false, s)
        };

        // Find where the symbol ends: first char that could start a number.
        let sym_end = after_sign
            .char_indices()
            .find(|(_, c)| c.is_ascii_digit() || *c == '-' || *c == ',' || *c == '.')
            .map(|(i, _)| i)
            .unwrap_or(0);

        if sym_end > 0 && sym_end < after_sign.len() {
            let sym = &after_sign[..sym_end];
            // Reject if the "symbol" contains a space — that would be a
            // spaced prefix already handled above, or garbled input.
            if !sym.contains(' ') {
                let num_part = &after_sign[sym_end..];
                if let Some(amount) = parse_eu_number(num_part) {
                    let final_amount = if outer_neg { -amount } else { amount };
                    return Some((final_amount, sym.to_string()));
                }
            }
        }
    }

    None
}

/// Parse the **first** valid amount from a balance string that may contain
/// multiple comma-separated commodities, e.g. `"652.00 A, 601.00 C, -3.00 D"`.
///
/// Falls back to `parse_amount_str` for single-commodity strings so that the
/// common path has no overhead.
pub(super) fn parse_first_amount_str(s: &str) -> Option<(f64, String)> {
    // Fast path: if there's no ", " separator the string holds a single
    // commodity and the standard parser handles it correctly.
    if !s.contains(", ") {
        return parse_amount_str(s);
    }
    // Multi-commodity: try each part in order and return the first that parses.
    for part in s.split(", ") {
        if let Some(result) = parse_amount_str(part.trim()) {
            return Some(result);
        }
    }
    None
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
        if let Some((amount, commodity)) = parse_first_amount_str(balance) {
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

/// Map a 3-letter month abbreviation (case-insensitive) to a canonical full name.
fn month_name_from_abbrev(h: &str) -> &'static str {
    match h
        .trim()
        .get(..3)
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jan" => "January",
        "feb" => "February",
        "mar" => "March",
        "apr" => "April",
        "may" => "May",
        "jun" => "June",
        "jul" => "July",
        "aug" => "August",
        "sep" => "September",
        "oct" => "October",
        "nov" => "November",
        "dec" => "December",
        _ => "Unknown",
    }
}

pub(super) fn parse_monthly_csv(text: &str, currency_symbol: &str) -> Result<MonthlyData> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());

    // hledger incomestatement CSV can have two header row formats:
    //
    //   Single-row (tests / simple cases):
    //     "Monthly Income Statement,Jan 2024,Feb 2024"
    //     month abbreviations are in the header row itself.
    //
    //   Two-row (real hledger output):
    //     row 1 — "Monthly Income Statement 2024","","", ...  (month cols are empty)
    //     row 2 — "Account","Jan","Feb", ...                  (actual month headers)
    //
    // Detect the two-row format by checking whether the month-position columns
    // in the CSV header are all empty, and if so consume the first data record
    // to obtain the real month names.
    let headers = rdr.headers().cloned().unwrap_or_default();
    let num_cols = headers.len();

    let month_names_from_header: Vec<&'static str> = headers
        .iter()
        .skip(1)
        .map(|h| month_name_from_abbrev(h))
        .collect();

    let all_unknown = month_names_from_header.iter().all(|&n| n == "Unknown");

    let mut records = rdr.records().peekable();

    let month_names: Vec<&'static str> = if all_unknown {
        // Two-row format: the first data record holds the real column headers.
        match records.next() {
            Some(Ok(r)) => r
                .iter()
                .skip(1)
                .map(|h| month_name_from_abbrev(h))
                .collect(),
            _ => return Ok(MonthlyData::default()),
        }
    } else {
        month_names_from_header
    };

    let mut months_data: Vec<SingleMonth> = month_names
        .iter()
        .map(|&name| SingleMonth {
            month_name: name.to_string(),
            ..Default::default()
        })
        .collect();

    let mut in_revenues = false;
    let mut in_expenses = false;

    for row in records {
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
                if currency_matches(currency_symbol, &commodity) && amount.abs() > 0.005 {
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

#[allow(dead_code)]
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

    // US-format regression tests (Bug 1: comma-as-thousands, dot-as-decimal).
    #[test]
    fn eu_number_us_thousands_comma() {
        assert!((parse_eu_number("1,234.56").unwrap() - 1234.56).abs() < 1e-10);
    }

    #[test]
    fn eu_number_us_large_amount() {
        assert!((parse_eu_number("10,000.00").unwrap() - 10_000.0).abs() < 1e-10);
    }

    #[test]
    fn eu_number_us_negative() {
        assert!((parse_eu_number("-1,234.56").unwrap() - (-1234.56)).abs() < 1e-10);
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

    #[test]
    fn amount_str_prefix_commodity() {
        let (amt, com) = parse_amount_str("Eur 100").unwrap();
        assert!((amt - 100.0).abs() < 1e-10);
        assert_eq!(com, "Eur");
    }

    #[test]
    fn amount_str_prefix_negative_amount() {
        let (amt, com) = parse_amount_str("Eur -100").unwrap();
        assert!((amt - (-100.0)).abs() < 1e-10);
        assert_eq!(com, "Eur");
    }

    #[test]
    fn amount_str_prefix_negative_symbol() {
        let (amt, com) = parse_amount_str("-Eur 100").unwrap();
        assert!((amt - (-100.0)).abs() < 1e-10);
        assert_eq!(com, "Eur");
    }

    #[test]
    fn amount_str_suffix_dot_thousands() {
        // Regression guard: dot-as-thousands-separator (hledger CSV output).
        let (amt, com) = parse_amount_str("1.430,15 €").unwrap();
        assert!((amt - 1430.15).abs() < 1e-10);
        assert_eq!(com, "€");
    }

    // US-format attached-prefix regressions (Bug 2: "$" without space).
    #[test]
    fn amount_str_dollar_prefix_no_space() {
        let (amt, com) = parse_amount_str("$100").unwrap();
        assert!((amt - 100.0).abs() < 1e-10);
        assert_eq!(com, "$");
    }

    #[test]
    fn amount_str_dollar_prefix_us_thousands() {
        let (amt, com) = parse_amount_str("$1,234.56").unwrap();
        assert!((amt - 1234.56).abs() < 1e-10);
        assert_eq!(com, "$");
    }

    #[test]
    fn amount_str_negative_dollar_prefix() {
        let (amt, com) = parse_amount_str("-$100.00").unwrap();
        assert!((amt - (-100.0)).abs() < 1e-10);
        assert_eq!(com, "$");
    }

    #[test]
    fn amount_str_dollar_prefix_large_us() {
        let (amt, com) = parse_amount_str("$2,000.00").unwrap();
        assert!((amt - 2000.0).abs() < 1e-10);
        assert_eq!(com, "$");
    }

    // ── parse_first_amount_str ────────────────────────────────────────────────

    #[test]
    fn first_amount_single_commodity() {
        let (amt, com) = parse_first_amount_str("652.00 A").unwrap();
        assert!((amt - 652.0).abs() < 1e-10);
        assert_eq!(com, "A");
    }

    #[test]
    fn first_amount_multi_commodity_takes_first() {
        let (amt, com) = parse_first_amount_str("652.00 A, 601.00 C, 551.00 E").unwrap();
        assert!((amt - 652.0).abs() < 1e-10);
        assert_eq!(com, "A");
    }

    #[test]
    fn first_amount_multi_commodity_negative() {
        let (amt, com) = parse_first_amount_str("-651.00 A, -0.71 B").unwrap();
        assert!((amt - (-651.0)).abs() < 1e-10);
        assert_eq!(com, "A");
    }

    #[test]
    fn first_amount_zero_string_returns_none() {
        assert_eq!(parse_first_amount_str("0"), None);
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
    fn balance_csv_multi_commodity_takes_first() {
        let csv = "account,balance\n\"1\",\"652.00 A, 601.00 C, 551.00 E\"\n";
        let result = parse_balance_csv(csv).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].account, "1");
        assert!((result[0].amount - 652.0).abs() < 1e-10);
        assert_eq!(result[0].commodity, "A");
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
    fn monthly_csv_accepts_currency_aliases() {
        let csv = two_month_csv("EUR");
        let data = parse_monthly_csv(&csv, "€").unwrap();
        assert_eq!(data.months.len(), 2);
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

    // US-format regression: "$2,000.00" attached-prefix amounts (Bugs 1 & 2).
    #[test]
    fn monthly_csv_us_format_dollar_prefix() {
        let csv = "Monthly Income Statement,Jan 2026,Feb 2026\n\
                   Revenues,,\n\
                   income:salary,\"$2,000.00\",\"$2,000.00\"\n\
                   Expenses,,\n\
                   expenses:food,\"$150.00\",0\n\
                   expenses:housing,0,\"$800.00\"\n";
        let data = parse_monthly_csv(csv, "$").unwrap();
        assert_eq!(data.months.len(), 2);

        let jan = &data.months[0];
        assert_eq!(jan.month_name, "January");
        assert!(
            (jan.total_income - 2000.0).abs() < 0.01,
            "jan income: {}",
            jan.total_income
        );
        assert!(
            (jan.total_expenses - 150.0).abs() < 0.01,
            "jan expenses: {}",
            jan.total_expenses
        );

        let feb = &data.months[1];
        assert_eq!(feb.month_name, "February");
        assert!(
            (feb.total_income - 2000.0).abs() < 0.01,
            "feb income: {}",
            feb.total_income
        );
        assert!(
            (feb.total_expenses - 800.0).abs() < 0.01,
            "feb expenses: {}",
            feb.total_expenses
        );
    }
}
