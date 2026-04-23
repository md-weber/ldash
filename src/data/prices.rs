use chrono::NaiveDate;
use std::collections::HashMap;
use std::path::Path;

use super::commodity_key;
use super::parse::parse_eu_number;
use super::PriceEntry;

pub fn load_price_history(journal_dir: &Path) -> Vec<PriceEntry> {
    let prices_file = journal_dir.join("prices.journal");
    let content = std::fs::read_to_string(&prices_file).unwrap_or_default();

    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with('P') {
            continue;
        }

        // Format: P DATE COMMODITY PRICE CURRENCY
        // Use split_whitespace so tabs / repeated spaces still parse.
        let mut parts = line.split_whitespace();
        if parts.next() != Some("P") {
            continue;
        }
        let date_raw = match parts.next() {
            Some(v) => v,
            None => continue,
        };
        let commodity = match parts.next() {
            Some(v) => v.trim().trim_matches('"').to_string(),
            None => continue,
        };
        let price_raw = match parts.next() {
            Some(v) => v,
            None => continue,
        };

        let date = match NaiveDate::parse_from_str(date_raw, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };
        let price = match parse_eu_number(price_raw) {
            Some(p) => p,
            None => continue,
        };

        entries.push(PriceEntry {
            date,
            commodity,
            price_eur: price,
        });
    }

    entries.sort_by_key(|e| e.date);
    entries
}

pub fn latest_prices(price_history: &[PriceEntry]) -> HashMap<String, f64> {
    let mut latest: HashMap<String, f64> = HashMap::new();
    for entry in price_history {
        latest.insert(commodity_key(&entry.commodity), entry.price_eur);
    }
    latest
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_prices_dir() -> std::path::PathBuf {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("ldash-prices-test-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    #[test]
    fn price_history_parses_mixed_whitespace() {
        let dir = temp_prices_dir();
        std::fs::write(
            dir.join("prices.journal"),
            "P 2026-04-14 BTC 76543,21 €\nP\t2026-04-15\tSOL\t123,45\t€\n",
        )
        .expect("write");

        let entries = load_price_history(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].commodity, "BTC");
        assert_eq!(entries[1].commodity, "SOL");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn price_history_parses_line_with_trailing_comment() {
        let dir = temp_prices_dir();
        std::fs::write(
            dir.join("prices.journal"),
            "P   2026-04-14   ETH   2500,00   EUR   ; comment\n",
        )
        .expect("write");

        let entries = load_price_history(&dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].commodity, "ETH");
        assert!((entries[0].price_eur - 2500.0).abs() < 1e-10);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_prices_normalizes_commodity_key() {
        let entries = vec![
            PriceEntry {
                date: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
                commodity: "\"eth\"".to_string(),
                price_eur: 1000.0,
            },
            PriceEntry {
                date: NaiveDate::from_ymd_opt(2026, 1, 2).expect("date"),
                commodity: "ETH".to_string(),
                price_eur: 1100.0,
            },
        ];

        let lp = latest_prices(&entries);
        assert_eq!(lp.get("ETH").copied(), Some(1100.0));
    }
}
