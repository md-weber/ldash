use chrono::Local;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::data;

use super::refresh::load_all_data;
use super::{App, Tab};

/// Escape `s` for safe interpolation into HTML text / attribute context.
/// Covers the OWASP "HTML body" + attribute set: `& < > " '`. Account names,
/// commodities and month strings come from the user's journal and may contain
/// any of these (e.g. an account literally named `<script>`).
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn finish_csv(wtr: csv::Writer<Vec<u8>>) -> String {
    match wtr.into_inner() {
        Ok(buf) => String::from_utf8(buf).unwrap_or_default(),
        Err(e) => match e.into_inner().into_inner() {
            Ok(buf) => String::from_utf8(buf).unwrap_or_default(),
            Err(_) => String::new(),
        },
    }
}

impl App {
    pub fn export_current_view(&self) -> String {
        let sym = &self.config.currency_symbol;
        match self.tab {
            Tab::Portfolio => {
                let mut wtr = csv::Writer::from_writer(vec![]);
                let _ = wtr.write_record([
                    "Coin".to_string(),
                    "Amount".to_string(),
                    format!("Price {sym}"),
                    format!("Value {sym}"),
                    "Allocation %".to_string(),
                ]);
                let total = self.total_portfolio_value();
                for h in &self.holdings {
                    let alloc = if total > 0.0 {
                        h.value_eur / total * 100.0
                    } else {
                        0.0
                    };
                    let _ = wtr.write_record([
                        h.commodity.clone(),
                        format!("{:.6}", h.amount),
                        format!("{:.2}", h.price_eur),
                        format!("{:.2}", h.value_eur),
                        format!("{:.1}", alloc),
                    ]);
                }
                finish_csv(wtr)
            }
            Tab::Accounts => {
                if let Some(sel) = self.account_state.selected() {
                    if let Some(b) = self.account_balances.get(sel) {
                        return format!("{}\t{}", b.account, self.config.fmt_amount(b.amount, 2));
                    }
                }
                let mut wtr = csv::Writer::from_writer(vec![]);
                let _ = wtr.write_record(["Account".to_string(), format!("Balance {sym}")]);
                for b in &self.account_balances {
                    let _ = wtr.write_record([b.account.clone(), format!("{:.2}", b.amount)]);
                }
                finish_csv(wtr)
            }
            Tab::Monthly => {
                let empty = data::SingleMonth::default();
                let m = self.current_month().unwrap_or(&empty);
                let mut wtr = csv::Writer::from_writer(vec![]);
                let _ = wtr.write_record([format!("# {} Income/Expenses", m.month_name)]);
                let _ = wtr.write_record(["Category".to_string(), "Amount".to_string()]);
                for (name, amount) in &m.income {
                    let _ = wtr.write_record([name.clone(), format!("{:.2}", amount)]);
                }
                for (name, amount) in &m.expenses {
                    let _ = wtr.write_record([name.clone(), format!("-{:.2}", amount)]);
                }
                finish_csv(wtr)
            }
        }
    }

    /// Synchronously load any tabs whose data hasn't been fetched yet, so that
    /// an export always contains the full three-tab snapshot.
    fn ensure_all_tabs_for_export(&mut self) {
        let need = super::TabFlags {
            portfolio: self.crypto_enabled() && !self.tabs_loaded.portfolio,
            accounts: !self.tabs_loaded.accounts,
            monthly: !self.tabs_loaded.monthly,
        };
        if need.any() {
            let jp = self.journal_path.clone();
            let jd = self.journal_dir.clone();
            let nw_period = self.nw_range.period_arg().to_string();
            let currency = self.config.currency_symbol.clone();
            let assets = self.config.assets_account.clone();
            let liabilities = self.config.liabilities_account.clone();
            let result =
                load_all_data(&jp, &jd, &nw_period, &currency, &assets, &liabilities, need);
            self.apply_refresh(result);
        }
    }

    /// Write export to `dest_path`. Format is inferred from the file extension
    /// (.json → JSON, anything else → HTML). "both" config writes an additional
    /// sibling file with the other extension.
    pub fn export_to_file(&mut self, dest_path: &str) -> Result<String, String> {
        self.ensure_all_tabs_for_export();
        let path = PathBuf::from(dest_path);
        if let Some(dir) = path.parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("Cannot create dir {}: {e}", dir.display()))?;
            }
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("html");
        let fmt = self.config.export_format.as_str();

        let write_html = fmt == "both" || ext != "json";
        let write_json = fmt == "both" || ext == "json";

        let mut written: Vec<String> = Vec::new();

        if write_html {
            let html_path = path.with_extension("html");
            std::fs::write(&html_path, self.render_html())
                .map_err(|e| format!("Cannot write {}: {e}", html_path.display()))?;
            written.push(html_path.display().to_string());
        }

        if write_json {
            let json_path = path.with_extension("json");
            std::fs::write(&json_path, self.render_json())
                .map_err(|e| format!("Cannot write {}: {e}", json_path.display()))?;
            written.push(json_path.display().to_string());
        }

        Ok(written.join(" + "))
    }

    fn render_html(&self) -> String {
        let sym_raw = &self.config.currency_symbol;
        let sym = html_escape(sym_raw);
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");

        let mut portfolio_rows = String::new();
        let total = self.total_portfolio_value();
        for h in &self.holdings {
            let alloc = if total > 0.0 {
                h.value_eur / total * 100.0
            } else {
                0.0
            };
            portfolio_rows.push_str(&format!(
                "<tr><td>{}</td><td>{:.6}</td><td>{:.2}</td><td>{:.2}</td><td>{:.1}%</td></tr>\n",
                html_escape(&h.commodity),
                h.amount,
                h.price_eur,
                h.value_eur,
                alloc
            ));
        }

        let mut account_rows = String::new();
        for b in &self.account_balances {
            account_rows.push_str(&format!(
                "<tr><td>{}</td><td>{:.2}</td></tr>\n",
                html_escape(&b.account),
                b.amount
            ));
        }

        let mut monthly_rows = String::new();
        let empty = data::SingleMonth::default();
        let m = self.current_month().unwrap_or(&empty);
        for (name, amount) in &m.income {
            monthly_rows.push_str(&format!(
                "<tr><td>{}</td><td class=\"pos\">{:.2}</td></tr>\n",
                html_escape(name),
                amount
            ));
        }
        for (name, amount) in &m.expenses {
            monthly_rows.push_str(&format!(
                "<tr><td>{}</td><td class=\"neg\">-{:.2}</td></tr>\n",
                html_escape(name),
                amount
            ));
        }
        let month_title = html_escape(&m.month_name);

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>ldash export {ts}</title>
<style>
  body {{ font-family: system-ui, sans-serif; background: #0f0f17; color: #cdd6f4; margin: 2rem; }}
  h1   {{ color: #cba6f7; font-size: 1.1rem; margin-bottom: 1.5rem; }}
  h2   {{ color: #89b4fa; font-size: 0.95rem; margin: 2rem 0 0.5rem; border-bottom: 1px solid #313244; padding-bottom: 0.25rem; }}
  table {{ border-collapse: collapse; width: 100%; max-width: 700px; font-size: 0.85rem; }}
  th   {{ text-align: left; color: #6c7086; font-weight: 600; padding: 0.3rem 0.8rem; border-bottom: 1px solid #313244; }}
  td   {{ padding: 0.25rem 0.8rem; }}
  tr:hover td {{ background: #1e1e2e; }}
  .pos {{ color: #a6e3a1; }}
  .neg {{ color: #f38ba8; }}
  footer {{ margin-top: 3rem; font-size: 0.75rem; color: #45475a; }}
</style>
</head>
<body>
<h1>ldash export &mdash; {ts}</h1>

<h2>Portfolio</h2>
<table>
<tr><th>Coin</th><th>Amount</th><th>Price {sym}</th><th>Value {sym}</th><th>Allocation</th></tr>
{portfolio_rows}</table>

<h2>Accounts</h2>
<table>
<tr><th>Account</th><th>Balance {sym}</th></tr>
{account_rows}</table>

<h2>Monthly &mdash; {month_title}</h2>
<table>
<tr><th>Category</th><th>Amount {sym}</th></tr>
{monthly_rows}</table>

<footer>Generated by ldash &bull; {ts}</footer>
</body>
</html>
"#
        )
    }

    fn render_json(&self) -> String {
        let sym = &self.config.currency_symbol;
        let ts = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();

        let total = self.total_portfolio_value();
        let price_key = format!("price_{sym}");
        let value_key = format!("value_{sym}");

        let portfolio: Vec<Value> = self
            .holdings
            .iter()
            .map(|h| {
                let alloc = if total > 0.0 {
                    h.value_eur / total * 100.0
                } else {
                    0.0
                };
                json!({
                    "coin": h.commodity,
                    "amount": h.amount,
                    price_key.clone(): h.price_eur,
                    value_key.clone(): h.value_eur,
                    "allocation_pct": alloc,
                })
            })
            .collect();

        let accounts: Vec<Value> = self
            .account_balances
            .iter()
            .map(|b| json!({ "account": b.account, "balance": b.amount }))
            .collect();

        let empty = data::SingleMonth::default();
        let m = self.current_month().unwrap_or(&empty);
        let mut monthly: Vec<Value> = Vec::new();
        for (name, amount) in &m.income {
            monthly.push(json!({
                "category": name,
                "type": "income",
                "amount": amount,
            }));
        }
        for (name, amount) in &m.expenses {
            monthly.push(json!({
                "category": name,
                "type": "expense",
                "amount": -amount,
            }));
        }

        let root = json!({
            "exported": ts,
            "currency": sym,
            "portfolio": portfolio,
            "accounts": accounts,
            "monthly": {
                "month": m.month_name,
                "entries": monthly,
            },
        });

        // Pretty-print so the file stays human-readable. Falls back to the raw
        // Debug repr only if serialization itself fails (effectively impossible
        // for the value tree above).
        serde_json::to_string_pretty(&root).unwrap_or_else(|_| format!("{root:?}"))
    }

    pub fn open_export_prompt(&mut self) {
        let ext = match self.config.export_format.as_str() {
            "json" => "json",
            _ => "html",
        };
        let base_dir = self
            .config
            .export_dir
            .as_deref()
            .map(|d| {
                if d.starts_with('~') {
                    std::env::var("HOME")
                        .map(|h| d.replacen('~', &h, 1))
                        .unwrap_or_else(|_| d.to_string())
                } else {
                    d.to_string()
                }
            })
            .unwrap_or_else(|| {
                std::env::var("HOME")
                    .map(|h| format!("{}/Downloads", h))
                    .unwrap_or_else(|_| ".".to_string())
            });
        let ts = Local::now().format("%Y-%m-%d-%H%M%S");
        self.export_prompt_path = format!("{}/ldash-{}.{}", base_dir, ts, ext);
        self.export_prompt_active = true;
    }

    pub fn cancel_export_prompt(&mut self) {
        self.export_prompt_active = false;
        self.export_prompt_path.clear();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Tab};
    use crate::data::{CryptoHolding, SingleMonth};

    #[test]
    fn portfolio_export_csv_escapes_coin_name() {
        let mut app = App::fixture_empty();
        app.tab = Tab::Portfolio;
        app.holdings = vec![CryptoHolding {
            commodity: "BTC, \"main\"".to_string(),
            amount: 1.0,
            price_eur: 100.0,
            value_eur: 100.0,
        }];

        let out = app.export_current_view();
        let mut rdr = csv::Reader::from_reader(out.as_bytes());
        let rows = rdr.records().collect::<Result<Vec<_>, _>>().expect("csv");
        assert_eq!(rows[0].get(0), Some("BTC, \"main\""));
    }

    #[test]
    fn monthly_export_csv_escapes_special_chars() {
        let mut app = App::fixture_empty();
        app.tab = Tab::Monthly;
        app.monthly.months = vec![SingleMonth {
            month_name: "January".to_string(),
            income: vec![("income:salary,bonus".to_string(), 3000.0)],
            expenses: vec![("expenses:rent \"city\"\nA".to_string(), 1200.0)],
            total_income: 3000.0,
            total_expenses: 1200.0,
        }];
        app.combined_months = vec![(app.monthly.months[0].clone(), false)];
        app.combined_selected = 0;

        let out = app.export_current_view();
        assert!(out.contains("\"income:salary,bonus\",3000.00"));
        assert!(out.contains("\"expenses:rent \"\"city\"\"\nA\",-1200.00"));
    }
}
