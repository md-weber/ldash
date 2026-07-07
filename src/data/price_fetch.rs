use chrono::Local;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::config::PriceFetchToken;

/// Returns `true` when the prices journal already contains at least one `P`
/// entry dated today.  Used to skip the startup fetch when prices are fresh.
pub fn today_prices_present(prices_path: &Path) -> bool {
    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
    let content = std::fs::read_to_string(prices_path).unwrap_or_default();
    content.lines().any(|line| {
        let mut parts = line.split_whitespace();
        if parts.next() != Some("P") {
            return false;
        }
        parts.next() == Some(today.as_str())
    })
}

/// Fetch spot prices from the CoinGecko free API and append `P` directives for
/// today to `prices_path`.  Returns a human-readable status string on success
/// or an error message string on failure.
///
/// # Format written
///
/// ```text
/// P 2026-07-07 BTC 90.123,45 €
/// ```
///
/// Numbers use EU notation (`.` thousands separator, `,` decimal mark) to
/// match what the existing prices.journal produced by the Go script uses and
/// what `parse_eu_number` in ldash expects.
pub fn fetch_and_append_prices(
    prices_path: &Path,
    tokens: &[PriceFetchToken],
    currency: &str,
    currency_symbol: &str,
) -> Result<String, String> {
    if tokens.is_empty() {
        return Err("No tokens configured for price fetch".to_string());
    }

    let ids: Vec<&str> = tokens.iter().map(|t| t.id.as_str()).collect();
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
        ids.join(","),
        currency,
    );

    let response: HashMap<String, HashMap<String, f64>> = ureq::get(&url)
        .call()
        .map_err(|e| format!("HTTP error fetching prices: {e}"))?
        .into_json()
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let today = Local::now().date_naive().format("%Y-%m-%d").to_string();

    let mut lines: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for token in tokens {
        match response
            .get(token.id.as_str())
            .and_then(|m| m.get(currency))
        {
            Some(&price) => {
                let price_str = format_eu_price(price);
                lines.push(format!(
                    "P {} {} {} {}",
                    today, token.symbol, price_str, currency_symbol
                ));
            }
            None => missing.push(token.symbol.clone()),
        }
    }

    if lines.is_empty() {
        return Err(format!(
            "No prices received for any token (missing: {})",
            missing.join(", ")
        ));
    }

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(prices_path)
        .map_err(|e| format!("Cannot open prices file: {e}"))?;

    let block = lines.join("\n") + "\n";
    file.write_all(block.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;

    if missing.is_empty() {
        Ok(format!("Fetched prices for {} coins", lines.len()))
    } else {
        Ok(format!(
            "Fetched {} prices (not found: {})",
            lines.len(),
            missing.join(", ")
        ))
    }
}

/// Format a price as an EU-notation number with 2 decimal places.
///
/// Examples:  90123.45 → "90.123,45"   1234.5 → "1.234,50"   99.9 → "99,90"
fn format_eu_price(price: f64) -> String {
    let s = format!("{:.2}", price);
    let (int_part, frac_part) = s.split_once('.').unwrap_or((&s, "00"));

    let int_chars: Vec<char> = int_part.chars().collect();
    let mut int_out = String::with_capacity(int_chars.len() + int_chars.len() / 3);
    for (i, c) in int_chars.iter().enumerate() {
        if i > 0 && (int_chars.len() - i) % 3 == 0 {
            int_out.push('.');
        }
        int_out.push(*c);
    }
    format!("{},{}", int_out, frac_part)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_eu_price_no_thousands() {
        assert_eq!(format_eu_price(99.9), "99,90");
        assert_eq!(format_eu_price(0.5), "0,50");
    }

    #[test]
    fn format_eu_price_thousands() {
        assert_eq!(format_eu_price(1234.56), "1.234,56");
        assert_eq!(format_eu_price(90123.45), "90.123,45");
        assert_eq!(format_eu_price(1_000_000.0), "1.000.000,00");
    }

    #[test]
    fn today_prices_present_detects_todays_entry() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ID: AtomicU64 = AtomicU64::new(0);
        let id = ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("ldash-pf-test-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("prices.journal");

        assert!(!today_prices_present(&p));

        let today = Local::now().date_naive().format("%Y-%m-%d").to_string();
        std::fs::write(&p, format!("P {today} BTC 90.000,00 €\n")).unwrap();
        assert!(today_prices_present(&p));

        std::fs::write(&p, "P 2000-01-01 BTC 100,00 €\n").unwrap();
        assert!(!today_prices_present(&p));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
